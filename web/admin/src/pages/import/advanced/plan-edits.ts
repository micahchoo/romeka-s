/**
 * Every change an Advanced setting makes, as a pure edit of the plan
 * (`(plan) => plan`, what `useDraft().change` takes). A setting reads the plan
 * and calls one of these; Simple's answers edit the same members, so one
 * answer has one home in the plan whichever mode gave it.
 */
import { NAMED_SLOTS, isMultiValued, isTerm } from '../../../api/import-plan';
import { itemOnly, levelOf } from '../simple/list-plan';
import type {
  Action,
  ColumnOptions,
  ColumnRef,
  Defaults,
  FlagKind,
  FlagPolicy,
  ImportPlan,
  Join,
  MatchRule,
  ReadOptions,
  RecordKind,
  SheetPlan,
  SlotName,
  SlotRule,
  SlotRules,
  SourceRef,
} from '../../../api/import-plan';

/** Titles compare after trim, NFC and lower-casing (`docs/import-v2.md`, "Boundary details"). */
export const foldTitle = (text: string): string => text.trim().normalize('NFC').toLowerCase();

const onSheet = (plan: ImportPlan, sheet: string, edit: (one: SheetPlan) => SheetPlan): ImportPlan => ({
  ...plan,
  sheets: plan.sheets.map((one) => (one.id === sheet ? edit(one) : one)),
});

export const setRead = (plan: ImportPlan, sheet: string, read: Partial<ReadOptions>): ImportPlan =>
  onSheet(plan, sheet, (one) => ({ ...one, read: { ...one.read, ...read } }));

export const NO_NORMALIZE: Join['normalize'] = { caseFold: false, leadingZeros: false, separators: false };

/** The sheet joined on `column`, keeping how it joined before; `null` joins it to nothing. */
export const setJoinColumn = (plan: ImportPlan, sheet: string, column: ColumnRef | null): ImportPlan =>
  onSheet(plan, sheet, (one) => ({
    ...one,
    join: column === null ? null : { on: one.join?.on ?? 'identifier', column, normalize: one.join?.normalize ?? NO_NORMALIZE },
  }));

export const setJoin = (plan: ImportPlan, sheet: string, join: Partial<Omit<Join, 'column'>>): ImportPlan =>
  onSheet(plan, sheet, (one) => (one.join === null ? one : { ...one, join: { ...one.join, ...join } }));

const sameColumn = (a: ColumnRef, b: ColumnRef): boolean => a.index === b.index;

const readsColumn = (source: SourceRef, sheet: string, column: ColumnRef): boolean =>
  source.from === 'column' && source.sheet === sheet && sameColumn(source.column, column);

const rulesOf = (plan: ImportPlan, kind: RecordKind): SlotRules => plan.slots[kind];

const withRules = (plan: ImportPlan, kind: RecordKind, rules: SlotRules): ImportPlan => ({
  ...plan,
  slots: { ...plan.slots, [kind]: rules },
});

const slotName = (key: string): SlotName | null => NAMED_SLOTS.find((name) => name === key) ?? (isTerm(key) ? key : null);

/** Each rule of `rules` with its field, in the plan's order. */
export const entriesOf = (rules: SlotRules): readonly (readonly [SlotName, SlotRule])[] =>
  Object.entries<SlotRule | undefined>(rules).flatMap(([key, rule]) => {
    const slot = slotName(key);
    return rule === undefined || slot === null ? [] : [[slot, rule] as const];
  });

const rulesFrom = (entries: readonly (readonly [SlotName, SlotRule])[]): SlotRules => Object.fromEntries(entries);

/** The field a list column feeds, on the item or on each file, or `null` when it feeds none. */
export const columnTarget = (plan: ImportPlan, sheet: string, column: ColumnRef): SlotName | null =>
  [...entriesOf(rulesOf(plan, 'item')), ...entriesOf(rulesOf(plan, 'media'))].find(([, rule]) => rule.sources.some((source) => readsColumn(source, sheet, column)))?.[0] ??
  null;

const withoutColumn = (rules: SlotRules, sheet: string, column: ColumnRef): SlotRules =>
  rulesFrom(
    entriesOf(rules).flatMap(([name, rule]) => {
      const sources = rule.sources.filter((source) => !readsColumn(source, sheet, column));
      if (sources.length === rule.sources.length) {
        return [[name, rule] as const];
      }
      return sources.length === 0 && rule.map === undefined ? [] : [[name, { ...rule, sources }] as const];
    }),
  );

