/**
 * The locales the admin SPA ships, and the boundary that reads one back.
 *
 * A locale arrives from `localStorage` as an untrusted string, so
 * [`parseLocale`] narrows it here and everything downstream hands around the
 * [`Locale`] union. Upstream stores a `locale` setting on the global settings,
 * the user and the site (`application/src/Mvc/MvcListeners.php`). The server
 * resolves the user and global settings into `o:admin_locale`, which the
 * session applies over the browser-stored choice (`src/api/session.ts`).
 */
import { CATALOGUE_LOADERS } from './catalogues';

/** A locale a catalogue exists for: a key of [`CATALOGUE_LOADERS`]. */
export type Locale = keyof typeof CATALOGUE_LOADERS;

/**
 * The locale the SPA opens with when nothing is stored, and the one whose
 * catalogue is in the initial bundle. Upstream's default is `en_US`.
 */
export const DEFAULT_LOCALE = 'en' satisfies Locale;

const isLocale = (value: string): value is Locale => Object.hasOwn(CATALOGUE_LOADERS, value);

/** Every shipped locale, in switch order. The first is [`DEFAULT_LOCALE`]. */
export const LOCALES: readonly Locale[] = Object.keys(CATALOGUE_LOADERS).filter(isLocale);

/**
 * A stored tag as a `Locale`, falling back to [`DEFAULT_LOCALE`]. The boundary
 * accepts a bare language tag (`en`) and drops a region (`en_US` becomes `en`),
 * which is how upstream's catalogue names reach the SPA.
 */
export const parseLocale = (value: string): Locale => {
  if (isLocale(value)) {
    return value;
  }
  const language = value.split('_')[0]?.split('-')[0] ?? '';
  return isLocale(language) ? language : DEFAULT_LOCALE;
};
