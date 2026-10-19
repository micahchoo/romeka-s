/**
 * The Standards page's own messages, in English and Spanish. They load with
 * the page chunk rather than with the main catalogue, which every first paint
 * pays for (`perf/budget.json`). `es` is typed against `en`, so a key added in
 * English and missing in Spanish is a `tsc` error.
 */
import { useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale, MessageParams } from '../../i18n';

const en = {
  'standards.lede':
    'The shared vocabularies your fields and item types are saved in. You do not need to change them to catalogue a collection.',
  'standards.define': 'What {term} means',
} as const;

type StandardsMessageKey = keyof typeof en;

const es: Readonly<Record<StandardsMessageKey, string>> = {
  'standards.lede':
    'Los vocabularios compartidos en los que se guardan sus campos y tipos de elemento. No necesita cambiarlos para catalogar una colección.',
  'standards.define': 'Qué significa {term}',
};

const MESSAGES: Readonly<Record<Locale, Readonly<Record<StandardsMessageKey, string>>>> = {
  en,
  es,
};

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The Standards page's message lookup in the current locale. */
export const useStandardsText = (): ((
  key: StandardsMessageKey,
  params?: MessageParams,
) => string) => {
  const locale = useLocale();
  return useMemo(() => {
    const messages = MESSAGES[locale];
    return (key: StandardsMessageKey, params?: MessageParams): string =>
      interpolate(messages[key], params);
  }, [locale]);
};
