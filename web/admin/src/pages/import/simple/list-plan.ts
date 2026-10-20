/**
 * The plan edits "Your list" makes. A list found in a folder becomes a sheet
 * of the plan read with automatic options; its pairing column is the one with
 * the most cells that exactly equal an item's code or a file's name, a count
 * the screen shows; and a column goes into a field only when its header is
 * that field's label (compared as titles). Every other column waits for the
 * person.
 */
import type { SheetColumnMatches } from '../../../api/import-sections';
import type { SheetRead } from '../../../api/import-sheet-views';
import type { ColumnRef, ImportPlan, SheetPlan, SlotName, SlotRule, SlotRules, SourceRef, Term } from '../../../api/import-plan';
import { isTerm } from '../../../api/import-plan';

/** Where a column goes: nowhere, a field, the item type, the item set, or a new field. */
export type Target = '' | `field:${Term}` | 'template' | 'sets' | `new:${string}`;

export const fold = (text: string): string => text.trim().normalize('NFC').toLowerCase();

const nextId = (plan: ImportPlan): string => {
  const taken = new Set(plan.sheets.map((sheet) => sheet.id));
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

export const withJoin = (plan: ImportPlan, sheet: string, column: SheetColumnMatches['columns'][number]): ImportPlan => ({
  ...plan,
  sheets: plan.sheets.map((one) => (one.id === sheet ? { ...one, join: joinOn(column) } : one)),
});

const rulesWithout = (rules: SlotRules, sheet: string, index: number): SlotRules => {
  const next: { [slot: string]: SlotRule } = {};
  for (const [slot, rule] of Object.entries<SlotRule | undefined>(rules)) {
    if (rule !== undefined) {
      next[slot] = { ...rule, sources: rule.sources.filter((source) => !(source.from === 'column' && source.sheet === sheet && source.column.index === index)) };
    }
  }
  return next;
};

const slotOf = (target: Exclude<Target, ''>): SlotName => {
  if (target === 'template' || target === 'sets') {
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

/** The column's place in the item rules: the first rule that reads it. */
export const targetOf = (plan: ImportPlan, sheet: string, index: number): Target => {
  for (const [slot, rule] of Object.entries<SlotRule | undefined>(plan.slots.item)) {
    if (rule?.sources.some((source) => source.from === 'column' && source.sheet === sheet && source.column.index === index) === true) {
      if (slot === 'template' || slot === 'sets') {
        return slot;
      }
      const made = plan.newTerms.find((term) => localTerm(term.label) === slot);
      if (made !== undefined) {
        return `new:${made.label}`;
      }
      return isTerm(slot) ? `field:${slot}` : '';
    }
  }
  return '';
};

/** Sends the column to `target` only: its earlier place is dropped, and a list's column comes first among the sources. */
export const withTarget = (plan: ImportPlan, sheet: string, column: ColumnRef, target: Target): ImportPlan => {
  const item = rulesWithout(plan.slots.item, sheet, column.index);
  const newTerms =
    target.startsWith('new:') && !plan.newTerms.some((term) => term.label === target.slice('new:'.length))
      ? [...plan.newTerms, { label: target.slice('new:'.length) }]
      : plan.newTerms;
  if (target === '') {
    return { ...plan, slots: { ...plan.slots, item } };
  }
  const slot = slotOf(target);
  const rule = item[slot] ?? { sources: [], combine: 'first' as const };
  const source: SourceRef = { from: 'column', sheet, column };
  return { ...plan, newTerms, slots: { ...plan.slots, item: { ...item, [slot]: { ...rule, sources: [source, ...rule.sources] } } } };
};

/**
 * The list read: every column with plain options, the pairing column with the
 * most exact cells, and each column whose header is a field's label in that
 * field.
 */
export const withRead = (
  plan: ImportPlan,
  sheet: string,
  read: SheetRead,
  matches: SheetColumnMatches | undefined,
  fields: readonly { readonly term: string; readonly label: string | null }[],
): ImportPlan => {
  const columns = read.columns
    .filter((column) => column.mappable)
    .map((column) => ({ column: { index: column.index, header: column.header }, options: { split: null, type: 'literal' as const, lang: null, private: false } }));
  const pair = best(matches);
  let next: ImportPlan = {
    ...plan,
    sheets: plan.sheets.map((one) =>
      one.id === sheet ? { ...one, columns, join: pair === undefined || Math.max(pair.groupKeys, pair.fileNames) === 0 ? null : joinOn(pair) } : one,
    ),
  };
  for (const { column } of columns) {
    if (pair !== undefined && column.index === pair.index) {
      continue;
    }
    const field = fields.find((one) => one.label !== null && fold(one.label) === fold(column.header));
    if (field !== undefined && isTerm(field.term)) {
      next = withTarget(next, sheet, column, `field:${field.term}`);
    }
  }
  return next;
};

/** The cells of one raw line, read with the list's delimiter and double quotes. */
export const cellsOf = (line: string, delimiter: string): readonly string[] => {
  const cells: string[] = [];
  let cell = '';
  let quoted = false;
  for (let at = 0; at < line.length; at += 1) {
    const char = line.charAt(at);
    if (quoted) {
      if (char === '"' && line.charAt(at + 1) === '"') {
        cell += '"';
        at += 1;
      } else if (char === '"') {
        quoted = false;
      } else {
        cell += char;
      }
    } else if (char === '"' && cell === '') {
      quoted = true;
    } else if (char === delimiter) {
      cells.push(cell);
      cell = '';
    } else {
      cell += char;
    }
  }
  cells.push(cell);
  return cells;
};

/** A `<select>` value read back as a target, or `null` for one no option has. */
export const asTarget = (value: string): Target | null => {
  if (value === '' || value === 'template' || value === 'sets') {
    return value;
  }
  if (value.startsWith('new:')) {
    return `new:${value.slice('new:'.length)}`;
  }
  const term = value.slice('field:'.length);
  return value.startsWith('field:') && isTerm(term) ? `field:${term}` : null;
};
