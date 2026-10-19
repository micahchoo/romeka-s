/**
 * The admin i18n surface: the catalogue, the translator and the locale store.
 */
export { en, MESSAGE_KEYS, missingKeys } from './catalogue';
export type { Catalogue, MessageKey, MessageParams } from './catalogue';
export { CATALOGUE_LOADERS } from './catalogues';
export { DEFAULT_LOCALE, LOCALES, parseLocale } from './locale';
export type { Locale } from './locale';
export {
  LOCALE_STORAGE,
  currentLocale,
  restoreLocale,
  setLocale,
  subscribeLocale,
  useLocale,
  useTranslator,
} from './store';
export { loadTermText, termText, useTermText } from './termText';
export type { TermText } from './termText';
export { localizeServerErrors, serverTranslator } from './serverText';
export type { ServerTranslate } from './serverText';
export { createTranslator, MissingMessageError, assertComplete } from './translator';
export type { Translator } from './translator';
