/**
 * The server's English messages as gettext reads them: the pattern is the key.
 *
 * `src/generated/serverMessages.json` is written by
 * `crates/omeka-api/src/messages.rs`, the one Rust home of every message the
 * API puts in an error document. Each entry maps a catalogue key to the English
 * pattern, with each substituted value as a `{name}` hole. A locale's server
 * catalogue is typed [`ServerCatalogue`], so a pattern the catalogue lacks is a
 * `tsc` error.
 *
 * This module is imported only from a lazy chunk: English needs no table,
 * because the server text already is the English.
 */
import generated from '../generated/serverMessages.json';

/** A key of the generated registry, `server.<name>`. */
export type ServerMessageKey = keyof typeof generated.messages;

/** One locale's translation of every server pattern, with the same holes. */
export type ServerCatalogue = Readonly<Record<ServerMessageKey, string>>;

const HOLE = /\{([a-z]+)\}/g;

const isServerKey = (key: string): key is ServerMessageKey =>
  Object.hasOwn(generated.messages, key);

const escapeRegExp = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** A pattern with holes, as an anchored regex whose groups are the holes. */
interface HoledPattern {
  readonly key: ServerMessageKey;
  readonly holes: readonly string[];
  readonly regex: RegExp;
  /** The pattern's text outside its holes; the longer, the more specific. */
  readonly literalLength: number;
}

const compile = (key: ServerMessageKey, pattern: string): HoledPattern => {
  const holes: string[] = [];
  let source = '^';
  let literalLength = 0;
  let last = 0;
  for (const match of pattern.matchAll(HOLE)) {
    const literal = pattern.slice(last, match.index);
    source += `${escapeRegExp(literal)}([\\s\\S]*?)`;
    literalLength += literal.length;
    holes.push(match[1] ?? '');
    last = match.index + match[0].length;
  }
  const tail = pattern.slice(last);
  return {
    key,
    holes,
    regex: new RegExp(`${source}${escapeRegExp(tail)}$`),
    literalLength: literalLength + tail.length,
  };
};

const fill = (template: string, values: ReadonlyMap<string, string>): string =>
  template.replace(HOLE, (hole, name: string) => values.get(name) ?? hole);

/**
 * A server message in the catalogue's language: an exact pattern first, then
 * the patterns with holes, most specific first, each captured value put back
 * in its hole. Text no pattern matches comes back as it is, gettext's fallback.
 */
export const createServerTranslator = (catalogue: ServerCatalogue): ((message: string) => string) => {
  const exact = new Map<string, ServerMessageKey>();
  const holed: HoledPattern[] = [];
  for (const [key, pattern] of Object.entries(generated.messages)) {
    if (!isServerKey(key)) {
      continue;
    }
    const compiled = compile(key, pattern);
    if (compiled.holes.length > 0) {
      holed.push(compiled);
    } else {
      exact.set(pattern, key);
    }
  }
  holed.sort((a, b) => b.literalLength - a.literalLength);
  return (message) => {
    const key = exact.get(message);
    if (key !== undefined) {
      return catalogue[key];
    }
    for (const pattern of holed) {
      const match = pattern.regex.exec(message);
      if (match !== null) {
        const values = new Map(pattern.holes.map((hole, index) => [hole, match[index + 1] ?? '']));
        return fill(catalogue[pattern.key], values);
      }
    }
    return message;
  };
};
