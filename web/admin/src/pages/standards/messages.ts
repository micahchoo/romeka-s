/**
 * The Standards page's own messages, read through the `standards` namespace
 * (`src/i18n/namespaces/standards/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { standards } from '../../i18n/namespaces/standards';

/** The Standards page's message lookup in the current locale. */
export const useStandardsText = (): ((
  key: KeyOf<typeof standards>,
  params?: MessageParams,
) => string) => useTranslator(standards).t;
