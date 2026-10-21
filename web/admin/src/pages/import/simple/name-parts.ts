/**
 * A file name in parts: split on `_`, `-`, `.` and space, as the server
 * splits it for its name words (`preview/names.rs`), and also where a run of
 * digits meets a run of letters (`010a`, `p12v`), which the server does not
 * split. Such a glued part has no separator before it. The person clicks
 * one part of a name; the mark covers that part and every part after it up
 * to the ending, because the shared pattern (`../example.ts`) reads the
 * marked place at the end of the name.
 */
import type { Mark } from '../example';

/** One part of a name and the separator before it (`''` for the first and for a glued part). */
export interface NamePart {
  readonly text: string;
  readonly sep: string;
}

const SEPARATORS = '_-. ';
const DIGIT = /^\p{Nd}$/u;
const LETTER = /^\p{L}$/u;

export const splitName = (name: string): readonly NamePart[] => {
  const parts: NamePart[] = [];
  let sep = '';
  let text = '';
  let last = '';
  for (const char of name) {
    if (SEPARATORS.includes(char)) {
      parts.push({ text, sep });
      sep = char;
      text = '';
    } else if (DIGIT.test(last) && LETTER.test(char)) {
      parts.push({ text, sep });
      sep = '';
      text = char;
    } else {
      text += char;
    }
    last = char;
  }
  parts.push({ text, sep });
  return parts;
};

const hasEnding = (parts: readonly NamePart[]): boolean => parts.length > 1 && parts[parts.length - 1]?.sep === '.';

/** The parts a person may click: all but the ending. */
export const stemParts = (name: string): readonly NamePart[] => {
  const parts = splitName(name);
  return hasEnding(parts) ? parts.slice(0, -1) : parts;
};

export const endingOf = (name: string): string | null => {
  const parts = splitName(name);
  return hasEnding(parts) ? (parts[parts.length - 1]?.text ?? null) : null;
};

export const nameOf = (path: string): string => path.slice(path.lastIndexOf('/') + 1);

/** The text from part `from` to the end of the name, separators kept ("page 001"). */
export const markedText = (path: string, from: number): string =>
  stemParts(nameOf(path))
    .slice(from)
    .map((part, index) => (index === 0 ? part.text : part.sep + part.text))
    .join('');

/** The mark for the parts from `from` to the end. The first part starts the item's name, so it cannot be marked. */
export const markOf = (path: string, from: number, answer: { readonly kind: Mark['kind']; readonly words: readonly string[] }): Mark => {
  const first = stemParts(nameOf(path))[from];
  if (from < 1 || first === undefined) {
    throw new RangeError(`part ${String(from)} cannot be marked in ${path}`);
  }
  return answer.kind === 'words' ? { kind: 'words', words: answer.words, sep: first.sep } : { kind: answer.kind, text: markedText(path, from), sep: first.sep };
};
