/**
 * The messages of the import's last screen: adding, stopping, the end, undo
 * and the undo report.
 *
 * A key under `advanced.` is shown in Advanced mode only; every other key may
 * show in Simple. `tests/import-glossary.test.ts` holds each to its word list.
 *
 * English is here, in the chunk of the screen that imports it; every other
 * locale is its own chunk under `locales/`. Each locale file is typed
 * `ImportRunMessages`, so a key added here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  // Counts that every part of the screen names. `What` lists join them.
  'run.count.items': '{count} items',
  'run.count.itemsOne': '1 item',
  'run.count.files': '{count} files',
  'run.count.filesOne': '1 file',
  'run.count.itemSets': '{count} item sets',
  'run.count.itemSetsOne': '1 item set',
  'run.count.itemTypes': '{count} item types',
  'run.count.itemTypesOne': '1 item type',
  'run.count.fields': '{count} fields',
  'run.count.fieldsOne': '1 field',
  'run.count.problems': '{count} problems',
  'run.count.problemsOne': '1 problem',
  'run.kind.item': 'Item {id}',
  'run.kind.media': 'File {id}',
  'run.kind.itemSet': 'Item set {id}',
  'run.kind.resourceTemplate': 'Item type {id}',
  'run.kind.property': 'Field {id}',
  // Before the start.
  'run.empty.title': 'Nothing to add yet',
  'run.empty.note': 'When your items are ready, you add them here. You can undo the whole import later.',
  'run.empty.link': 'Add your files',
  'run.ready.counting': 'Counting what goes into Omeka...',
  'run.ready.saving': 'Saving your last change. Then the counts are made again.',
  'run.ready.counted': 'Counted at {time}.',
  'run.ready.recount': 'Count again',
  'run.ready.must': '{what} must be fixed before you can add.',
  'run.ready.look': 'Look at them',
  'run.ready.nothing': 'Nothing goes into Omeka: every item is left out.',
  'run.ready.later': 'You can undo the whole import later.',
  'run.ready.add': 'Add {what} to Omeka',
  'run.ready.delete': 'Delete them from Omeka',
  'run.ready.starting': 'Starting...',
  'run.confirm.note': 'This deletes {count} from Omeka: items, files and item sets, counted together. It cannot be undone.',
  'run.confirm.label': 'Type {count} to confirm',
  // A start, stop or undo the server refused, with the next step.
  'run.no.changed': 'Your files or your answers changed after they were counted. The counts are new now: look at them, then add.',
  'run.no.blocked': 'This import cannot start: {what} must be fixed first.',
  'run.no.confirm': 'A delete needs its count typed: {count} items, files and item sets, counted together. Type it below.',
  'run.no.busy': 'Another import is being added or undone now. Only one works at a time. Try again when it has ended.',
  'run.no.busyOpen': 'Open that import',
  'run.no.busySelf': 'This import is still looking at its files. Try again in a moment.',
  'run.no.nothing': 'There is nothing to add: every item is left out.',
  'run.no.nothingLink': 'See what we found',
  'run.no.cannotRun': 'This import cannot be added now. It is: {status}. The page now shows where it stands.',
  'run.no.cannotUndo': 'This import cannot be undone now. It is: {status}. The page now shows where it stands.',
  'run.no.notRunning': 'This import is not working any more. The page now shows where it stands.',
  'run.no.again': 'Try again',
  // While it works.
  'run.work.adding': 'Writing to Omeka...',
  'run.work.undoing': 'Undoing...',
  'run.work.done': '{done} of {total} done.',
  'run.work.unit': 'Each item, each file and each item set counts as one.',
  'run.work.minutes': 'About {count} minutes left at this speed.',
  'run.work.minute': 'About 1 minute left at this speed.',
  'run.work.seconds': 'Less than a minute left at this speed.',
  'run.work.noTime': 'The time left shows when the first one is done.',
  'run.work.leave': 'You can leave this page. The import goes on, and this page shows where it stands when you come back.',
  'run.work.leaveUndo': 'You can leave this page. The undo goes on, and this page shows where it stands when you come back.',
  'run.work.stop': 'Stop after the one in hand',
  'run.work.stopNote': 'What is in Omeka now stays there. Then you can add the rest, or undo it.',
  'run.work.stopUndo': 'Stop the undo after the one in hand',
  'run.work.stopUndoNote': 'What is undone stays undone. Then you can undo the rest.',
  'run.work.stopping': 'Stopping after the one in hand...',
  'run.work.log': 'See the log on the Jobs page',
  // The end of a run.
  'run.end.done': 'The import is done.',
  'run.end.stopped': 'The import stopped before the end.',
  'run.end.asked': 'It stopped because someone asked it to.',
  'run.end.died': 'The work stopped without finishing. It was found stopped at {time}.',
  'run.end.failed': '{count} failed:',
  'run.end.failedOne': '1 failed:',
  'run.end.what': 'What',
  'run.end.why': 'Why',
  'run.end.failures': 'failed',
  'run.end.retry': 'Try again: the {count} not done yet',
  'run.end.retryNote': 'What is done already is skipped.',
  'run.end.left': 'Download the list of what was left out or failed (.csv)',
  'run.end.leftLoading': 'Downloading...',
  // Undo, before it starts.
  'run.undo.open': 'Undo this import...',
  'run.undo.reading': 'Reading what undo would do...',
  'run.undo.removes': 'Undo removes {what} that this import added.',
  'run.undo.restores': 'It puts {what} back as they were before this import changed them.',
  'run.undo.empty': 'Undo has nothing to remove or put back.',
  'run.undo.gone': '{count} are not in Omeka any more. Undo skips them.',
  'run.undo.goneOne': '1 is not in Omeka any more. Undo skips it.',
  'run.undo.kept': '{count} are kept: since the import, something in Omeka changed them or uses them. Each row says what. Undo keeps them, unless you choose “Undo it anyway”.',
  'run.undo.keptOne': '1 is kept: since the import, something in Omeka changed it or uses it. Its row says what. Undo keeps it, unless you choose “Undo it anyway”.',
  'run.undo.anyway': 'Undo it anyway',
  'run.undo.all': 'Undo all {count} anyway',
  'run.undo.list': 'kept',
  'run.undo.loss': 'Undoing {count} anyway also undoes what was done to them in Omeka after the import.',
  'run.undo.lossOne': 'Undoing 1 anyway also undoes what was done to it in Omeka after the import.',
  'run.undo.lossTypes': 'An item type or a field that is undone also loses its values on items outside this import.',
  'run.undo.go': 'Undo the import',
  'run.undo.cancel': 'Cancel',
  'run.undo.starting': 'Starting the undo...',
  // After an undo.
  'run.undone.done': 'Undone.',
  'run.undone.partly': 'Partly undone.',
  'run.undone.failed': 'The undo stopped before the end.',
  'run.undone.removed': 'Removed {what}.',
  'run.undone.restored': 'Put back {what}.',
  'run.undone.anyway': '{count} were undone anyway, although something in Omeka changed them or used them after the import.',
  'run.undone.anywayOne': '1 was undone anyway, although something in Omeka changed it or used it after the import.',
  'run.undone.gone': '{count} were not in Omeka any more.',
  'run.undone.goneOne': '1 was not in Omeka any more.',
  'run.undone.asked': 'It stopped because someone asked it to, after {done} of {total}.',
  'run.undone.error': 'It stopped after {done} of {total}: {reason}',
  'run.undone.kept': '{count} were kept: since the import, something in Omeka changed them or uses them.',
  'run.undone.keptOne': '1 was kept: since the import, something in Omeka changed it or uses it.',
  'run.undone.rest': 'Undo the rest',
  // Advanced only.
  'advanced.runs.counts': 'What the run writes, by record kind and action.',
  'advanced.runs.kind': 'Record kind',
  'advanced.runs.does': 'Does',
  'advanced.runs.count': 'Count',
  'advanced.runs.leave': 'left out',
  'advanced.runs.go': 'Run the import',
} as const;

/** A complete table of this namespace, in one locale. */
export type ImportRunMessages = Table<keyof typeof en>;

export const importRun = defineNamespace('import-run', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
