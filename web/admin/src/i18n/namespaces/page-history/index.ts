/**
 * The page editor's History: the list of saved versions, what each changed, the version view and
 * the dialog a refused save opens.
 *
 * English is here, in the chunks History and the refused-save dialog load when they open; every
 * other locale is its own chunk under `locales/`. Each locale file is typed `PageHistoryMessages`,
 * so a key added here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'history.title': 'Page history',
  'history.close': 'Close',
  'history.loading': 'Loading the versions...',
  'history.empty': 'No versions yet. Each save keeps one.',
  'history.entry': '{time} · {name} · {blocks}',
  'history.today': 'Today {time}',
  'history.notRecorded': 'Author not recorded',
  'history.deletedUser': 'Deleted user',
  'history.firstVersion': 'First saved version',
  'history.created': 'Created',
  'history.olderGone': 'Older versions are no longer kept',
  'history.noChanges': 'No changes',
  'history.retitled': 'Title changed from “{before}” to “{after}”',
  'history.settingsChanged': 'Page settings changed',
  'history.addedOne': '1 block added',
  'history.added': '{count} blocks added',
  'history.removedOne': '1 block removed',
  'history.removed': '{count} blocks removed',
  'history.changedOne': '1 block changed',
  'history.changed': '{count} blocks changed',
  'history.changedNamedOne': '1 block changed: {names}',
  'history.changedNamed': '{count} blocks changed: {names}',
  'history.movedOne': '1 block moved',
  'history.moved': '{count} blocks moved',
  'history.live': 'Live now',
  'history.viewing': 'Viewing the version from {time}',
  'history.restore': 'Restore this version',
  'history.back': 'Back to the current draft',
  'history.rendering': 'Rendering this version...',
  'history.renderFailed': 'This version could not be rendered.',
  'history.conflictTitle': 'Someone else saved this page',
  'history.conflict': 'Saved by {name} at {time} since you opened this page',
  'history.conflictUntimed': 'Saved by {name} since you opened this page',
  'history.conflictChoices':
    'Their version opens in the editor, and yours stays one Undo away. Keep mine saves yours over theirs.',
  'history.someoneElse': 'someone else',
  'history.showTheirs': 'Show their version',
  'history.keepMine': 'Keep mine',
} as const;

/** A complete table of this namespace, in one locale. */
export type PageHistoryMessages = Table<keyof typeof en>;

export const pageHistory = defineNamespace('page-history', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
