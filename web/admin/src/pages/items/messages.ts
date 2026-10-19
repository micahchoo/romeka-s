/**
 * The item form's own messages, read through the `resources` namespace
 * (`src/i18n/namespaces/resources/`).
 */
import { useTranslator } from '../../i18n';
import type { KeyOf } from '../../i18n';
import { resources } from '../../i18n/namespaces/resources';

/** The item form's message lookup in the current locale. */
export const useItemText = (): ((key: KeyOf<typeof resources>) => string) =>
  useTranslator(resources).t;
