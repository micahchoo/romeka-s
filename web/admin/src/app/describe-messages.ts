/**
 * The strings the item, item set and media forms share with the item type
 * pages: the Type picker, the "Advanced" fold, the term picker and "?". They
 * are read through the `common` namespace (`src/i18n/namespaces/common/`).
 */
import { useTranslator } from '../i18n';
import type { KeyOf, MessageParams } from '../i18n';
import { common } from '../i18n/namespaces/common';

/** The shared describe strings' lookup in the current locale. */
export const useDescribeText = (): ((
  key: KeyOf<typeof common>,
  params?: MessageParams,
) => string) => useTranslator(common).t;
