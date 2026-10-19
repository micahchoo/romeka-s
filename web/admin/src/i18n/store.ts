/**
 * The locale store: the active locale and its translator, where the choice is
 * kept, and who may read it.
 *
 * The active locale is the one the UI renders in, so it changes only once its
 * catalogue and the table of every namespace defined so far have arrived:
 * [`setLocale`] and [`restoreLocale`] load first and switch once after, so no
 * mounted page suspends and no draft is lost. English is in the bundle and is
 * active until then. The choice
 * lives in `localStorage` so a reload keeps it, and it is read back through
 * [`parseLocale`], the boundary that narrows the stored string.
 *
 * This is the same shape as `src/api/auth.ts`, a module store with a
 * `useSyncExternalStore` hook, so a component under test renders without a
 * provider.
 */
import { use, useSyncExternalStore } from 'react';
import { en } from './catalogue';
import type { Catalogue, MessageKey } from './catalogue';
import { CATALOGUE_LOADERS } from './catalogues';
import { DEFAULT_LOCALE, parseLocale } from './locale';
import type { Locale } from './locale';
import { followLocale, loadNamespaces, tableFor } from './namespace';
import type { KeyOf, Namespace, Table } from './namespace';
import { createTranslator, translatorOver } from './translator';
import type { Translator } from './translator';

/** The `localStorage` key. Namespaced, because the origin may host other apps. */
export const LOCALE_STORAGE = 'romeka.locale';

type Listener = () => void;

const listeners = new Set<Listener>();
/** The active locale's translator and the catalogue it reads, swapped together. */
interface Active {
  readonly translator: Translator;
  readonly catalogue: Catalogue;
}

let active: Active = { translator: createTranslator(DEFAULT_LOCALE, en), catalogue: en };
/** The latest switch asked for. A slower, older load must not overwrite it. */
let latest = 0;

const activate = (translator: Translator, catalogue: Catalogue): void => {
  active = { translator, catalogue };
  followLocale(translator.locale);
  document.documentElement.lang = translator.locale;
  for (const listener of listeners) {
    listener();
  }
};

/**
 * Load a locale's catalogue and every defined namespace's table, then make the
 * locale active, or reject with the load error and leave the active locale as
 * it was. When a newer switch starts first, this one resolves without
 * switching.
 */
const load = async (locale: Locale): Promise<boolean> => {
  latest += 1;
  const ticket = latest;
  const [catalogue] = await Promise.all([CATALOGUE_LOADERS[locale](), loadNamespaces(locale)]);
  const translator = createTranslator(locale, catalogue);
  if (ticket !== latest) {
    return false;
  }
  activate(translator, catalogue);
  return true;
};

/** The locale the UI renders in. */
export const currentLocale = (): Locale => active.translator.locale;

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

const activeSnapshot = (): Active => active;

/** Per active locale, the translator of each namespace list, so `t` stays stable. */
const extended = new WeakMap<Active, Map<string, Translator<string>>>();

const extend = (
  snapshot: Active,
  namespaces: readonly Namespace<string>[],
  tables: readonly Table<string>[],
): Translator<string> => {
  let byIds = extended.get(snapshot);
  if (byIds === undefined) {
    byIds = new Map();
    extended.set(snapshot, byIds);
  }
  const ids = namespaces.map((namespace) => namespace.id).join(' ');
  let translator = byIds.get(ids);
  if (translator === undefined) {
    translator = translatorOver(snapshot.translator.locale, [snapshot.catalogue, ...tables]);
    byIds.set(ids, translator);
  }
  return translator;
};

/**
 * The translator for the active locale over the shell catalogue and the
 * namespaces named, tracked so a switch re-renders the caller. A namespace
 * whose table has not arrived suspends the caller until it has, so the nearest
 * Suspense boundary shows its fallback rather than English.
 */
export const useTranslator = <const N extends readonly Namespace<string>[]>(
  ...namespaces: N
): Translator<MessageKey | KeyOf<N[number]>> => {
  const snapshot = useSyncExternalStore(subscribeLocale, activeSnapshot, activeSnapshot);
  const tables: Table<string>[] = [];
  for (const namespace of namespaces) {
    const table = tableFor(namespace, snapshot.translator.locale);
    tables.push(table instanceof Promise ? use(table) : table);
  }
  return extend(snapshot, namespaces, tables);
};

/** The active locale, tracked so a switch re-renders the caller. */
export const useLocale = (): Locale =>
  useSyncExternalStore(subscribeLocale, activeSnapshot, activeSnapshot).translator.locale;
