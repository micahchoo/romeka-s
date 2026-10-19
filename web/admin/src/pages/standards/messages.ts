/**
 * The Standards page's own messages. They load with the page chunk rather than
 * with the main catalogue, which every first paint pays for
 * (`perf/budget.json`). Two strings a locale is too little to fetch on its own,
 * so every locale is imported here. Each table is typed against `en`, so a key
 * added in English and missing elsewhere is a `tsc` error.
 */
import { useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale, MessageParams } from '../../i18n';
import { hi } from './locales/hi';
import { kn } from './locales/kn';
import { ml } from './locales/ml';
import { mr } from './locales/mr';
import { ne } from './locales/ne';
import { ta } from './locales/ta';
import { te } from './locales/te';

export const en = {
  'standards.lede':
    'The shared vocabularies your fields and item types are saved in. You do not need to change them to catalogue a collection.',
  'standards.define': 'What {term} means',
  'standards.import': 'Import a vocabulary',
} as const;

type StandardsMessageKey = keyof typeof en;

/** A complete Standards page table. */
export type StandardsMessages = Readonly<Record<StandardsMessageKey, string>>;

const es: StandardsMessages = {
  'standards.lede':
    'Los vocabularios compartidos en los que se guardan sus campos y tipos de elemento. No necesita cambiarlos para catalogar una colección.',
  'standards.define': 'Qué significa {term}',
  'standards.import': 'Importar un vocabulario',
};

const MESSAGES: Readonly<Record<Locale, StandardsMessages>> = { en, es, hi, mr, ml, ta, kn, te, ne };

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
