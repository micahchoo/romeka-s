/**
 * Every shipped locale, in switch order, and how its catalogue arrives.
 *
 * English is the default, the fallback and the shape every other catalogue is
 * typed against, so it is in the initial bundle. Every other catalogue is a
 * dynamic import, which Vite emits as its own chunk: a visitor downloads only
 * the language they read. Adding a locale is one file under `locales/` and one
 * entry here; `tsc` then asks for its `locale.<tag>` name in the catalogues.
 *
 * The key is the BCP 47 language tag, the value stored and handed to `Intl`.
 */
import { en } from './catalogue';
import type { Catalogue } from './catalogue';

export const CATALOGUE_LOADERS = {
  en: () => Promise.resolve(en),
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
} satisfies Readonly<Record<string, () => Promise<Catalogue>>>;
