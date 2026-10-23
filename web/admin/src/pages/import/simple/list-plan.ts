/**
 * The plan edits "Your list" makes. A list found in a folder becomes a sheet
 * of the plan read with automatic options; its pairing column is the one with
 * the most cells that exactly equal an item's code or a file's name, a count
 * the screen shows; and a column goes into a field only when its header is
 * that field's label (compared as titles). Every other column waits for the
 * person.
 */
import type { ColumnCells, Grain, SheetColumnMatches, Varies } from '../../../api/import-sections';
import type { SheetRead, SheetView } from '../../../api/import-sheet-views';
import type { ColumnRef, ImportPlan, ListFile, RecordKind, SheetPlan, SlotName, SlotRule, SlotRules, Slots, SourceRef, Term } from '../../../api/import-plan';
import type { ColumnRollUp, Proposal } from '../../../api/import-proposal';
import { isTerm } from '../../../api/import-plan';
import type { CsvRead } from '../csv-records';
import { readCsv } from '../csv-records';
import { allLists, listOf, seriesHolding, slotsOf, withList, withSlots } from '../series';

/**
 * Where a column goes: nowhere, a field, the item type, or a new field. Never
 * an item set: sets are a folder's Set and the archive's own sets
 * (`advanced/free-sets.tsx`), and a column becomes sets only through them.
 */
export type Target = '' | `field:${Term}` | 'template' | `new:${string}`;

export const fold = (text: string): string => text.trim().normalize('NFC').toLowerCase();

/** A sheet id no series of the plan has: the server refuses two that are equal anywhere in the plan. */
const nextId = (plan: ImportPlan): string => {
  const taken = new Set(allLists(plan).map((sheet) => sheet.id));
  let n = 1;
  while (taken.has(`folder-list-${String(n)}`)) {
    n += 1;
  }
  return `folder-list-${String(n)}`;
};

/** A list file of a folder, read with automatic options and no column yet. */
export const sheetOf = (plan: ImportPlan, source: string, file: string): SheetPlan => ({
  id: nextId(plan),
  source,
  file,
  hash: null,
  read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
  rowKind: 'item',
  join: null,
  itemKey: null,
  columns: [],
});

const best = (matches: SheetColumnMatches | undefined): SheetColumnMatches['columns'][number] | undefined =>
  matches?.columns.reduce<SheetColumnMatches['columns'][number] | undefined>(
    (top, column) => (top === undefined || Math.max(column.groupKeys, column.fileNames) > Math.max(top.groupKeys, top.fileNames) ? column : top),
    undefined,
  );

const joinOn = (column: SheetColumnMatches['columns'][number]): SheetPlan['join'] => ({
  on: column.fileNames >= column.groupKeys ? 'fileName' : 'identifier',
  column: { index: column.index, header: column.header },
  normalize: { caseFold: false, leadingZeros: false, separators: false },
});

export const withJoin = (plan: ImportPlan, sheet: string, column: SheetColumnMatches['columns'][number]): ImportPlan =>
  withList(plan, sheet, (one) => ({ ...one, join: joinOn(column) }));

/** Whether the sheet pairs with every name fold on, which Simple turns on and off as one. */
export const normalizedOf = (sheet: SheetPlan): boolean =>
  sheet.join !== null && sheet.join.normalize.caseFold && sheet.join.normalize.leadingZeros && sheet.join.normalize.separators;

/** Every name fold of the sheet's pairing on, or every one off; a sheet that pairs by no column stays as it is. */
export const withNormalize = (plan: ImportPlan, sheet: string, on: boolean): ImportPlan =>
  withList(plan, sheet, (one) => (one.join === null ? one : { ...one, join: { ...one.join, normalize: { caseFold: on, leadingZeros: on, separators: on } } }));

const readsColumn = (source: SourceRef, sheet: string, index: number): boolean => source.from === 'column' && source.sheet === sheet && source.column.index === index;

