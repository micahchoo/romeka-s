/**
 * A list sent again in place of one the person uploaded (`?replace=`). The
 * server keeps the list's choices and reads each column by its place, so the
 * admin first matches the new file's header row to the old list's columns by
 * their text. A column that moved is followed; a column added is fine; a
 * column the plan's choices read that the new file lacks refuses the file
 * before it is sent.
 */
import type { ColumnRef, ImportPlan, SheetPlan, SlotRule, SlotRules, SourceRef } from '../../../api/import-plan';
import { readCsv } from '../csv-records';
import { allSeries, listOf, setsOf, withEverySeries, withList, withSets } from '../series';

/** The header row of a CSV or TSV file, read as the old list read its file. */
export const headerOf = async (file: File, read: SheetPlan['read']): Promise<readonly string[]> => {
  const delimiter = read.delimiter === 'auto' ? (/\.tsv$/i.test(file.name) ? '\t' : ',') : read.delimiter;
  const { records } = readCsv(await file.text(), delimiter, read.headerLine);
  const cells = records[read.headerLine - 1]?.cells ?? [];
  return read.trim ? cells.map((cell) => cell.trim()) : cells;
};

const readsSheet = (source: SourceRef, sheet: string): source is Extract<SourceRef, { from: 'column' }> => source.from === 'column' && source.sheet === sheet;

/** The rules of every series and the free sets that read the list. */
const rulesOf = (plan: ImportPlan): readonly SlotRule[] => [
  ...allSeries(plan).flatMap((series) => [series.slots.item, series.slots.media, series.slots.itemSet].flatMap((rules) => Object.values<SlotRule | undefined>(rules))),
  ...setsOf(plan).flatMap((set) => ('each' in set ? [set.each] : [])),
].flatMap((rule) => (rule === undefined ? [] : [rule]));

/** The list's own uses of its columns: what says a row's kind, the column that pairs it, and its item key. */
const listUses = (list: SheetPlan): readonly ColumnRef[] => [
  ...(typeof list.rowKind === 'object' ? [list.rowKind.column] : []),
  ...(list.join === null ? [] : [list.join.column]),
  ...(list.itemKey === null ? [] : [list.itemKey]),
];

/** A column the plan's choices read that the new file lacks, with how many choices read it. */
export interface Lacking {
  readonly column: string;
  readonly uses: number;
}

export type SendAgain = { readonly lacking: Lacking } | { readonly follow: (plan: ImportPlan) => ImportPlan };

/**
 * Whether the new header can take the place of list `sheet`: the first
 * column its choices read and the header lacks, or the edit that moves each
 * column reference of the list to its place in the new file.
 */
export const sendAgain = (plan: ImportPlan, sheet: string, header: readonly string[]): SendAgain => {
  const list = listOf(plan, sheet);
  if (list === undefined) {
    return { follow: (same) => same };
  }
  const uses = new Map<string, number>();
  const use = (column: ColumnRef): void => {
    uses.set(column.header, (uses.get(column.header) ?? 0) + 1);
  };
  listUses(list).forEach(use);
  rulesOf(plan).forEach((rule) => {
    rule.sources.filter((source) => readsSheet(source, sheet)).forEach((source) => {
      use(source.column);
    });
  });
  const lacking = [...uses].find(([column]) => !header.includes(column));
  if (lacking !== undefined) {
    return { lacking: { column: lacking[0], uses: lacking[1] } };
  }
  const moved = (column: ColumnRef): ColumnRef => ({ ...column, index: header.indexOf(column.header) });
  const inRule = (rule: SlotRule): SlotRule => ({ ...rule, sources: rule.sources.map((source) => (readsSheet(source, sheet) ? { ...source, column: moved(source.column) } : source)) });
  const inRules = (rules: SlotRules): SlotRules =>
    Object.fromEntries(Object.entries<SlotRule | undefined>(rules).flatMap(([slot, rule]) => (rule === undefined ? [] : [[slot, inRule(rule)]])));
  return {
    follow: (before) =>
      withSets(
        withEverySeries(
          withList(before, sheet, (one) => ({
            ...one,
            rowKind: typeof one.rowKind === 'object' ? { column: moved(one.rowKind.column) } : one.rowKind,
            join: one.join === null ? null : { ...one.join, column: moved(one.join.column) },
            itemKey: one.itemKey === null ? null : moved(one.itemKey),
            // Options of a column the new file lacks go: nothing reads it, and its place now holds another column.
            columns: one.columns.filter((column) => header.includes(column.column.header)).map((column) => ({ ...column, column: moved(column.column) })),
          })),
          (series) => ({ ...series, slots: { item: inRules(series.slots.item), media: inRules(series.slots.media), itemSet: inRules(series.slots.itemSet) } }),
        ),
        (sets) => sets.map((set) => ('each' in set ? { each: { ...set.each, ...inRule(set.each) } } : set)),
      ),
  };
};
