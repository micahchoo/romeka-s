/**
 * The finding messages: the jump-to box's answers in the header, and the Items screen's saved
 * lists. The box itself and the nav's list of saved lists are in the shell catalogue, because
 * they paint before any route.
 *
 * English is here, in the chunks that import it; every other locale is its own chunk under
 * `locales/`. Each locale file is typed `FindingMessages`, so a key added here and missing there
 * is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'jump.loading': 'Looking for “{text}”…',
  'jump.none': 'No record that you can see matches “{text}”.',
  'jump.many': '{count} records match “{text}”',
  'jump.firstTen': 'The first {count} are listed. Type more of the title to narrow them.',
  'jump.failed': 'The search did not finish: {message}',
  'jump.close': 'Close',
  'jump.kind.items': 'Item',
  'jump.kind.item_sets': 'Item set',
  'jump.kind.media': 'Media',
  'jump.untitled': '[Untitled]',
  'savedLists.save': 'Save this list',
  'savedLists.name': 'Name',
  'savedLists.where': 'Saves these filters and this order. The name shows under Items in the menu, on any computer you sign in to.',
  'savedLists.submit': 'Save',
  'savedLists.saving': 'Saving…',
  'savedLists.cancel': 'Cancel',
  'savedLists.current': 'Saved list: {name}',
  'savedLists.rename': 'Rename saved list',
  'savedLists.delete': 'Delete saved list',
  'savedLists.deleted': 'Deleted the saved list {name}.',
  'savedLists.nameEmpty': 'Give the list a name.',
  'savedLists.nameTaken': 'You already have a list named {name}.',
  'savedLists.tooMany': 'You have {count} saved lists. That is the most that one person can keep. Delete one first.',
  'savedLists.gone': 'This saved list was deleted in another window.',
} as const;

/** A complete table of this namespace, in one locale. */
export type FindingMessages = Table<keyof typeof en>;

export const finding = defineNamespace('finding', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