/** The rules without the column; a free set that reads it (`slotsOf`'s `sets`) stays, as no column target writes one. */
const rulesWithout = (rules: SlotRules, sheet: string, index: number): SlotRules => {
  const next: { [slot: string]: SlotRule } = {};
  for (const [slot, rule] of Object.entries<SlotRule | undefined>(rules)) {
    if (rule !== undefined) {
      next[slot] = slot === 'sets' ? rule : { ...rule, sources: rule.sources.filter((source) => !readsColumn(source, sheet, index)) };
    }
  }
  return next;
};

/**
 * Where a list column's values go when its rows describe files: on the item
 * (every row's values, each once), on each file, or nowhere. It is read from
 * where the column's sources are, never stored: `slots.item` is the item,
 * `slots.media` each file.
 */
export type Level = 'item' | 'file' | 'out';

const KIND: Readonly<Record<Exclude<Level, 'out'>, 'item' | 'media'>> = { item: 'item', file: 'media' };

/** A slot only an item has: a column feeding one cannot go on each file. */
export const itemOnly = (slot: string): boolean => slot === 'sets' || slot === 'template';

interface Place {
  readonly kind: 'item' | 'media';
  readonly slot: string;
  readonly rule: SlotRule;
}

/** The rules of the series that holds `sheet`, the only ones that may read its columns; none for a list no series holds. */
const rulesFor = (plan: ImportPlan, sheet: string): Slots | null => {
  const holder = seriesHolding(plan, sheet);
  return holder === undefined ? null : slotsOf(plan, holder.id);
};

/** Every item and media rule that reads the column, items first. */
const placesOf = (plan: ImportPlan, sheet: string, index: number): readonly Place[] => {
  const rules = rulesFor(plan, sheet);
  return rules === null
    ? []
    : (['item', 'media'] as const).flatMap((kind) =>
        Object.entries<SlotRule | undefined>(rules[kind]).flatMap(([slot, rule]) =>
          rule?.sources.some((source) => readsColumn(source, sheet, index)) === true ? [{ kind, slot, rule }] : [],
        ),
      );
};

export const levelOf = (plan: ImportPlan, sheet: string, index: number): Level => {
  const places = placesOf(plan, sheet, index);
  if (places.some((place) => place.kind === 'item')) {
    return 'item';
  }
  return places.length === 0 ? 'out' : 'file';
};

/** Whether the column may go on each file: no slot it feeds is an item's alone. */
export const fileLevelAllowed = (plan: ImportPlan, sheet: string, index: number): boolean =>
  !placesOf(plan, sheet, index).some((place) => itemOnly(place.slot));

/**
 * The column's sources moved to `level`, each into the same slot with its
 * rule's combine and value map, ahead of the slot's other sources; `out`
 * removes them. A rule left with no source goes too, unless `out` leaves it
 * a value map. A column feeding `sets` or `template` cannot go on each file,
 * so that edit leaves the plan as it is.
 */
export const withColumnLevel = (plan: ImportPlan, sheet: string, column: ColumnRef, level: Level): ImportPlan => {
  const holder = seriesHolding(plan, sheet);
  if (holder === undefined || (level === 'file' && !fileLevelAllowed(plan, sheet, column.index))) {
    return plan;
  }
  const places = placesOf(plan, sheet, column.index);
  const rules = slotsOf(plan, holder.id);
  const without = (kind: 'item' | 'media'): SlotRules => {
    const next: { [slot: string]: SlotRule } = {};
    for (const [slot, rule] of Object.entries<SlotRule | undefined>(rules[kind])) {
      if (rule === undefined) {
        continue;
      }
      const sources = rule.sources.filter((source) => !readsColumn(source, sheet, column.index));
      const lostItsLast = sources.length === 0 && rule.sources.length > 0;
      if (!lostItsLast || (level === 'out' && rule.map !== undefined)) {
        next[slot] = { ...rule, sources };
      }
    }
    return next;
  };
  const slots = { ...rules, item: without('item'), media: without('media') };
  if (level === 'out') {
    return withSlots(plan, holder.id, slots);
  }
  const kind = KIND[level];
  const into: { [slot: string]: SlotRule | undefined } = { ...slots[kind] };
  const source: SourceRef = { from: 'column', sheet, column };
  for (const place of places) {
    const there = into[place.slot];
    into[place.slot] = there === undefined ? { ...place.rule, sources: [source] } : { ...there, sources: [source, ...there.sources.filter((one) => !readsColumn(one, sheet, column.index))] };
  }
  return withSlots(plan, holder.id, { ...slots, [kind]: into });
};

