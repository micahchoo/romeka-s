/**
 * The class filter's words, wherever resources are filtered by class: the Items
 * browse, the item picker and a block's query (`src/app/class-choices.ts`).
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `ClassFilterMessages`, so a
 * key added here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'classFilter.option': '{label} · {count} items',
  'classFilter.optionOne': '{label} · 1 item',
  'classFilter.optionItemSets': '{label} · {count} item sets',
  'classFilter.optionItemSetOne': '{label} · 1 item set',
  'classFilter.optionMedia': '{label} · {count} media',
  'classFilter.optionMediaOne': '{label} · 1 media',
  'classFilter.standard': '{label} ({standard})',
} as const;

/** A complete table of this namespace, in one locale. */
export type ClassFilterMessages = Table<keyof typeof en>;

export const classFilter = defineNamespace('class-filter', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  kn: () => import('./locales/kn').then((module) => module.kn),
  ml: () => import('./locales/ml').then((module) => module.ml),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ne: () => import('./locales/ne').then((module) => module.ne),
  ta: () => import('./locales/ta').then((module) => module.ta),
  te: () => import('./locales/te').then((module) => module.te),
});
