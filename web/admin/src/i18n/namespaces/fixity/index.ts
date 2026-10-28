/**
 * The fixity check's messages: its start control on the Jobs screen, its result on the job
 * page, the dashboard line, and the Media list filter a count opens.
 *
 * English is here, in the chunk of the routes that import it; every other locale is its own
 * chunk under `locales/`. Each locale file is typed `FixityMessages`, so a key added here and
 * missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'fixity.class': 'Check stored files',
  'fixity.start': 'Check stored files',
  'fixity.start.note':
    'Reads every stored original and compares it with the SHA-256 recorded when the file was added.',
  'fixity.start.busy': 'Starting the check…',
  'fixity.start.failed': 'The check did not start: {message}',
  'fixity.result.title': 'Result',
  'fixity.result.loading': 'Loading the result…',
  'fixity.result.failed': 'The result could not be read: {message}',
  'fixity.result.waiting': 'The result shows here when the check ends.',
  'fixity.result.none': 'This check ended before it wrote a result. Its log says why.',
  'fixity.result.checkedOne': '1 file checked on {date}.',
  'fixity.result.checkedMany': '{count} files checked on {date}.',
  'fixity.result.differsOne': '1 differs.',
  'fixity.result.differsMany': '{count} differ.',
  'fixity.result.missingOne': '1 missing.',
  'fixity.result.missingMany': '{count} missing.',
  'fixity.result.unreadableOne': '1 cannot be read.',
  'fixity.result.unreadableMany': '{count} cannot be read.',
  'fixity.result.allMatch': 'All match.',
  'fixity.result.stopped': 'The check was stopped before the last file.',
  'fixity.line.problemsOne': 'Files last checked {date}: 1 problem',
  'fixity.line.problemsMany': 'Files last checked {date}: {count} problems',
  'fixity.line.noProblems': 'Files last checked {date}: no problems',
  'fixity.line.never': 'Stored files have never been checked.',
  'fixity.line.running': 'Checking stored files now.',
  'fixity.filter.checked': 'Files read by check #{job}.',
  'fixity.filter.ok': 'Files that matched in check #{job}.',
  'fixity.filter.differs': 'Files that differ in check #{job}.',
  'fixity.filter.missing': 'Files missing in check #{job}.',
  'fixity.filter.unreadable': 'Files that could not be read in check #{job}.',
  'fixity.filter.back': 'Open the check',
  'fixity.filter.clear': 'Show all media',
} as const;

/** A complete table of this namespace, in one locale. */
export type FixityMessages = Table<keyof typeof en>;

export const fixity = defineNamespace('fixity', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