/** The columns of `sheet` that some rule of `kinds` reads. */
const columnsIn = (plan: ImportPlan, sheet: string, kinds: readonly ('item' | 'media')[]): readonly ColumnRef[] => {
  const found = new Map<number, ColumnRef>();
  const rules = rulesFor(plan, sheet);
  for (const rule of rules === null ? [] : kinds.flatMap((kind) => Object.values<SlotRule | undefined>(rules[kind]))) {
    for (const source of rule?.sources ?? []) {
      if (source.from === 'column' && source.sheet === sheet) {
        found.set(source.column.index, source.column);
      }
    }
  }
  return [...found.values()];
};

/**
 * What each row of the list describes. Rows that describe files find their
 * file by name, so `media` joins by file name (same column, same folds);
 * any other kind brings the columns it put on each file back to the item,
 * where its rows can give them values.
 */
export const withRowKind = (plan: ImportPlan, sheet: string, kind: RecordKind): ImportPlan => {
  const next = withList(plan, sheet, (one) => ({ ...one, rowKind: kind, join: kind === 'media' && one.join !== null ? { ...one.join, on: 'fileName' as const } : one.join }));
  return kind === 'media' ? next : columnsIn(next, sheet, ['media']).reduce((edited, column) => withColumnLevel(edited, sheet, column, 'item'), next);
};

export const variesOf = (grain: Grain | null, index: number): Varies | null => grain?.columns.find((column) => column.index === index)?.varies ?? null;

/** The proposer's roll-up pick for the column, matched by its place and its header. */
export const rollUpOf = (rollUps: readonly ColumnRollUp[], column: ColumnRef): ColumnRollUp | undefined =>
  rollUps.find((one) => one.index === column.index && one.header === column.header);

/** The roll-up picks of the list `file`: those of the suggested series whose chosen list it is, else none. */
export const rollUpsOf = (proposal: Proposal, file: ListFile): readonly ColumnRollUp[] =>
  proposal.nodes.flatMap((node) => {
    const chosen = node.kind === 'series' ? node.list.chosen.value : null;
    return node.kind === 'series' && chosen?.source === file.source && chosen.file === file.file ? [node.columns] : [];
  })[0] ?? [];

/**
 * The level the proposer picks for the column: on each file only when its
 * rows share no value in most items (rule C, `proposal/grain.rs`), and only
 * when no slot it feeds is an item's alone. With no pick, as for a series
 * written by hand, the column goes on the item, where nothing is lost.
 */
export const suggestedLevel = (plan: ImportPlan, sheet: string, column: ColumnRef, rollUps: readonly ColumnRollUp[]): Exclude<Level, 'out'> =>
  rollUpOf(rollUps, column)?.pick.chosen.value === 'file' && fileLevelAllowed(plan, sheet, column.index) ? 'file' : 'item';

/** "Each row describes one file": the rows become media rows, and each column in use goes where the proposer picks. */
export const withFileRows = (plan: ImportPlan, sheet: string, rollUps: readonly ColumnRollUp[]): ImportPlan =>
  columnsIn(plan, sheet, ['item', 'media']).reduce(
    (edited, column) => withColumnLevel(edited, sheet, column, suggestedLevel(edited, sheet, column, rollUps)),
    withRowKind(plan, sheet, 'media'),
  );

const rowKindOf = (plan: ImportPlan, sheet: string): SheetPlan['rowKind'] | undefined => listOf(plan, sheet)?.rowKind;

const gotItsFirstPlace = (before: ImportPlan, after: ImportPlan, sheet: string, index: number): boolean =>
  levelOf(before, sheet, index) === 'out' && levelOf(after, sheet, index) !== 'out';

/** What the preview counted for one list: where its columns' values vary, how many cells each fills, the separator they hold, and the proposer's roll-up picks. */
export interface ListCounts {
  readonly grain: Grain | null;
  readonly matches: SheetColumnMatches | undefined;
  readonly cells: readonly ColumnCells[];
  readonly rollUps: readonly ColumnRollUp[];
}

