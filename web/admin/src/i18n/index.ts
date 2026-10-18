/**
 * The admin i18n surface: the catalogue, the translator and the locale store.
 */
export { en, MESSAGE_KEYS, missingKeys } from './catalogue';
export type { Catalogue, MessageKey, MessageParams } from './catalogue';
export { CATALOGUES } from './catalogues';
export { DEFAULT_LOCALE, LOCALES, LOCALE_TAGS, parseLocale } from './locale';
export type { Locale } from './locale';
export {
  LOCALE_STORAGE,
  applyDocumentLang,
  currentLocale,
  setLocale,
  subscribeLocale,
  useLocale,
  useTranslator,
} from './store';
export { createTranslator, MissingMessageError, assertComplete } from './translator';
export type { Translator } from './translator';
