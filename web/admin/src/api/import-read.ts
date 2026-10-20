/**
 * What the import parsers share: an object read that refuses members it does
 * not know, the shapes serde gives a Rust enum, and counts.
 *
 * A member the client does not know is an error, not a silent drop: the plan
 * the admin saves is built from what it parsed, so a dropped member would be
 * lost on the next save. The same rule on the read-only answers is what makes
 * a server change fail a test (`import-api.test.ts` parses every committed
 * fixture) before it reaches a person.
 */
import { ParseError, asNumber, asRecord, asString, field, isRecord, isUnknownArray, parseListOf } from './parse';
import type { JsonValue, Parser, Path } from './types';

/**
 * The object at `path`, with every member of `required` present and every
 * other member in `optional` or absent. The members are read with `field`,
 * `nullableField` and `optionalField` from `parse.ts`.
 */
export const strict = (
  raw: unknown,
  path: Path,
  required: readonly string[],
  optional: readonly string[] = [],
): Record<string, unknown> => {
  const object = asRecord(raw, path);
  for (const key of required) {
    field(object, key, path);
  }
  for (const key of Object.keys(object)) {
    if (!required.includes(key) && !optional.includes(key)) {
      throw new ParseError([...path, key], 'a member of this shape');
    }
  }
  return object;
};

/** A member read by `parse`, with the path extended for its errors. */
export const member = <T>(
  object: Record<string, unknown>,
  key: string,
  path: Path,
  parse: (value: unknown, path: Path) => T,
): T => parse(field(object, key, path), [...path, key]);

/** A member that is `null` or what `parse` reads. */
export const nullable =
  <T>(parse: (value: unknown, path: Path) => T) =>
  (value: unknown, path: Path): T | null =>
    value === null ? null : parse(value, path);

/** A whole number from 0, such as a count or a record's place. */
export const asCount = (value: unknown, path: Path): number => {
  const count = asNumber(value, path);
  if (!Number.isSafeInteger(count) || count < 0) {
    throw new ParseError(path, 'a whole number from 0');
  }
  return count;
};

/** A whole number from 1, such as a header line or a resource id. */
export const asPositive = (value: unknown, path: Path): number => {
  const count = asCount(value, path);
  if (count < 1) {
    throw new ParseError(path, 'a whole number from 1');
  }
  return count;
};

export const asFlag = (value: unknown, path: Path): boolean => {
  if (typeof value !== 'boolean') {
    throw new ParseError(path, 'boolean');
  }
  return value;
};

export const asInteger = (value: unknown, path: Path): number => {
  const number = asNumber(value, path);
  if (!Number.isSafeInteger(number)) {
    throw new ParseError(path, 'a whole number');
  }
  return number;
};

export const list = <T>(parse: (value: unknown, path: Path) => T): Parser<readonly T[]> => parseListOf(parse);

/** One of a closed set of strings. The set is an array, so a union is also its source. */
export const oneOf =
  <T extends string>(choices: readonly T[]) =>
  (value: unknown, path: Path): T => {
    const text = asString(value, path);
    const found = choices.find((choice) => choice === text);
    if (found === undefined) {
      throw new ParseError(path, `one of ${choices.join(', ')}`);
    }
    return found;
  };

/** One of `words`, or whatever `parse` reads: a string set beside an object, or beside a wider string set. */
export const wordOr =
  <W extends string, T>(words: readonly W[], parse: (value: unknown, path: Path) => T) =>
  (value: unknown, path: Path): W | T => {
    const word = words.find((candidate) => candidate === value);
    return word === undefined ? parse(value, path) : word;
  };

/**
 * A Rust enum variant with a payload, `{ "name": payload }`: the one member's
 * name and its raw payload. Unit variants are plain strings and are read with
 * `oneOf` or `wordOr`.
 */
export const variant = (raw: unknown, path: Path): readonly [string, unknown] => {
  const object = asRecord(raw, path);
  const keys = Object.keys(object);
  const [key] = keys;
  if (keys.length !== 1 || key === undefined) {
    throw new ParseError(path, 'an object with exactly one member');
  }
  return [key, object[key]];
};

/** `{ "name": payload }` read as `name` mapped to `parsers[name]`. */
export const variants =
  <T>(parsers: Readonly<Record<string, (value: unknown, path: Path) => T>>) =>
  (raw: unknown, path: Path): T => {
    const [name, payload] = variant(raw, path);
    const parse = parsers[name];
    if (parse === undefined) {
      throw new ParseError(path, `one of ${Object.keys(parsers).join(', ')}`);
    }
    return parse(payload, [...path, name]);
  };

/** An object keyed by anything, each value read by `parse`. */
export const dictionary =
  <T>(parse: (value: unknown, path: Path) => T) =>
  (raw: unknown, path: Path): Readonly<Record<string, T>> =>
    Object.fromEntries(Object.entries(asRecord(raw, path)).map(([key, value]) => [key, parse(value, [...path, key])]));

/** Any JSON value, for the members the server keeps and never reads. */
export const asJson = (value: unknown, path: Path): JsonValue => {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') {
    return value;
  }
  if (typeof value === 'number') {
    return asNumber(value, path);
  }
  if (isUnknownArray(value)) {
    return value.map((entry, index) => asJson(entry, [...path, index]));
  }
  if (isRecord(value)) {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, asJson(entry, [...path, key])]));
  }
  throw new ParseError(path, 'a JSON value');
};
