/**
 * The Term lists messages.
 *
 * English is here, in the chunk of the routes that import it; every other
 * locale is its own chunk under `locales/`, fetched the first time a route in
 * that locale reads it. Each locale file is typed `TermListMessages`, so a key added
 * here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

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
  'termLists.tagsLegend': 'Tags to promote',
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
  'merge.deprecates':
    'This list is published, and a published word never leaves it. {values} will stay as deprecated, replaced by “{value}”, so links to it keep working. Publish the list again to make this public.',
  'termPicker.deprecated':
    '“{value}” is deprecated in this list. It stays on this item until you pick a current entry.',
} as const;

/** A complete table of this namespace, in one locale. */
export type TermListMessages = Table<keyof typeof en>;

export const termLists = defineNamespace('term-lists', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
