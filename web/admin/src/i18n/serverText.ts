/**
 * Server error text in the active locale, for the fetch boundaries.
 *
 * The API answers in English, and the English is upstream's msgid, so the SPA
 * translates it the way gettext does (`serverMessages.ts`). English needs no
 * table: the server text is the English. Any other locale fetches its server
 * catalogue and the pattern table the first time an error arrives, as lazy
 * chunks, so neither is in the initial bundle.
 */
import { DEFAULT_LOCALE } from './locale';
import type { Locale } from './locale';
import type { ServerCatalogue } from './serverMessages';
import { currentLocale } from './store';

/** A server message in one locale's words. */
export type ServerTranslate = (message: string) => string;

/**
 * Each non-default locale's server catalogue. A locale added to
 * `CATALOGUE_LOADERS` without one here is a `tsc` error.
 */
const SERVER_CATALOGUE_LOADERS = {
  es: () => import('./locales/es.server').then((module) => module.esServer),
  hi: () => import('./locales/hi.server').then((module) => module.hiServer),
  mr: () => import('./locales/mr.server').then((module) => module.mrServer),
  ml: () => import('./locales/ml.server').then((module) => module.mlServer),
  ta: () => import('./locales/ta.server').then((module) => module.taServer),
  kn: () => import('./locales/kn.server').then((module) => module.knServer),
  te: () => import('./locales/te.server').then((module) => module.teServer),
  ne: () => import('./locales/ne.server').then((module) => module.neServer),
} satisfies Readonly<
  Record<Exclude<Locale, typeof DEFAULT_LOCALE>, () => Promise<ServerCatalogue>>
>;

const unchanged: ServerTranslate = (message) => message;

const loaded = new Map<Locale, Promise<ServerTranslate>>();

/**
 * The server-message translator for a locale. A chunk that fails to load
 * leaves the text in English rather than hide the error, and is tried again
 * on the next error.
 */
export const serverTranslator = (locale: Locale): Promise<ServerTranslate> => {
  if (locale === DEFAULT_LOCALE) {
    return Promise.resolve(unchanged);
  }
  const cached = loaded.get(locale);
  if (cached !== undefined) {
    return cached;
  }
  const loading = Promise.all([SERVER_CATALOGUE_LOADERS[locale](), import('./serverMessages')])
    .then(([catalogue, { createServerTranslator }]) => createServerTranslator(catalogue))
    .catch(() => {
      loaded.delete(locale);
      return unchanged;
    });
  loaded.set(locale, loading);
  return loading;
};

/** An error document's messages in the active locale, keys unchanged. */
export const localizeServerErrors = async (
  errors: Readonly<Record<string, readonly string[]>>,
): Promise<Readonly<Record<string, readonly string[]>>> => {
  const translate = await serverTranslator(currentLocale());
  return Object.fromEntries(
    Object.entries(errors).map(([key, messages]) => [key, messages.map(translate)]),
  );
};
