/**
 * The words of the corrections the Structure screen offers on a folder
 * ("these files are one item", "this row is that file", "this row describes
 * nothing"): each offer's count, what its click changes, each instance as
 * facts, and the same instances under their change in the changes strip.
 *
 * The offers are a lazy chunk of their own (`pages/import/simple/structure-fixes.tsx`),
 * and so are these words: the Structure screen's own chunk does not carry
 * them. Every key shows in Simple, so `tests/import-glossary.test.ts` holds
 * each to Simple's banned list and `tests/import-copy-length.test.tsx` to 12
 * words. Each locale file is typed `ImportFixesMessages`, so a key added here
 * and missing there is a `tsc` error.
 */
import { defineNamespace } from '../../namespace';
import type { Table } from '../../namespace';

export const en = {
  'fix.q.items': 'Offer: items',
  'fix.q.rows': 'Offer: rows',
  'fix.oneItem': '{count} items could go into another item.',
  'fix.oneItemOne': '1 item could go into another item.',
  'fix.rowIs': '{count} rows name no file.',
  'fix.rowIsOne': '1 row names no file.',
  'fix.rowIs.note': 'Each points at one file that has no row.',
  'fix.rowIs.noteOne': 'It points at one file that has no row.',
  'fix.describesNothing': '{count} rows name no file, and no file is near.',
  'fix.describesNothingOne': '1 row names no file, and no file is near.',
  'fix.describesNothing.note': 'Each makes an item with no file.',
  'fix.describesNothing.noteOne': 'It makes an item with no file.',
  'fix.items': 'Items: {before} → {after}',
  'fix.rows': 'Rows that find a file: {before} → {after} of {of}',
  'fix.show': 'Show the {count}',
  'fix.showOne': 'Show it',
  'fix.oneItem.take': 'Put these {count} into their item',
  'fix.oneItem.takeOne': 'Put it into its item',
  'fix.rowIs.take': 'Pair these {count}',
  'fix.rowIs.takeOne': 'Pair this row',
  'fix.describesNothing.take': 'Leave these {count} out',
  'fix.describesNothing.takeOne': 'Leave this row out',
  'fix.join': '{with} goes into the item of {item}',
  'fix.pair': 'Row {line}, {cell}: file {file}',
  'fix.leave': 'Row {line}, {cell}',
  'fix.because.keyExtends': 'Its name starts with the other item’s name.',
  'fix.because.rowNamesBoth': 'One row names both.',
  'fix.because.near': 'The names differ by one character.',
  'fix.because.namesItem': 'The row names the item of this file.',
  'fix.because.namesPart': 'The row names this file’s part of its item.',
  'fix.joined': 'Into another item: {count}',
  'fix.paired': 'Paired: {count}',
  'fix.left': 'Left out: {count}',
  'fix.stale': '{count} no longer apply.',
  'fix.staleOne': '1 no longer applies.',
  'fix.rowChanged': 'This row changed.',
  'fix.fileGone': 'This file is gone.',
} as const;

/** A complete table of this namespace, in one locale. */
export type ImportFixesMessages = Table<keyof typeof en>;

export const importFixes = defineNamespace('import-fixes', en, {
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
});
