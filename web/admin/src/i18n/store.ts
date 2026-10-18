/**
 * The locale store: the selected locale, where it is kept, and who may read it.
 *
 * The locale lives in `localStorage` so a reload keeps the choice, and it is
 * read back through [`parseLocale`], the boundary that narrows the stored
 * string. Components subscribe with [`useLocale`]; the translator is built per
 * locale with [`useTranslator`].
 *
 * This is the same shape as `src/api/auth.ts`, a module store with a
 * `useSyncExternalStore` hook, so a component under test renders without a
 * provider.
 */
import { useMemo, useSyncExternalStore } from 'react';
import { DEFAULT_LOCALE, LOCALE_TAGS, parseLocale } from './locale';
import type { Locale } from './locale';
import { createTranslator } from './translator';
import type { Translator } from './translator';

/** The `localStorage` key. Namespaced, because the origin may host other apps. */
export const LOCALE_STORAGE = 'romeka.locale';

type Listener = () => void;

const listeners = new Set<Listener>();
let current: Locale = DEFAULT_LOCALE;
let loaded = false;

const readStored = (): Locale => {
  const raw = window.localStorage.getItem(LOCALE_STORAGE);
  return raw === null ? DEFAULT_LOCALE : parseLocale(raw);
};

const publish = (): void => {
  for (const listener of listeners) {
    listener();
  }
};

/**
 * The selected locale. The first call reads `localStorage`, so a reload keeps
 * the choice.
 */
export const currentLocale = (): Locale => {
  if (!loaded) {
    loaded = true;
    current = readStored();
  }
  return current;
};

/** Subscribe to locale changes, for `useSyncExternalStore`. */
export const subscribeLocale = (listener: Listener): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** Select a locale, store it, and re-render every subscriber. */
export const setLocale = (locale: Locale): void => {
  window.localStorage.setItem(LOCALE_STORAGE, locale);
  loaded = true;
  current = locale;
  applyDocumentLang();
  publish();
};

/**
 * Write the selected locale to the document's `lang` attribute. The SPA opens
 * with `lang="en"` in `index.html`, so a switch to another language must update
 * it, or a screen reader announces Spanish text as English (WCAG 3.1.1). Called
 * once at startup and on every switch.
 */
export const applyDocumentLang = (): void => {
  document.documentElement.lang = LOCALE_TAGS[currentLocale()];
};

/** The selected locale, tracked so a switch re-renders the caller. */
export const useLocale = (): Locale => useSyncExternalStore(subscribeLocale, currentLocale, currentLocale);

/** The translator for the selected locale. Memoised per locale. */
export const useTranslator = (): Translator => {
  const locale = useLocale();
  return useMemo(() => createTranslator(locale), [locale]);
};
