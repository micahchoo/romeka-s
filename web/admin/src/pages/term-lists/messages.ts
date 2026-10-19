/**
 * The Term lists page's messages and the term picker's.
 *
 * They live beside the page rather than in `src/i18n/catalogue.ts` because the
 * main catalogue loads with the first paint and its budget is tight
 * (`perf/budget.json`). English is here; every other locale is its own chunk
 * under `locales/`, so a page that shows the picker pays for one locale only.
 * Each table is typed against `en`, so a key added in English and missing
 * elsewhere is a `tsc` error, and a locale with no loader is one too.
 */
import { use, useMemo } from 'react';
import { useLocale } from '../../i18n';
import type { Locale, MessageParams } from '../../i18n';

export const en = {
  'termLists.resource': 'term lists',
  'termLists.invalidId': '"{param}" is not a term list id.',
  'termLists.intro': 'A term list turns a field into a choice from a fixed set of entries, so the same place or subject is always spelled the same way.',
  'termLists.add': 'Add term list',
  'termLists.legend': 'Term list',
  'termLists.kindLegend': 'What the list holds',
  'termLists.kindTerms': 'A list of words',
  'termLists.kindItems': 'Items from an item set',
  'termLists.kindUris': 'Links (URIs) with labels',
  'termLists.kindEmpty': 'Nothing yet',
  'termLists.termsLabel': 'Words, one per line',
  'termLists.urisLabel': 'Links, one per line: the URI, a space, then the label',
  'termLists.itemSetLabel': 'Item set',
  'termLists.selectItemSet': 'Select an item set',
  'termLists.langLabel': 'Language of the words',
  'termLists.langHint': 'Optional. Every word picked from the list carries this language tag, for example en.',
  'termLists.errorNoEntries': 'Add at least one entry.',
  'termLists.errorNoItemSet': 'Choose an item set.',
  'termLists.columnKind': 'Holds',
  'termLists.columnEntries': 'Entries',
  'termLists.columnUsage': 'Items using it',
  'termLists.editLabel': 'Edit this term list',
  'termLists.deleteLabel': 'Delete this term list',
  'termLists.deleteNote': 'Deleting a list keeps the values chosen from it. They stay on their items as plain text.',
  'termPicker.loading': 'Loading the list...',
  'termPicker.noMatch': 'No entry matches "{query}".',
  'termPicker.notInList': '"{value}" is not in this list. Pick an entry to replace it.',
  'termPicker.hint': 'Type to filter the {list} list, then pick an entry.',
  'termPicker.typeLabel': 'Term list: {list}',
  'tags.title': 'Tags',
  'tags.intro': 'Free words people added while describing. When a few keep coming back, make them a list.',
  'tags.empty': 'No tags yet. Add them in the Tags field of an item, item set or media.',
  'tags.legend': 'Tags to promote',
  'tags.listName': 'List name',
  'tags.listHint': 'A new list, or the name of a list of words you have, to add the tags to it.',
  'tags.promote': 'Promote to list',
  'tags.promoteTitle': 'Promote to “{list}”',
  'tags.newList': 'This makes a new list.',
  'tags.fields': 'Item types whose Subject field will offer the list',
  'tags.fieldsSkipped': 'Item types you cannot edit, left as they are',
  'tags.skipped': 'Resources you cannot edit, which keep their tags',
  'tags.promoted': 'Promoted to “{list}”. Resources changed: {changed}.',
  'rewrite.changes': 'Resources that change',
  'rewrite.skipped': 'Resources you cannot edit, which keep their spelling',
  'rewrite.none': 'none',
  'merge.title': 'Similar spellings',
  'merge.intro': 'These look like the same value. Merge them so a search for one finds them all.',
  'merge.keep': 'Keep “{value}”',
  'merge.usedBy': 'used by {count}',
  'merge.into': 'Merge into “{value}”',
  'merge.confirm': 'Merge',
  'merge.different': 'They are different',
  'merge.merged': 'Merged into “{value}”. Resources changed: {changed}.',
  'merge.remembered': 'Noted. These spellings will not be offered for merging again.',
} as const;

/** A Term lists message id. */
export type TermListMessageKey = keyof typeof en;

/** A complete Term lists table. */
export type TermListMessages = Readonly<Record<TermListMessageKey, string>>;

const LOADERS: Readonly<Record<Locale, () => Promise<TermListMessages>>> = {
  en: () => Promise.resolve(en),
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
};

const loads = new Map<Locale, Promise<TermListMessages>>([['en', Promise.resolve(en)]]);
const loaded = new Map<Locale, TermListMessages>([['en', en]]);

// A table not loaded yet suspends the caller, so the page's Suspense boundary
// waits for it instead of every caller handling a missing table.
const tableFor = (locale: Locale): TermListMessages | Promise<TermListMessages> => {
  const ready = loaded.get(locale);
  if (ready !== undefined) {
    return ready;
  }
  let load = loads.get(locale);
  if (load === undefined) {
    load = LOADERS[locale]().then((messages) => {
      loaded.set(locale, messages);
      return messages;
    });
    loads.set(locale, load);
  }
  return load;
};

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The Term lists message lookup in the current locale. */
export const useTermListText = (): ((
  key: TermListMessageKey,
  params?: MessageParams,
) => string) => {
  const locale = useLocale();
  const table = tableFor(locale);
  const messages = table instanceof Promise ? use(table) : table;
  return useMemo(
    () =>
      (key: TermListMessageKey, params?: MessageParams): string =>
        interpolate(messages[key], params),
    [messages],
  );
};
