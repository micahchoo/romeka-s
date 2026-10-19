/**
 * The item form's own messages. They load with the items chunk rather than
 * with the main catalogue, which every first paint pays for
 * (`perf/budget.json`). Three strings a locale is too little to fetch on its
 * own, so every locale is imported here. Each table is typed against `en`, so
 * a key added in English and missing elsewhere is a `tsc` error.
 */
import { useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale } from '../../i18n';
import { hi } from './locales/hi';
import { kn } from './locales/kn';
import { ml } from './locales/ml';
import { mr } from './locales/mr';
import { ne } from './locales/ne';
import { ta } from './locales/ta';
import { te } from './locales/te';

export const en = {
  'itemSites.legend': 'Sites',
  'itemSites.none': 'You cannot add items to any site.',
  'itemSites.createHelp':
    'If you choose no site, every site that keeps in sync with its search will add this item when it matches.',
} as const;

type ItemMessageKey = keyof typeof en;

/** A complete item form table. */
export type ItemMessages = Readonly<Record<ItemMessageKey, string>>;

const es: ItemMessages = {
  'itemSites.legend': 'Sitios',
  'itemSites.none': 'No puede añadir elementos a ningún sitio.',
  'itemSites.createHelp':
    'Si no elige ningún sitio, cada sitio sincronizado con su búsqueda añadirá este elemento cuando coincida.',
};

const MESSAGES: Readonly<Record<Locale, ItemMessages>> = { en, es, hi, mr, ml, ta, kn, te, ne };

/** The item form's message lookup in the current locale. */
export const useItemText = (): ((key: ItemMessageKey) => string) => {
  const locale = useLocale();
  return useMemo(() => {
    const messages = MESSAGES[locale];
    return (key: ItemMessageKey): string => messages[key];
  }, [locale]);
};
