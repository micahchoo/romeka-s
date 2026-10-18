/**
 * The locales the admin SPA ships, and the boundary that reads one back.
 *
 * A locale arrives from `localStorage` as an untrusted string, so
 * [`parseLocale`] narrows it here and everything downstream hands around the
 * [`Locale`] union. Upstream stores a `locale` setting on the global settings,
 * the user and the site (`application/src/Mvc/MvcListeners.php`); the SPA reads
 * the browser-stored admin choice, which is the user-level locale of that
 * chain.
 */

/** The locales a catalogue exists for, in switch order. */
export type Locale = 'en' | 'es';

/** The locale the SPA opens with when nothing is stored. Upstream's default is `en_US`. */
export const DEFAULT_LOCALE: Locale = 'en';

/** Every shipped locale, in switch order. The first is [`DEFAULT_LOCALE`]. */
export const LOCALES: readonly Locale[] = ['en', 'es'];

/** The BCP 47 tag for a locale, the value stored and handed to `Intl`. */
export const LOCALE_TAGS: Readonly<Record<Locale, string>> = {
  en: 'en',
  es: 'es',
};

const isLocale = (value: string): value is Locale => LOCALES.some((locale) => locale === value);

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
