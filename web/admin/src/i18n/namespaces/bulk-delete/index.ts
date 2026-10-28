/**
 * The bulk delete's messages: its confirm step, and the administrator's "Empty the trash" in the
 * recent changes list.
 *
 * English is here, in the chunk the delete step loads when it opens; every other locale is its
 * own chunk under `locales/`. Each locale file is typed `BulkDeleteMessages`, so a key added here
 * and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'bulkDelete.scope.refused': 'Your role can delete ticked items only. Tick the items to delete.',
  'bulkDelete.ask.note': 'Preview shows what goes. Nothing moves to the trash until you confirm.',
  'bulkDelete.confirm.itemsOne': '1 item goes to the trash.',
  'bulkDelete.confirm.itemsMany': '{count} items go to the trash.',
  'bulkDelete.confirm.none': 'None of these items is one you may delete.',
  'bulkDelete.confirm.mediaOne': '1 media file goes too: {size}.',
  'bulkDelete.confirm.mediaMany': '{count} media files go too: {size} in all.',
  'bulkDelete.confirm.deniedOne': '1 item you may not delete stays as it is.',
  'bulkDelete.confirm.deniedMany': '{count} items you may not delete stay as they are.',
  'bulkDelete.confirm.samples': 'The first items that go',
  'bulkDelete.confirm.trash':
    'They stay in the trash for {days} days. Until then, Undo puts them back with their media and files.',
  'bulkDelete.confirm.type': 'To confirm, type the number of items, {count}',
  'bulkDelete.confirm.moveOne': 'Move 1 item to the trash',
  'bulkDelete.confirm.moveMany': 'Move {count} items to the trash',
  'bulkDelete.empty.open': 'Empty the trash…',
  'bulkDelete.empty.ask':
    'Every item a bulk delete moved to the trash is deleted for good, with its media and files. Those changes can no longer be undone.',
  'bulkDelete.empty.confirm': 'Empty the trash',
  'bulkDelete.empty.cancel': 'Cancel',
  'bulkDelete.empty.doneOne': 'The trash is empty. 1 item was deleted for good.',
  'bulkDelete.empty.done': 'The trash is empty. {count} items were deleted for good.',
  'bulkDelete.empty.nothing': 'The trash was already empty.',
  'bulkDelete.item.checking': 'Checking what goes with this item…',
  'bulkDelete.item.cannotCheck': 'Cannot check what goes with this item, so it cannot move to the trash now.',
  'bulkDelete.item.checkAgain': 'Check again',
  'bulkDelete.item.denied': 'You may not delete this item.',
  'bulkDelete.item.ask': 'Move {name} to the trash?',
  'bulkDelete.item.linked': 'Other records link to it:',
  'bulkDelete.item.linksBack': 'Their links come back if you restore it.',
  'bulkDelete.item.notLinked': 'Nothing links to it.',
  'bulkDelete.item.trash':
    'The trash keeps it for {days} days. Until then, Undo on the Items screen puts it back with its media and files.',
  'bulkDelete.item.move': 'Move to the trash',
  'bulkDelete.item.moving': 'Moving to the trash…',
  'bulkDelete.item.failed': '{name} was not moved to the trash. It stays as it is.',
  'bulkDelete.item.trashed': 'Deleted {name}.',
  'bulkDelete.item.undo': 'Undo',
  'bulkDelete.item.undoing': 'Putting {name} back…',
  'bulkDelete.item.back': '{name} is back.',
  'bulkDelete.item.notBack': '{name} stays in the trash.',
} as const;

/** A complete table of this namespace, in one locale. */
export type BulkDeleteMessages = Table<keyof typeof en>;

export const bulkDelete = defineNamespace('bulk-delete', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
