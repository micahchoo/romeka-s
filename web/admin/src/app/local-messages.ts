/**
 * Message tables a lazy page owns, the page-local pattern the Theme page
 * started (`pages/sites/themeEditor/messages.ts`): the main catalogues are
 * part of the first paint and sit near their budget, so a page with many
 * strings keeps them in a module its chunk imports. The guarantee is the main
 * catalogue's: a `LocalMessages` table is a record over every `Locale`, each
 * holding every key the English table names, so a missing key is a `tsc` error.
 */
import { useMemo } from 'react';
import { useLocale } from '../i18n';
import type { Locale, MessageParams } from '../i18n';

export type LocalMessages<K extends string> = Readonly<Record<Locale, Readonly<Record<K, string>>>>;

export interface LocalText<K extends string> {
  readonly t: (key: K, params?: MessageParams) => string;
  readonly number: (value: number) => string;
}

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

export const localText = <K extends string>(
  messages: LocalMessages<K>,
  locale: Locale,
): LocalText<K> => {
  const table = messages[locale];
  const numbers = new Intl.NumberFormat(locale);
  return {
    t: (key, params) => interpolate(table[key], params),
    number: (value) => numbers.format(value),
  };
};

/** The lookup over a page's own table, in the selected locale. */
export const useLocalText = <K extends string>(messages: LocalMessages<K>): LocalText<K> => {
  const locale = useLocale();
  return useMemo(() => localText(messages, locale), [messages, locale]);
};
