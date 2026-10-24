/**
 * The Items list's bulk change panel messages.
 *
 * English is here, in the chunk the Items list loads when the panel opens; every other locale is
 * its own chunk under `locales/`, fetched the first time the panel reads it in that locale. Each
 * locale file is typed `BulkMessages`, so a key added here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'bulk.panel.label': 'Change items',
  'bulk.panel.close': 'Close',
  // Which items.
  'bulk.scope.legend': 'Which items',
  'bulk.scope.tickedOne': 'The ticked item',
  'bulk.scope.tickedMany': 'The {count} ticked items',
  'bulk.scope.matchingOne': 'The 1 item that matches',
  'bulk.scope.matchingMany': 'All {count} items that match',
  'bulk.scope.allOne': 'The 1 item',
  'bulk.scope.allMany': 'All {count} items',
  'bulk.scope.matchingRefused': 'Your role can change ticked items only. Tick the items to change.',
  // The fields.
  'bulk.field.visibility': 'Visibility',
  'bulk.field.owner': 'Owner',
  'bulk.field.class': 'Class',
  'bulk.field.template': 'Item type',
  'bulk.field.addSet': 'Add to item set',
  'bulk.field.removeSet': 'Remove from item set',
  'bulk.field.addSite': 'Add to site',
  'bulk.field.removeSite': 'Remove from site',
  'bulk.choice.keep': 'No change',
  'bulk.choice.public': 'Public',
  'bulk.choice.private': 'Private',
  'bulk.choice.noClass': 'No class',
  'bulk.choice.noTemplate': 'No item type',
  'bulk.edit.nothing': 'Choose at least one change.',
  'bulk.edit.preview': 'Preview',
  'bulk.edit.cancel': 'Cancel',
  // The preview.
  'bulk.preview.changeOne': '1 item will change.',
  'bulk.preview.changeMany': '{count} items will change.',
  'bulk.preview.changeNone': 'No item will change.',
  'bulk.preview.unchangedOne': '1 already matches, no change.',
  'bulk.preview.unchanged': '{count} already match, no change.',
  'bulk.preview.denied': '{count} you may not change.',
  'bulk.preview.samples': 'Examples of the change',
  'bulk.preview.item': 'Item',
  'bulk.preview.before': 'Before',
  'bulk.preview.after': 'After',
  'bulk.preview.apply': 'Apply',
  'bulk.preview.back': 'Back',
  'bulk.preview.expired': 'This preview is more than ten minutes old, so the items may have changed since. Preview the change again.',
  'bulk.preview.again': 'Preview again',
  // What a field reads as, before and after.
  'bulk.word.public': 'Public',
  'bulk.word.private': 'Private',
  'bulk.word.owner': 'Owner: {name}',
  'bulk.word.noOwner': 'No owner',
  'bulk.word.class': 'Class: {name}',
  'bulk.word.noClass': 'No class',
  'bulk.word.template': 'Item type: {name}',
  'bulk.word.noTemplate': 'No item type',
  'bulk.word.sets': 'Item sets: {names}',
  'bulk.word.noSets': 'In no item set',
  'bulk.word.sites': 'Sites: {names}',
  'bulk.word.noSites': 'In no site',
  'bulk.word.addSets': 'Add to {names}',
  'bulk.word.removeSets': 'Remove from {names}',
  'bulk.word.addSites': 'Add to site {names}',
  'bulk.word.removeSites': 'Remove from site {names}',
  'bulk.word.untitled': 'Item {id}',
  // Running.
  'bulk.run.progress': 'Changing items: {done} of {total}',
  'bulk.run.undoProgress': 'Undoing: {done} of {total}',
  'bulk.run.stop': 'Stop',
  'bulk.run.stopping': 'Stopping…',
  'bulk.run.leave': 'You can leave this page; the change keeps running.',
  // Ended.
  'bulk.done.changedOne': '1 item changed.',
  'bulk.done.changedMany': '{count} items changed.',
  'bulk.done.unchangedOne': '1 already matched, no change.',
  'bulk.done.unchanged': '{count} already matched, no change.',
  'bulk.done.failed': '{count} could not be changed:',
  'bulk.done.stopped': 'Stopped. {done} of {total} items changed.',
  'bulk.done.error': 'The change stopped on an error: {error}',
  'bulk.done.goOn': 'Go on',
  'bulk.done.undo': 'Undo',
  'bulk.done.undoneOne': 'Undone. 1 item is back as it was.',
  'bulk.done.undone': 'Undone. {count} items are back as they were.',
  'bulk.done.partlyUndone': 'Back as they were: {count}. Edited after the change, so left as they are: {kept}.',
  'bulk.done.undoAgain': 'Undo the rest',
  // Undo.
  'bulk.undo.restoreOne': 'Undo puts 1 item back as it was before this change.',
  'bulk.undo.restore': 'Undo puts {count} items back as they were before this change.',
  'bulk.undo.editedOne': '1 item was edited after this change. Undo leaves it as it is:',
  'bulk.undo.edited': '{count} items were edited after this change. Undo leaves them as they are:',
  'bulk.undo.force': 'Restore these too',
  'bulk.undo.goneOne': '1 item is no longer in Omeka.',
  'bulk.undo.gone': '{count} items are no longer in Omeka.',
  'bulk.undo.confirm': 'Undo the change',
  'bulk.undo.back': 'Back',
  // Refusals.
  'bulk.refused.denied': 'You may not make this change. Ask an administrator.',
  // Recent changes.
  'bulk.recent.heading': 'Recent bulk changes',
  'bulk.recent.loading': 'Loading recent changes…',
  'bulk.recent.none': 'No bulk changes yet.',
  'bulk.recent.when': 'When',
  'bulk.recent.change': 'Change',
  'bulk.recent.status': 'Status',
  'bulk.recent.items': 'Items',
  'bulk.recent.count': '{done} of {total}',
  'bulk.recent.show': 'Show',
  'bulk.status.previewed': 'Not applied',
  'bulk.status.running': 'Running',
  'bulk.status.stopping': 'Stopping',
  'bulk.status.stopped': 'Stopped',
  'bulk.status.done': 'Done',
  'bulk.status.failed': 'Stopped on an error',
  'bulk.status.undoing': 'Undoing',
  'bulk.status.undo_stopping': 'Stopping the undo',
  'bulk.status.undone': 'Undone',
  'bulk.status.partly_undone': 'Partly undone',
} as const;

/** A complete table of this namespace, in one locale. */
export type BulkMessages = Table<keyof typeof en>;

export const bulk = defineNamespace('bulk', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
