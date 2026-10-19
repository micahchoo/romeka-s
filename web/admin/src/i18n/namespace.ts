/**
 * Message namespaces: the strings a group of routes renders, out of the first
 * paint.
 *
 * The shell catalogue (`catalogue.ts`) holds only what the shell itself shows.
 * Every other string lives in a namespace under `namespaces/<id>/`: its English
 * table is a static import of the routes that use it, so English never waits,
 * and every other locale is a chunk fetched the first time a route in that
 * locale needs it. A component names the namespaces it reads,
 * `useTranslator(sites, common)`, and the key type follows, so a key from an
 * undeclared namespace is a `tsc` error.
 *
 * A table that has not arrived suspends the caller (`store.ts`), so a page
 * shows its Suspense fallback and never English first. Defining a namespace
 * starts the fetch for the active locale at once, while the route's module is
 * still evaluating, and [`loadNamespaces`] fetches every defined namespace
 * before a locale switch, so the switch re-renders mounted pages without
 * suspending them.
 */
import { DEFAULT_LOCALE } from './locale';
import type { Locale } from './locale';
import { MissingMessageError } from './translator';

/** A complete table over the keys `K`, in one locale. */
export type Table<K extends string> = Readonly<Record<K, string>>;

/** One loader per non-default locale. A locale without one is a `tsc` error. */
export type NamespaceLoaders<K extends string> = Readonly<
  Record<Exclude<Locale, typeof DEFAULT_LOCALE>, () => Promise<Table<K>>>
>;

/** A namespace: its id, its English table and a key guard for ids read from data. */
export interface Namespace<K extends string> {
  readonly id: string;
  readonly en: Table<K>;
  /** Whether `key` is one of this namespace's ids, for a key a JSON registry names. */
  readonly has: (key: string) => key is K;
}

/** The key ids of a namespace type. */
export type KeyOf<N> = N extends Namespace<infer K> ? K : never;

interface Entry {
  /** Whether a locale switch fetches this table: from definition, or from its first read. */
  wanted: boolean;
  readonly english: Readonly<Record<string, string>>;
  readonly loaders: NamespaceLoaders<string>;
  readonly loaded: Map<Locale, Table<string>>;
  readonly pending: Map<Locale, Promise<Table<string>>>;
  /** One settled thenable per loaded table, so `use()` returns it at once and every render calls it. */
  readonly settled: Map<Locale, Promise<Table<string>>>;
}

const entries = new Map<Namespace<string>, Entry>();
/** The locale a newly defined namespace starts fetching for: the active one. */
let following: Locale = DEFAULT_LOCALE;

const checked = (
  id: string,
  locale: Locale,
  english: Readonly<Record<string, string>>,
  table: Table<string>,
): Table<string> => {
  const missing = Object.keys(english).filter((key) => !Object.hasOwn(table, key));
  if (missing.length > 0) {
    throw new MissingMessageError(locale, missing.map((key) => `${id}:${key}`));
  }
  return table;
};

const load = (namespace: Namespace<string>, entry: Entry, locale: Locale): Promise<Table<string>> => {
  const ready = entry.loaded.get(locale);
  if (ready !== undefined) {
    return Promise.resolve(ready);
  }
  const inFlight = entry.pending.get(locale);
  if (inFlight !== undefined) {
    return inFlight;
  }
  // `loaded` covers English, so `locale` here has a loader.
  const loader = locale === DEFAULT_LOCALE ? null : entry.loaders[locale];
  const started = (loader === null ? Promise.resolve(entry.english) : loader())
    .then((table) => {
      const complete = checked(namespace.id, locale, entry.english, table);
      entry.loaded.set(locale, complete);
      return complete;
    })
    .finally(() => {
      // A failed load is tried again by the next caller, as a retry renders.
      entry.pending.delete(locale);
    });
  entry.pending.set(locale, started);
  return started;
};

/** How a namespace is fetched. */
export interface NamespaceOptions {
  /**
   * Fetch the table only once a component reads it, not when its module loads.
   * For a heavy table a route's module graph holds but not every view of the
   * route shows, such as the vocabulary comments.
   */
  readonly onFirstRead?: boolean;
}

/**
 * Define a namespace from its English table and its locale loaders. The table
 * is the source of the key type; the loaders return tables typed against it.
 */
export const defineNamespace = <K extends string>(
  id: string,
  en: Table<K>,
  loaders: NamespaceLoaders<K>,
  options: NamespaceOptions = {},
): Namespace<K> => {
  const keys: ReadonlySet<string> = new Set(Object.keys(en));
  const namespace: Namespace<K> = { id, en, has: (key): key is K => keys.has(key) };
  const entry: Entry = {
    wanted: options.onFirstRead !== true,
    english: en,
    loaders,
    loaded: new Map([[DEFAULT_LOCALE, en]]),
    settled: new Map(),
    pending: new Map(),
  };
  entries.set(namespace, entry);
  if (entry.wanted && following !== DEFAULT_LOCALE) {
    // A failure surfaces when a component reads the table and suspends on a retry.
    load(namespace, entry, following).catch(() => undefined);
  }
  return namespace;
};

/** A namespace's table in `locale`, or the load a caller should suspend on. */
export const tableFor = <K extends string>(
  namespace: Namespace<K>,
  locale: Locale,
): Table<K> | Promise<Table<K>> => {
  const entry = entries.get(namespace);
  if (entry === undefined) {
    throw new Error(`namespace ${namespace.id} was not made by defineNamespace`);
  }
  entry.wanted = true;
  return entry.loaded.get(locale) ?? load(namespace, entry, locale);
};

/**
 * The table to hand to `use()`, the same object on every render. React replays
 * a component whose promise settled while it waited, and it mounts the hooks
 * after the suspension point only from inside `use()`. A replay that skipped
 * `use()` because the table had meanwhile arrived renders those hooks as
 * updates (React error 467), so a caller always calls `use()` on this. A
 * loaded table is a promise already marked fulfilled, which `use()` reads
 * without suspending.
 */
export const thenableFor = <K extends string>(namespace: Namespace<K>, locale: Locale): Promise<Table<K>> => {
  const table = tableFor(namespace, locale);
  if (table instanceof Promise) {
    return table;
  }
  const entry = entries.get(namespace);
  if (entry === undefined) {
    throw new Error(`namespace ${namespace.id} was not made by defineNamespace`);
  }
  const cached = entry.settled.get(locale);
  if (cached !== undefined) {
    return cached;
  }
  const settled = Object.assign(Promise.resolve(table), { status: 'fulfilled', value: table });
  entry.settled.set(locale, settled);
  return settled;
};

/**
 * Fetch the table in `locale` of every namespace defined so far, except one
 * defined to wait for its first read that nothing has read yet. Rejects if
 * any fails.
 */
export const loadNamespaces = async (locale: Locale): Promise<void> => {
  await Promise.all(
    [...entries]
      .filter(([, entry]) => entry.wanted)
      .map(([namespace, entry]) => load(namespace, entry, locale)),
  );
};

/** Make `locale` the one a namespace defined from now on starts fetching. */
export const followLocale = (locale: Locale): void => {
  following = locale;
};
