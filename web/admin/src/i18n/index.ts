/**
 * The admin i18n surface: the shell catalogue, the namespaces, the translator
 * and the locale store.
 */
export { en, MESSAGE_KEYS, missingKeys } from './catalogue';
export type { Catalogue, MessageKey, MessageParams } from './catalogue';
export { CATALOGUE_LOADERS } from './catalogues';
export { DEFAULT_LOCALE, LOCALES, parseLocale } from './locale';
export { defineNamespace, tableFor } from './namespace';
export type { KeyOf, Namespace, NamespaceLoaders, Table } from './namespace';
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
export { localizeServerErrors, serverTranslator } from './serverText';
export type { ServerTranslate } from './serverText';
export { createTranslator, MissingMessageError, assertComplete, translatorOver } from './translator';
export type { Translator } from './translator';
