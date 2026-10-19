/**
 * The Theme page's messages, read through the `theme` namespace
 * (`src/i18n/namespaces/theme/`), so the page suspends rather than renders
 * English while another locale's table is in flight.
 * `src/generated/themeSettings.json` names only keys that table holds.
 */
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageParams } from '../../../i18n';
import { theme } from '../../../i18n/namespaces/theme';

export type { ThemeMessages } from '../../../i18n/namespaces/theme';

/** A Theme page message id. */
export type ThemeMessageKey = KeyOf<typeof theme>;

/** Whether a string is a Theme page message id. */
export const isThemeMessageKey = theme.has;

/** The Theme page's message lookup in the active locale. */
export const useThemeText = (): ((key: ThemeMessageKey, params?: MessageParams) => string) =>
  useTranslator(theme).t;
