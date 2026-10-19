/**
 * The Term lists page's messages and the term picker's, read through the
 * `term-lists` namespace (`src/i18n/namespaces/term-lists/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { termLists } from '../../i18n/namespaces/term-lists';

export type { TermListMessages } from '../../i18n/namespaces/term-lists';

/** A Term lists message id. */
export type TermListMessageKey = KeyOf<typeof termLists>;

/** The Term lists message lookup in the current locale. */
export const useTermListText = (): ((key: TermListMessageKey, params?: MessageParams) => string) =>
  useTranslator(termLists).t;