/**
 * The column feeding `slot` alone, ahead of the slot's other sources, as a
 * filled list's columns are, at the level it had: a column on each file
 * stays on each file, unless `slot` is an item's alone. A rule the column
 * was the only source of, with no value map, goes with it: a rule left with
 * no source would clear the field on `update`.
 */
export const mapColumn = (plan: ImportPlan, sheet: string, column: ColumnRef, slot: SlotName | null): ImportPlan => {
  const onFiles = levelOf(plan, sheet, column.index) === 'file' && (slot === null || !itemOnly(slot));
  const cleared = withRules(withRules(plan, 'item', withoutColumn(rulesOf(plan, 'item'), sheet, column)), 'media', withoutColumn(rulesOf(plan, 'media'), sheet, column));
  if (slot === null) {
    return cleared;
  }
  const kind = onFiles ? 'media' : 'item';
  const rules = rulesOf(cleared, kind);
  const source: SourceRef = { from: 'column', sheet, column };
  const rule = rules[slot];
  return withRules(cleared, kind, {
    ...rules,
    [slot]: rule === undefined ? { sources: [source], combine: 'first' } : { ...rule, sources: [source, ...rule.sources] },
  });
};

/** Each row's kind read from `column`, whose cells name it. */
export const setRowKindColumn = (plan: ImportPlan, sheet: string, column: ColumnRef): ImportPlan => onSheet(plan, sheet, (one) => ({ ...one, rowKind: { column } }));

/** For media rows that do not join by file name: the column naming their item. */
export const setItemKey = (plan: ImportPlan, sheet: string, column: ColumnRef | null): ImportPlan => onSheet(plan, sheet, (one) => ({ ...one, itemKey: column }));

export const DEFAULT_COLUMN_OPTIONS: ColumnOptions = { split: null, type: 'literal', lang: null, private: false };

export const columnOptions = (sheet: SheetPlan, column: ColumnRef): ColumnOptions =>
  sheet.columns.find((one) => sameColumn(one.column, column))?.options ?? DEFAULT_COLUMN_OPTIONS;

export const setColumnOptions = (plan: ImportPlan, sheet: string, column: ColumnRef, options: Partial<ColumnOptions>): ImportPlan =>
  onSheet(plan, sheet, (one) => {
    const next = { ...columnOptions(one, column), ...options };
    const has = one.columns.some((each) => sameColumn(each.column, column));
    return {
      ...one,
      columns: has
        ? one.columns.map((each) => (sameColumn(each.column, column) ? { column, options: next } : each))
        : [...one.columns, { column, options: next }],
    };
  });

const onRule = (plan: ImportPlan, kind: RecordKind, slot: SlotName, edit: (rule: SlotRule) => SlotRule): ImportPlan => {
  const rule = rulesOf(plan, kind)[slot];
  return rule === undefined ? plan : withRules(plan, kind, { ...rulesOf(plan, kind), [slot]: edit(rule) });
};

export const addRule = (plan: ImportPlan, kind: RecordKind, slot: SlotName): ImportPlan =>
  rulesOf(plan, kind)[slot] === undefined ? withRules(plan, kind, { ...rulesOf(plan, kind), [slot]: { sources: [], combine: 'first' } }) : plan;

export const removeRule = (plan: ImportPlan, kind: RecordKind, slot: SlotName): ImportPlan =>
  withRules(plan, kind, rulesFrom(entriesOf(rulesOf(plan, kind)).filter(([name]) => name !== slot)));

/** Source `index` moved `by` places (-1 up, 1 down); a move past either end does nothing. */
export const moveSource = (plan: ImportPlan, kind: RecordKind, slot: SlotName, index: number, by: -1 | 1): ImportPlan =>
  onRule(plan, kind, slot, (rule) => {
    const to = index + by;
    const moving = rule.sources[index];
    const other = rule.sources[to];
    if (moving === undefined || other === undefined) {
      return rule;
    }
    const sources = [...rule.sources];
    sources[to] = moving;
    sources[index] = other;
    return { ...rule, sources };
  });

export const removeSource = (plan: ImportPlan, kind: RecordKind, slot: SlotName, index: number): ImportPlan =>
  onRule(plan, kind, slot, (rule) => ({ ...rule, sources: rule.sources.filter((_, at) => at !== index) }));