export const separatorOf = (counts: ListCounts, index: number): NonNullable<ColumnCells['separator']> | null =>
  counts.cells.find((one) => one.index === index)?.separator ?? null;

export const filledOf = (counts: ListCounts, index: number): number | null => {
  const column = counts.matches?.columns.find((one) => one.index === index);
  return column === undefined || counts.matches === undefined ? null : counts.matches.rows - column.empty;
};

/** The split a column's cells suggest: the separator more than half of its filled cells hold. */
export const suggestedSplit = (counts: ListCounts, index: number): string | null => {
  const separator = separatorOf(counts, index);
  const filled = filledOf(counts, index);
  return separator !== null && filled !== null && separator.cells * 2 > filled ? separator.text : null;
};

export const splitOf = (plan: ImportPlan, sheet: string, index: number): string | null =>
  listOf(plan, sheet)?.columns.find((one) => one.column.index === index)?.options.split ?? null;

/** The column's cells cut at `split` into several values, or kept whole for `null`. */
export const withSplit = (plan: ImportPlan, sheet: string, column: ColumnRef, split: string | null): ImportPlan =>
  withList(withMapped(plan, sheet, column), sheet, (one) => ({
    ...one,
    columns: one.columns.map((each) => (each.column.index === column.index ? { ...each, options: { ...each.options, split } } : each)),
  }));

/**
 * `after` is `before` with an edit that may give the column its first field.
 * That first field takes what its counts suggest: on a list whose rows describe
 * files, the level the proposer picks; and a split no one has set yet. After that only
 * the person changes them. Simple, Advanced and the list read all map a
 * column through this.
 */
export const withCounted = (before: ImportPlan, after: ImportPlan, sheet: string, column: ColumnRef, counts: ListCounts): ImportPlan => {
  if (!gotItsFirstPlace(before, after, sheet, column.index)) {
    return after;
  }
  const placed =
    rowKindOf(after, sheet) === 'media'
      ? withColumnLevel(after, sheet, column, suggestedLevel(after, sheet, column, counts.rollUps))
      : after;
  const split = suggestedSplit(counts, column.index);
  return split !== null && splitOf(placed, sheet, column.index) === null ? withSplit(placed, sheet, column, split) : placed;
};

/** Simple's "Goes into": the column to `target`, with what its counts suggest when that is its first field. */
export const withCountedTarget = (plan: ImportPlan, sheet: string, column: ColumnRef, target: Target, counts: ListCounts): ImportPlan =>
  withCounted(plan, withTarget(plan, sheet, column, target), sheet, column, counts);

const slotOf = (target: Exclude<Target, ''>): SlotName => {
  if (target === 'template') {
    return target;
  }
  if (target.startsWith('field:')) {
    const term = target.slice('field:'.length);
    if (isTerm(term)) {
      return term;
    }
  }
  return localTerm(target.slice('new:'.length));
};

/** The term the run gives a new field: `local:` and the label in lower camel case (`plan/names.rs`). */
export const localTerm = (label: string): Term => {
  const words = label.split(/[^\p{Alphabetic}\p{N}]/u).filter((word) => word !== '');
  const camel = words.map((word, index) => {
    const lower = word.toLowerCase();
    return index === 0 ? lower : lower.charAt(0).toUpperCase() + lower.slice(1);
  });
  return `local:${camel.join('')}`;
};

/** The column's place in the rules, on the item or on each file: the first rule but a free set's that reads it. */
export const targetOf = (plan: ImportPlan, sheet: string, index: number): Target => {
  const slot = placesOf(plan, sheet, index).find((place) => place.slot !== 'sets')?.slot;
  if (slot === undefined) {
    return '';
  }
  if (slot === 'template') {
    return slot;
  }
  const made = plan.newTerms.find((term) => localTerm(term.label) === slot);
  if (made !== undefined) {
    return `new:${made.label}`;
  }
  return isTerm(slot) ? `field:${slot}` : '';
};

/**
 * Sends the column to `target` only: its earlier place is dropped, and a
 * list's column comes first among the sources. A column on each file stays
 * on each file, unless the target is an item's alone.
 */
