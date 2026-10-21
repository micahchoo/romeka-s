/**
 * A language tag's name in the reader's language: `en` is "English" in an
 * English admin and "Inglés" in a Spanish one.
 *
 * The browser's own `Intl.DisplayNames` carries every language name, so the
 * admin holds no table that would go stale, and the admin's nine locales need
 * no nine translations of "French". A tag the browser does not know, or an
 * engine without `DisplayNames`, answers the tag itself, which is still exact.
 */
import type { Locale } from '../i18n';

const names = new Map<string, Intl.DisplayNames>();

const displayNames = (locale: Locale): Intl.DisplayNames | null => {
  const known = names.get(locale);
  if (known !== undefined) {
    return known;
  }
  try {
    const created = new Intl.DisplayNames([locale], { type: 'language' });
    names.set(locale, created);
    return created;
  } catch {
    return null;
  }
};

/** The language's own name, or its tag when the browser names none. */
export const languageName = (tag: string, locale: Locale): string => {
  if (tag === '') {
    return '';
  }
  try {
    return displayNames(locale)?.of(tag) ?? tag;
  } catch {
    // A malformed tag is the server's to hold; the row still shows something.
    return tag;
  }
};
