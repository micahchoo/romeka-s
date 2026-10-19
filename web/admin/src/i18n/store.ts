/**
 * The locale store: the active locale and its translator, where the choice is
 * kept, and who may read it.
 *
 * The active locale is the one the UI renders in, so it changes only once its
 * catalogue has arrived: [`setLocale`] and [`restoreLocale`] load first and
 * switch after. English is in the bundle and is active until then. The choice
 * lives in `localStorage` so a reload keeps it, and it is read back through
 * [`parseLocale`], the boundary that narrows the stored string.
 *
 * This is the same shape as `src/api/auth.ts`, a module store with a
 * `useSyncExternalStore` hook, so a component under test renders without a
 * provider.
 */
import { useSyncExternalStore } from 'react';
import { en } from './catalogue';
import { CATALOGUE_LOADERS } from './catalogues';
import { DEFAULT_LOCALE, parseLocale } from './locale';
import type { Locale } from './locale';
import { createTranslator } from './translator';
import type { Translator } from './translator';

/** The `localStorage` key. Namespaced, because the origin may host other apps. */
export const LOCALE_STORAGE = 'romeka.locale';

type Listener = () => void;

const listeners = new Set<Listener>();
let active: Translator = createTranslator(DEFAULT_LOCALE, en);
/** The latest switch asked for. A slower, older load must not overwrite it. */
let latest = 0;

const activate = (translator: Translator): void => {
  active = translator;
  document.documentElement.lang = translator.locale;
  for (const listener of listeners) {
    listener();
  }
};

/**
 * Load a locale's catalogue and make it active, or reject with the load error
 * and leave the active locale as it was. When a newer switch starts first, this
 * one resolves without switching.
 */
const load = async (locale: Locale): Promise<boolean> => {
  latest += 1;
  const ticket = latest;
  const translator = createTranslator(locale, await CATALOGUE_LOADERS[locale]());
  if (ticket !== latest) {
    return false;
  }
  activate(translator);
  return true;
};

/** The locale the UI renders in. */
export const currentLocale = (): Locale => active.locale;

/** Subscribe to locale changes, for `useSyncExternalStore`. */
export const subscribeLocale = (listener: Listener): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/**
 * Load a locale's catalogue, then store the choice and re-render every
 * subscriber in it. The `lang` attribute follows, or a screen reader announces
 * Spanish text as English (WCAG 3.1.1). Rejects, with nothing changed, when the
 * catalogue fails to load.
 */
export const setLocale = async (locale: Locale): Promise<void> => {
  if (await load(locale)) {
    window.localStorage.setItem(LOCALE_STORAGE, locale);
  }
};

/**
 * Make the stored locale active, so a reload keeps the choice. `main.tsx`
 * awaits it before the first render, so a reload in Spanish paints Spanish
 * first and never English.
 */
export const restoreLocale = async (): Promise<void> => {
  const raw = window.localStorage.getItem(LOCALE_STORAGE);
  await load(raw === null ? DEFAULT_LOCALE : parseLocale(raw));
};

const activeTranslator = (): Translator => active;

/** The translator for the active locale, tracked so a switch re-renders the caller. */
export const useTranslator = (): Translator =>
  useSyncExternalStore(subscribeLocale, activeTranslator, activeTranslator);

/** The active locale, tracked so a switch re-renders the caller. */
export const useLocale = (): Locale => useTranslator().locale;
