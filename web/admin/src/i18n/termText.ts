/**
 * The built-in vocabularies' labels and comments in the active locale.
 *
 * Upstream prints a vocabulary, class or property label through the
 * translator, `$translate($property->label())`, and its catalogue carries the
 * text of the four built-in vocabularies (Dublin Core, Dublin Core Type, BIBO,
 * FOAF). The port keys that text by term (`dcterms:title`, `bibo:Document`, or
 * a vocabulary's prefix) and keeps gettext's effect with one guard: a stored
 * text is translated only while it is still the built-in English for its term.
 * A user's own vocabulary, an edited label and a template's alternate label
 * therefore show as stored.
 *
 * The tables are `locales/vocabulary/{labels,comments}/<tag>.json`, which the
 * public site reads too (`crates/omeka-site/src/i18n/vocabulary.rs`). Each is
 * typed against English, so a term a locale lacks is a `tsc` error. A page that
 * shows the text loads the active locale's tables, and the English ones its
 * guard reads, as lazy chunks the first time it renders, as the page-local
 * message tables do; a locale switch alone fetches only the catalogue. English
 * needs no table, because the stored text is the English.
 */
import { useEffect, useSyncExternalStore } from 'react';
import type englishLabels from './locales/vocabulary/labels/en.json';
import type englishComments from './locales/vocabulary/comments/en.json';
import { DEFAULT_LOCALE } from './locale';
import type { Locale } from './locale';
import { useLocale } from './store';

type LabelTable = Readonly<Record<keyof (typeof englishLabels)['terms'], string>>;
type CommentTable = Readonly<Record<keyof (typeof englishComments)['terms'], string>>;

/** One locale's labels and comments, as its files hold them. */
interface TableFiles {
  readonly labels: LabelTable;
  readonly comments: CommentTable;
}

/** The two files of a locale, each its own chunk. */
const tables = (
  labels: Promise<{ readonly default: { readonly terms: LabelTable } }>,
  comments: Promise<{ readonly default: { readonly terms: CommentTable } }>,
): Promise<TableFiles> =>
  Promise.all([labels, comments]).then(([label, comment]) => ({
    labels: label.default.terms,
    comments: comment.default.terms,
  }));

/**
 * Every shipped locale's tables. A locale added to `CATALOGUE_LOADERS`
 * without them is a `tsc` error.
 */
const TABLE_LOADERS = {
  en: () => tables(import('./locales/vocabulary/labels/en.json'), import('./locales/vocabulary/comments/en.json')),
  es: () => tables(import('./locales/vocabulary/labels/es.json'), import('./locales/vocabulary/comments/es.json')),
  hi: () => tables(import('./locales/vocabulary/labels/hi.json'), import('./locales/vocabulary/comments/hi.json')),
  mr: () => tables(import('./locales/vocabulary/labels/mr.json'), import('./locales/vocabulary/comments/mr.json')),
  ml: () => tables(import('./locales/vocabulary/labels/ml.json'), import('./locales/vocabulary/comments/ml.json')),
  ta: () => tables(import('./locales/vocabulary/labels/ta.json'), import('./locales/vocabulary/comments/ta.json')),
  kn: () => tables(import('./locales/vocabulary/labels/kn.json'), import('./locales/vocabulary/comments/kn.json')),
  te: () => tables(import('./locales/vocabulary/labels/te.json'), import('./locales/vocabulary/comments/te.json')),
  ne: () => tables(import('./locales/vocabulary/labels/ne.json'), import('./locales/vocabulary/comments/ne.json')),
} satisfies Readonly<Record<Locale, () => Promise<TableFiles>>>;

/** A loaded table, looked up by any stored term. */
interface Lookup {
  readonly labels: ReadonlyMap<string, string>;
  readonly comments: ReadonlyMap<string, string>;
}

const lookup = (files: TableFiles): Lookup => ({
  labels: new Map(Object.entries(files.labels)),
  comments: new Map(Object.entries(files.comments)),
});

const loaded = new Map<Locale, Lookup>();
/** One stable snapshot per locale, replaced when its tables arrive. */
const texts = new Map<Locale, TermText>();
const loading = new Map<Locale, Promise<void>>();
const listeners = new Set<() => void>();

/**
 * Load a locale's tables and the English its guard compares against, once,
 * and re-render every [`useTermText`] caller. A chunk that fails to load
 * leaves the stored text in place and is tried again by the next render.
 */
export const loadTermText = (locale: Locale): Promise<void> => {
  if (locale === DEFAULT_LOCALE || loaded.has(locale)) {
    return Promise.resolve();
  }
  const pending = loading.get(locale);
  if (pending !== undefined) {
    return pending;
  }
  const started = Promise.all([TABLE_LOADERS.en(), TABLE_LOADERS[locale]()])
    .then(([english, local]) => {
      loaded.set(DEFAULT_LOCALE, lookup(english));
      loaded.set(locale, lookup(local));
      texts.delete(locale);
      for (const listener of listeners) {
        listener();
      }
    })
    .finally(() => {
      loading.delete(locale);
    });
  loading.set(locale, started);
  return started;
};

const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** A stored text in one locale's words; `null` stays `null`. */
type Translate = <Stored extends string | null>(term: string, stored: Stored) => Stored | string;

/** The label and the comment of a vocabulary, a class or a property, in one locale. */
export interface TermText {
  readonly locale: Locale;
  readonly label: Translate;
  readonly comment: Translate;
}

const asStored: Translate = (_term, stored) => stored;

const guarded =
  (english: ReadonlyMap<string, string>, local: ReadonlyMap<string, string>): Translate =>
  (term, stored) =>
    stored !== null && english.get(term) === stored ? (local.get(term) ?? stored) : stored;

/**
 * The term text of a locale: its tables once [`loadTermText`] has loaded them,
 * the stored text before that and in English. The same object comes back until
 * the tables arrive, so it is a stable snapshot.
 */
export const termText = (locale: Locale): TermText => {
  const cached = texts.get(locale);
  if (cached !== undefined) {
    return cached;
  }
  const english = loaded.get(DEFAULT_LOCALE);
  const local = loaded.get(locale);
  const text: TermText =
    locale !== DEFAULT_LOCALE && english !== undefined && local !== undefined
      ? {
          locale,
          label: guarded(english.labels, local.labels),
          comment: guarded(english.comments, local.comments),
        }
      : { locale, label: asStored, comment: asStored };
  texts.set(locale, text);
  return text;
};

/**
 * The built-in vocabularies' text in the active locale. The first render in a
 * locale shows the stored text and fetches the tables; their arrival renders
 * the caller again.
 */
export const useTermText = (): TermText => {
  const locale = useLocale();
  const text = useSyncExternalStore(subscribe, () => termText(locale), () => termText(locale));
  useEffect(() => {
    void loadTermText(locale).catch(() => undefined);
  }, [locale]);
  return text;
};