export const withTarget = (plan: ImportPlan, sheet: string, column: ColumnRef, target: Target): ImportPlan => {
  const holder = seriesHolding(plan, sheet);
  if (holder === undefined) {
    return plan;
  }
  const onFiles = levelOf(plan, sheet, column.index) === 'file' && !itemOnly(target);
  const slots = slotsOf(plan, holder.id);
  const item = rulesWithout(slots.item, sheet, column.index);
  const media = rulesWithout(slots.media, sheet, column.index);
  const newTerms =
    target.startsWith('new:') && !plan.newTerms.some((term) => term.label === target.slice('new:'.length))
      ? [...plan.newTerms, { label: target.slice('new:'.length) }]
      : plan.newTerms;
  if (target === '') {
    return withSlots(plan, holder.id, { ...slots, item, media });
  }
  const slot = slotOf(target);
  const rules = onFiles ? media : item;
  const rule = rules[slot] ?? { sources: [], combine: 'first' as const };
  const source: SourceRef = { from: 'column', sheet, column };
  const placed = { ...rules, [slot]: { ...rule, sources: [source, ...rule.sources] } };
  return withSlots(withMapped({ ...plan, newTerms }, sheet, column), holder.id, { ...slots, item: onFiles ? item : placed, media: onFiles ? placed : media });
};

/** The list maps the column, with plain options when it did not: the server refuses a rule that reads a column its list does not map. */
const withMapped = (plan: ImportPlan, sheet: string, column: ColumnRef): ImportPlan =>
  withList(plan, sheet, (one) =>
    one.columns.some((each) => each.column.index === column.index)
      ? one
      : { ...one, columns: [...one.columns, { column, options: { split: null, type: 'literal' as const, lang: null, private: false } }] },
  );

/**
 * The list read: every column with plain options, the pairing column with the
 * most exact cells, and each column whose header is a field's label in that
 * field, split as its counts suggest.
 */
export const withRead = (
  plan: ImportPlan,
  sheet: string,
  read: SheetRead,
  counts: ListCounts,
  fields: readonly { readonly term: string; readonly label: string | null }[],
): ImportPlan => {
  const columns = read.columns
    .filter((column) => column.mappable)
    .map((column) => ({ column: { index: column.index, header: column.header }, options: { split: null, type: 'literal' as const, lang: null, private: false } }));
  const pair = best(counts.matches);
  let next = withList(plan, sheet, (one) => ({ ...one, columns, join: pair === undefined || Math.max(pair.groupKeys, pair.fileNames) === 0 ? null : joinOn(pair) }));
  for (const { column } of columns) {
    if (pair !== undefined && column.index === pair.index) {
      continue;
    }
    const field = fields.find((one) => one.label !== null && fold(one.label) === fold(column.header));
    if (field !== undefined && isTerm(field.term)) {
      next = withCountedTarget(next, sheet, column, `field:${field.term}`, counts);
    }
  }
  return next;
};

/** The delimiter a sheet view's lines are written with. */
const delimiterOf = (view: SheetView): string => {
  const delimiter = view.read?.delimiter ?? view.guess?.delimiter ?? ',';
  return delimiter === 'auto' ? ',' : delimiter;
};

/** The records of a sheet view's lines, which start at line 1. */
export const linesOf = (view: SheetView): CsvRead => readCsv(view.lines.join('\n'), delimiterOf(view));

/** The first row under the header on `headerLine`, as the reader skips a row with no text; none when the lines stop first. */
export const firstRowOf = (view: SheetView, headerLine: number): readonly string[] => {
  const { records } = linesOf(view);
  const header = records.findIndex((record) => record.line === headerLine);
  const row = header === -1 ? undefined : records.slice(header + 1).find((record) => record.cells.some((cell) => cell.trim() !== ''));
  return row?.cells ?? [];
};

/** A `<select>` value read back as a target, or `null` for one no option has. */
export const asTarget = (value: string): Target | null => {
  if (value === '' || value === 'template') {
    return value;
  }
  if (value.startsWith('new:')) {
    return `new:${value.slice('new:'.length)}`;
  }
  const term = value.slice('field:'.length);
  return value.startsWith('field:') && isTerm(term) ? `field:${term}` : null;
};
