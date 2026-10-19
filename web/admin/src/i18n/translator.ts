/**
 * The translator: a locale and its catalogue behind `t(key, params)`.
 *
 * There is no read-time fallback. A key is a `MessageKey`, and a catalogue is
 * `Record<MessageKey, string>`, so a missing key is a compile error. The
 * runtime guard exists for a catalogue that arrives at the edge (a lazy chunk,
 * a test) and shares `missingKeys` with the test that proves it fails.
 */
import { missingKeys } from './catalogue';
import type { Catalogue, MessageKey, MessageParams } from './catalogue';
import type { Locale } from './locale';

/** Raised when a catalogue is incomplete, naming the missing ids. */
export class MissingMessageError extends Error {
  readonly locale: Locale;
  readonly keys: readonly string[];

  constructor(locale: Locale, keys: readonly string[]) {
    super(
      `the ${locale} catalogue is missing ${String(keys.length)} message(s): ${keys.join(', ')}`,
    );
    this.name = 'MissingMessageError';
    this.locale = locale;
    this.keys = keys;
  }
}

/**
 * Throws a `MissingMessageError` when a catalogue lacks any English key. The
 * shipped catalogues are `Catalogue` typed, so this never fires for them; it is
 * the check a catalogue loaded at runtime must pass, and the test that proves a
 * missing key fails calls it directly.
 */
export const assertComplete = (
  locale: Locale,
  catalogue: Readonly<Record<string, string>>,
): void => {
  const missing = missingKeys(catalogue);
  if (missing.length > 0) {
    throw new MissingMessageError(locale, missing);
  }
};

/** A locale-bound message lookup. */
export interface Translator {
  readonly locale: Locale;
  /** The message for `key`, with `{name}` placeholders filled from `params`. */
  readonly t: (key: MessageKey, params?: MessageParams) => string;
  /** A number in the locale's grouping, for counts and byte sizes. */
  readonly number: (value: number) => string;
}

const interpolate = (template: string, params: MessageParams | undefined): string => {
  if (params === undefined) {
    return template;
  }
  return template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
    const value = params[name];
    return value === undefined ? match : String(value);
  });
};

/**
 * A translator for one locale over its loaded catalogue, or a
 * `MissingMessageError` when the catalogue is short.
 */
export const createTranslator = (locale: Locale, catalogue: Catalogue): Translator => {
  assertComplete(locale, catalogue);
  const numbers = new Intl.NumberFormat(locale);
  return {
    locale,
    t: (key, params) => interpolate(catalogue[key], params),
    number: (value) => numbers.format(value),
  };
};
