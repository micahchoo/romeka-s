/**
 * Every shipped catalogue, by locale. The `Record<Locale, Catalogue>` shape
 * makes a locale with no catalogue a `tsc` error and keeps the set complete.
 */
import { en } from './catalogue';
import type { Catalogue } from './catalogue';
import type { Locale } from './locale';
import { es } from './locales/es';

export const CATALOGUES: Readonly<Record<Locale, Catalogue>> = { en, es };