export const addSource = (plan: ImportPlan, kind: RecordKind, slot: SlotName, source: SourceRef): ImportPlan =>
  onRule(plan, kind, slot, (rule) => ({ ...rule, sources: [...rule.sources, source] }));

/** `all` only where the field keeps more than one value: the boundary refuses it elsewhere. */
export const setCombine = (plan: ImportPlan, kind: RecordKind, slot: SlotName, combine: SlotRule['combine']): ImportPlan =>
  combine === 'all' && !isMultiValued(slot) ? plan : onRule(plan, kind, slot, (rule) => ({ ...rule, combine }));

/** What the value map makes of `value`: its mapped text, or the value itself. */
export const mappedText = (rule: SlotRule, value: string): string => {
  const key = Object.keys(rule.map ?? {}).find((one) => foldTitle(one) === foldTitle(value));
  return key === undefined ? value : (rule.map?.[key] ?? value);
};

/**
 * `value` mapped to `to`. Keys compare as titles, and two keys that fold to
 * one are refused on save, so the old key for the value goes first. A value
 * mapped to itself needs no key; an empty map is absent.
 */
export const setMapped = (plan: ImportPlan, kind: RecordKind, slot: SlotName, value: string, to: string): ImportPlan =>
  onRule(plan, kind, slot, (rule) => {
    const rest = Object.entries(rule.map ?? {}).filter(([key]) => foldTitle(key) !== foldTitle(value));
    const map = Object.fromEntries(to === value ? rest : [...rest, [value, to]]);
    return Object.keys(map).length === 0 ? { sources: rule.sources, combine: rule.combine } : { sources: rule.sources, combine: rule.combine, map };
  });

export const setDefaults = (plan: ImportPlan, defaults: Partial<Defaults>): ImportPlan => ({
  ...plan,
  defaults: { ...plan.defaults, ...defaults },
});

/**
 * The `unmatched` flag's policy falls back to `action.unmatched`, so the one
 * answer to it lives there: a policy stored for the kind would hide it.
 */
export const setUnmatched = (plan: ImportPlan, unmatched: ImportPlan['action']['unmatched']): ImportPlan => {
  const flags = Object.fromEntries(Object.entries(plan.flags).filter(([kind]) => kind !== 'unmatched'));
  return { ...plan, flags, action: { ...plan.action, unmatched } };
};

export const setFlagPolicy = (plan: ImportPlan, kind: FlagKind, policy: FlagPolicy): ImportPlan =>
  kind === 'unmatched' ? setUnmatched(plan, policy === 'leave' ? 'leave' : 'create') : { ...plan, flags: { ...plan.flags, [kind]: policy } };

export const DEFAULT_ANOMALIES: NonNullable<ImportPlan['anomalies']> = { identical: 'keepOne', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' };

export const anomaliesOf = (plan: ImportPlan): NonNullable<ImportPlan['anomalies']> => plan.anomalies ?? DEFAULT_ANOMALIES;

export const setAnomalies = (plan: ImportPlan, answers: Partial<NonNullable<ImportPlan['anomalies']>>): ImportPlan => ({
  ...plan,
  anomalies: { ...anomaliesOf(plan), ...answers },
});

/** Field rules a delete plan cannot hold: the boundary refuses them. */
export const ruleCount = (plan: ImportPlan): number =>
  entriesOf(plan.slots.item).length + entriesOf(plan.slots.media).length + entriesOf(plan.slots.itemSet).length;

/** The plan with `action`; a delete drops every field rule, which a delete plan may not hold. */
export const setAction = (plan: ImportPlan, action: Action): ImportPlan => ({
  ...plan,
  action: { ...plan.action, action },
  ...(action === 'delete' ? { slots: { item: {}, media: {}, itemSet: {} } } : {}),
});

export const setMatch = (plan: ImportPlan, kind: RecordKind, match: Partial<MatchRule>): ImportPlan => ({
  ...plan,
  action: { ...plan.action, match: { ...plan.action.match, [kind]: { ...plan.action.match[kind], ...match } } },
});

export const removeDecision = (plan: ImportPlan, index: number): ImportPlan => ({
  ...plan,
  decisions: plan.decisions.filter((_, at) => at !== index),
});

export const removeDecisions = (plan: ImportPlan): ImportPlan => ({ ...plan, decisions: [] });

export const setPattern = (plan: ImportPlan, pattern: string | null, caseFold: boolean): ImportPlan => ({
  ...plan,
  grouping: { pattern, caseFold },
});
