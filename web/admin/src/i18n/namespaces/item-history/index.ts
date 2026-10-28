/**
 * The item page's History: each recorded change, who made it and what made it, and "Put this
 * value back".
 *
 * English is here, in the chunk History loads when it opens; every other locale is its own
 * chunk under `locales/`. Each locale file is typed `ItemHistoryMessages`, so a key added here
 * and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'itemHistory.loading': 'Reading the history…',
  'itemHistory.empty': 'No change to this item is recorded yet.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'A deleted user',
  'itemHistory.source.bulk': 'Bulk change {id}',
  'itemHistory.source.import': 'Import {id}',
  'itemHistory.source.bulkUndo': 'Undo of bulk change {id}',
  'itemHistory.source.importUndo': 'Undo of import {id}',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'Visibility',
  'itemHistory.field.class': 'Class',
  'itemHistory.field.template': 'Item type',
  'itemHistory.field.itemSets': 'Item sets',
  'itemHistory.field.sites': 'Sites',
  'itemHistory.public': 'Public',
  'itemHistory.private': 'Private',
  'itemHistory.none': '(none)',
  'itemHistory.untitled': 'Untitled {id}',
  'itemHistory.inTrash': '{name} (in the trash)',
  'itemHistory.deletedSet': 'Deleted set {id}',
  'itemHistory.privateValue': '{value} (private)',
  'itemHistory.putBack': 'Put this value back',
  'itemHistory.puttingBack': 'Putting the value back…',
  'itemHistory.putBackDone': 'The value is back. The newest change at the top says so.',
  'itemHistory.putBackSame': 'Nothing changed: the item has this value now.',
  'itemHistory.older': 'Show older changes',
} as const;

/** A complete table of this namespace, in one locale. */
export type ItemHistoryMessages = Table<keyof typeof en>;

export const itemHistory = defineNamespace('item-history', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
