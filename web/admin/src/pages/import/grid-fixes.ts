/**
 * The fixes the grid offers per problem kind, as a table. A fix opens the
 * fill panel, changes how the spreadsheet is read, or changes the plan for a
 * set of rows: every row of the kind (a bulk fix, after reading their
 * identifiers from the server) or the one row it sits on.
 */
import type { ProblemKind } from '../../api/import-preview';
import type { ImportPlan } from '../../api/imports';
import type { ImportMessageKey } from './messages';
import { withCsv, withCsvRows, withSkipped, withoutFiles } from './plan-changes';

export type Fix =
  | { readonly do: 'fill'; readonly label: ImportMessageKey }
  | { readonly do: 'sheet'; readonly label: ImportMessageKey; readonly change: (plan: ImportPlan) => ImportPlan }
  | {
      readonly do: 'rows';
      readonly label: ImportMessageKey;
      readonly change: (plan: ImportPlan, identifiers: readonly string[]) => ImportPlan;
    };

const keep: Fix = {
  do: 'rows',
  label: 'fix.keep',
  change: (plan, identifiers) => withCsvRows(plan, identifiers, 'keep'),
};
const ignore: Fix = {
  do: 'rows',
  label: 'fix.ignore',
  change: (plan, identifiers) => withCsvRows(plan, identifiers, 'ignore'),
};

export const SKIP: Fix = { do: 'rows', label: 'fix.skip', change: withSkipped };

/** The fixes of a kind, for every row that has it. Skip them is offered for every kind besides. */
export const KIND_FIXES: Readonly<Record<ProblemKind, readonly Fix[]>> = {
  empty: [{ do: 'fill', label: 'fix.fill' }],
  date: [],
  space: [{ do: 'sheet', label: 'fix.trim', change: (plan) => withCsv(plan, (csv) => ({ ...csv, trim: true })) }],
  encoding: [
    {
      do: 'sheet',
      label: 'fix.windows1252',
      change: (plan) => withCsv(plan, (csv) => ({ ...csv, windows1252: true })),
    },
  ],
  cells: [keep, ignore],
  nofile: [{ do: 'rows', label: 'fix.withoutFiles', change: withoutFiles }],
};

/** The fixes of a row problem, for the row it sits on. */
export const ROW_FIXES: Readonly<Record<'cells' | 'nofile', readonly Fix[]>> = {
  cells: [keep, ignore],
  nofile: [{ do: 'rows', label: 'fix.withoutFile', change: withoutFiles }],
};

/** A fix that changes how the spreadsheet is read means nothing without one. */
export const offered = (fixes: readonly Fix[], plan: ImportPlan): readonly Fix[] =>
  fixes.filter((fix) => fix.do !== 'sheet' || plan.csv !== null);
