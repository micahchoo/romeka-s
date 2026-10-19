/**
 * The built-in vocabularies' labels in the active locale, and the types the
 * labels and comments share.
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
 * public site reads too (`crates/omeka-site/src/i18n/vocabulary.rs`). The
 * labels and the comments are two message namespaces (`namespace.ts`): a page
 * that shows the text names the one it reads and suspends until that table
 * has arrived, so it never shows the English first. Comments are the heavy
 * half, so they are a module of their own (`termComments.ts`) and are fetched
 * only once a view reads them: a view that shows labels only never fetches
 * them. English needs no table, because the stored text is the English.
 *
 * Neither module is exported from `src/i18n/index.ts`: that index is in the
 * first paint, and importing a namespace module is what defines it and starts
 * its fetch.
 */
import { useMemo, useRef } from 'react';
import { DEFAULT_LOCALE } from './locale';
import type { Locale } from './locale';
import { defineNamespace, tableFor } from './namespace';
import type { Namespace, NamespaceLoaders, NamespaceOptions, Table } from './namespace';
import { readTable, useLocale } from './store';

/** A vocabulary file's terms, keyed by term. */
export type TermFile = Readonly<Record<string, string>>;

/** A JSON table's module, as Vite imports it. */
export interface TermModule {
  readonly default: { readonly terms: TermFile };
}

/** A stored text in one locale's words; `null` stays `null`. */
export type Translate = <Stored extends string | null>(term: string, stored: Stored) => Stored | string;

/** The label of a vocabulary, a class or a property, in one locale. */
export interface TermLabels {
  readonly locale: Locale;
  readonly label: Translate;
}

/** The label and the comment of a vocabulary, a class or a property, in one locale. */
export interface TermText extends TermLabels {
  readonly comment: Translate;
}

// One key holds the guard: the term and the built-in English it was written for.
const guardKey = (term: string, english: string): string => `${term}\u0000${english}`;

/**
 * A locale's table as a namespace holds it: each translation keyed by its term
 * and that term's built-in English, so a lookup by (term, stored text) finds
 * it only while the stored text is still the built-in English.
 */
const guardedTable = (english: TermFile, local: TermFile): Table<string> =>
  Object.fromEntries(
    Object.entries(local).flatMap(([term, text]) => {
      const original = english[term];
      return original === undefined ? [] : [[guardKey(term, original), text]];
    }),
  );

/** A namespace over one kind of vocabulary text, from its English file and each locale's. */
export const defineTermTable = (
  id: string,
  english: () => Promise<TermModule>,
  files: Readonly<Record<Exclude<Locale, typeof DEFAULT_LOCALE>, () => Promise<TermModule>>>,
  options: NamespaceOptions = {},
): Namespace<string> => {
  const load =
    (file: () => Promise<TermModule>) => (): Promise<Table<string>> =>
      Promise.all([english(), file()]).then(([base, local]) =>
        guardedTable(base.default.terms, local.default.terms),
      );
  const loaders: NamespaceLoaders<string> = {
    es: load(files.es),
    hi: load(files.hi),
    mr: load(files.mr),
    ml: load(files.ml),
    ta: load(files.ta),
    kn: load(files.kn),
    te: load(files.te),
    ne: load(files.ne),
  };
  // English renders the stored text, so its table is empty and never fetched.
  return defineNamespace<string>(id, {}, loaders, options);
};

/** The lookup a guarded table answers. */
export const translateWith =
  (table: Table<string>): Translate =>
  (term, stored) =>
    stored === null ? stored : (table[guardKey(term, stored)] ?? stored);

/** The built-in vocabulary labels. A locale without its file is a `tsc` error. */
export const vocabularyLabels = defineTermTable(
  'vocabulary-labels',
  () => import('./locales/vocabulary/labels/en.json'),
  {
    es: () => import('./locales/vocabulary/labels/es.json'),
    hi: () => import('./locales/vocabulary/labels/hi.json'),
    mr: () => import('./locales/vocabulary/labels/mr.json'),
    ml: () => import('./locales/vocabulary/labels/ml.json'),
    ta: () => import('./locales/vocabulary/labels/ta.json'),
    kn: () => import('./locales/vocabulary/labels/kn.json'),
    te: () => import('./locales/vocabulary/labels/te.json'),
    ne: () => import('./locales/vocabulary/labels/ne.json'),
  },
);

/** A namespace's table in `locale`, suspending the caller until it has arrived. */
export const useTermTable = (namespace: Namespace<string>, locale: Locale): Table<string> => {
  const suspendedAt = useRef<Set<number>>(new Set());
  return readTable(namespace, locale, suspendedAt.current, 0);
};

/**
 * The built-in vocabularies' labels in the active locale. The caller suspends
 * until the active locale's table has arrived.
 */
export const useTermLabels = (): TermLabels => {
  const locale = useLocale();
  const labels = useTermTable(vocabularyLabels, locale);
  return useMemo(() => ({ locale, label: translateWith(labels) }), [locale, labels]);
};

/** The stored text, unchanged: English, or a table not yet arrived. */
export const asStored: Translate = (_term, stored) => stored;

/**
 * The labels of a locale outside a component: translated once that locale's
 * table has arrived, the stored text before that.
 */
export const termLabels = (locale: Locale): TermLabels => {
  const table = tableFor(vocabularyLabels, locale);
  return { locale, label: table instanceof Promise ? asStored : translateWith(table) };
};

