/**
 * The Import page messages.
 *
 * A key under `advanced.` is shown in Advanced mode only; every other key may
 * show in Simple. `tests/import-glossary.test.ts` holds each to its word list.
 *
 * English is here, in the chunk of the route that imports it; every other
 * locale is its own chunk under `locales/`, fetched the first time the route
 * in that locale reads it. Each locale file is typed `ImportMessages`, so a key
 * added here and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  // The start page: where the files are, and the past imports.
  'home.lede':
    'Add a folder of files to Omeka as items. Nothing goes into Omeka until the last step, and you can undo an import later.',
  'home.where': 'Where are your files?',
  'home.server.title': 'In a folder on the server',
  'home.server.note': 'For folders that the person who runs your Omeka has put there.',
  'home.server.none':
    'No folder on the server is set up for importing. The person who runs your Omeka can set one with ROMEKA_ARCHIVE_ROOTS.',
  'home.server.roots': 'Folders on the server',
  'home.server.loading': 'Reading the folder...',
  'home.server.path': 'Folder',
  'home.server.files': 'This folder holds {count} files, not counting the folders inside it.',
  'home.server.filesOne': 'This folder holds 1 file, not counting the folders inside it.',
  'home.server.noFolders': 'There are no folders inside this one.',
  'home.server.folders': 'folders',
  'home.server.use': 'Use the folder {folder}',
  'home.server.useAll': 'Use everything in {root}',
  'home.server.starting': 'Starting...',
  'home.server.top': 'Go to the top folder',
  'home.computer.title': 'On this computer',
  'home.computer.later':
    'Sending files from this computer comes in a later version. For now, ask the person who runs your Omeka to put your folder on the server.',
  'home.past.title': 'Past imports',
  'home.past.empty': 'No past imports yet. Choose a folder above to start one.',
  'home.past.resource': 'imports',
  'home.past.colFiles': 'Files',
  'home.past.colStatus': 'Status',
  'home.past.colStarted': 'Started',
  'home.past.colFinished': 'Finished',
  'home.past.colAdded': 'Added',
  'home.past.colMore': 'More',
  'home.past.added': '{done} of {total}',
  'home.past.copy': 'Make a copy',
  'home.past.copying': 'Copying...',
  'home.past.retry': 'Try again',
  // Words for an import that every screen shares.
  'words.folder': '{path} ({root})',
  'words.root': 'All of {root}',
  'words.computer': 'Files from this computer',
  'words.none': 'No files yet',
  'status.draft': 'Not added yet',
  'status.running': 'Adding',
  'status.stopping': 'Stopping',
  'status.done': 'Added',
  'status.run_failed': 'Stopped before the end',
  'status.undoing': 'Undoing',
  'status.undone': 'Undone',
  'status.partly_undone': 'Partly undone: some items were kept',
  'status.undo_failed': 'Undo stopped before the end',
  // The frame around one import: mode, steps, saving.
  'shell.loading': 'Loading the import...',
  'shell.badId': '"{param}" is not the number of an import.',
  'shell.badScreen': 'This import has no page with that name.',
  'shell.fromStart': 'Open the import from the start',
  'shell.home': 'All imports',
  'shell.mode': 'Mode',
  'shell.simple': 'Simple',
  'shell.advanced': 'Advanced',
  'shell.simpleNote': 'One question at a time, with suggested answers.',
  'shell.advancedNote': 'Every setting at once. Your answers in Simple are the same settings.',
  'shell.steps': 'Steps',
  'shell.settings': 'Settings',
  'shell.back': 'Back',
  'shell.next': 'Next',
  'shell.saved': 'Saved',
  'shell.saving': 'Saving...',
  'shell.notSaved': 'Not saved',
  'shell.stale':
    'This import was saved somewhere else after you opened it, perhaps in another tab. Your changes since then are not saved.',
  'shell.staleLoad': 'Load the saved answers',
  'shell.staleNote': 'This drops your changes since then.',
  'shell.failed': 'Your last change is not saved.',
  'shell.retry': 'Try again',
  'shell.closed': 'This import has started, so its answers cannot change. To change them, make a copy.',
  'shell.copy': 'Make a copy',
  'shell.delete': 'Delete this import',
  // The status line over each screen.
  'line.items': '{count} items so far.',
  'line.itemsOne': '1 item so far.',
  'line.left': '{count} left out.',
  'line.look': '{count} kinds of problem to look at',
  'line.lookOne': '1 kind of problem to look at',
  'line.nothing': 'Nothing needs a look.',
  'line.stops': '{count} must be fixed before you can add.',
  'line.notYet': 'Nothing is in Omeka until you add it.',
  'line.unread': 'The counts could not be read: {message}',
  'advanced.line': '{items} items · {left} left out · {changes} changes · action {action}',
  // Screen headings and step names. A key under `advanced.` is shown in Advanced only.
  'simple.files.title': 'Where are your files?',
  'simple.files.step': 'Your files',
  'simple.found.title': 'What we found in your files',
  'simple.found.step': 'What we found',
  'simple.example.title': 'Show us one example',
  'simple.allFiles.title': 'Every file',
  'simple.list.title': 'Do you have a list that describes them?',
  'simple.list.step': 'Your list',
  'simple.pair.title': 'Pair rows and files by hand',
  'simple.where.title': 'Where should they go in Omeka?',
  'simple.where.step': 'Where they go',
  'simple.check.title': 'Check',
  'simple.check.step': 'Check',
  'simple.allItems.title': 'Every item',
  'simple.add.title': 'Add them',
  'simple.add.step': 'Add',
  'advanced.files.title': 'Sources, pattern and files',
  'advanced.files.step': 'Files',
  'advanced.lists.title': 'Lists: reading, join and columns',
  'advanced.lists.step': 'Lists',
  'advanced.fields.title': 'Defaults and field rules',
  'advanced.fields.step': 'Fields',
  'advanced.problems.title': 'What to do with each kind of problem',
  'advanced.problems.step': 'Problems',
  'advanced.existing.title': 'Action and match',
  'advanced.existing.step': 'Existing items',
  'advanced.changes.title': 'Every change you made',
  'advanced.changes.step': 'Changes',
  'advanced.items.title': 'Every item',
  'advanced.items.step': 'Items',
  'advanced.run.title': 'Run the import',
  'advanced.run.step': 'Run',
  'advanced.allFiles.title': 'Every file',
  'advanced.pair.title': 'Pair rows and files by hand',
  // The shared paged list and its filters.
  'lists.loading': 'Loading page {page}...',
  'lists.search': 'Search',
  'lists.showAll': 'Show all',
} as const;

/** A complete table of this namespace, in one locale. */
export type ImportMessages = Table<keyof typeof en>;

export const archiveImport = defineNamespace('import', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
