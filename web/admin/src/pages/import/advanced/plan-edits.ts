/**
 * Every change an Advanced setting makes, as a pure edit of the plan
 * (`(plan) => plan`, what `useDraft().change` takes). A setting reads the plan
 * and calls one of these; Simple's answers edit the same members, so one
 * answer has one home in the plan whichever mode gave it. Each writer goes
 * through `written` with its name, so its kind in `WRITES` (`writes.ts`)
 * decides whether it also owns the folder it writes.
 */
import { NAMED_SLOTS, isMultiValued, isTerm } from '../../../api/import-plan';
import { allSeries, seriesHolding, seriesOf, withEverySeries, withList, withSeries, withSetLevel, withSets } from '../series';
import { itemOnly, levelOf, withColumnMapped } from '../simple/list-plan';
import { written } from './writes';
import type { Writer } from './writes';
import type {
  Action,
  ColumnOptions,
  ColumnRef,
  Defaults,
  FlagKind,
  FlagPolicy,
  ImportPlan,
  Join,
  LevelId,
  MatchRule,
  ReadOptions,
  RecordKind,
  SheetPlan,
  SlotName,
  SlotRule,
  SlotRules,
  SetSource,
  SourceRef,
} from '../../../api/import-plan';

/** Titles compare after trim, NFC and lower-casing (`docs/import-v2.md`, "Boundary details"). */
export const foldTitle = (text: string): string => text.trim().normalize('NFC').toLowerCase();

/** The write `name` of the list `sheet`, on the series that holds it. */
const onSheet = (plan: ImportPlan, name: Writer, sheet: string, edit: (one: SheetPlan) => SheetPlan): ImportPlan =>
  written(plan, name, seriesHolding(plan, sheet)?.id, (now) => withList(now, sheet, edit));

export const setRead = (plan: ImportPlan, sheet: string, read: Partial<ReadOptions>): ImportPlan =>
  onSheet(plan, 'setRead', sheet, (one) => ({ ...one, read: { ...one.read, ...read } }));

export const NO_NORMALIZE: Join['normalize'] = { caseFold: false, leadingZeros: false, separators: false };

/** The sheet joined on `column`, keeping how it joined before; `null` joins it to nothing. */
export const setJoinColumn = (plan: ImportPlan, sheet: string, column: ColumnRef | null): ImportPlan =>
  onSheet(plan, 'setJoinColumn', sheet, (one) => ({
    ...one,
    join: column === null ? null : { on: one.join?.on ?? 'identifier', column, normalize: one.join?.normalize ?? NO_NORMALIZE },
  }));

export const setJoin = (plan: ImportPlan, sheet: string, join: Partial<Omit<Join, 'column'>>): ImportPlan =>
  onSheet(plan, 'setJoin', sheet, (one) => (one.join === null ? one : { ...one, join: { ...one.join, ...join } }));

const sameColumn = (a: ColumnRef, b: ColumnRef): boolean => a.index === b.index;

const readsColumn = (source: SourceRef, sheet: string, column: ColumnRef): boolean =>
  source.from === 'column' && source.sheet === sheet && sameColumn(source.column, column);

const rulesOf = (plan: ImportPlan, level: LevelId, kind: RecordKind): SlotRules => seriesOf(plan, level).slots[kind];

const withRules = (plan: ImportPlan, level: LevelId, kind: RecordKind, rules: SlotRules): ImportPlan =>
  withSeries(plan, level, (series) => ({ ...series, slots: { ...series.slots, [kind]: rules } }));

const slotName = (key: string): SlotName | null => NAMED_SLOTS.find((name) => name === key) ?? (isTerm(key) ? key : null);

/** Each rule of `rules` with its field, in the plan's order. */
export const entriesOf = (rules: SlotRules): readonly (readonly [SlotName, SlotRule])[] =>
  Object.entries<SlotRule | undefined>(rules).flatMap(([key, rule]) => {
    const slot = slotName(key);
    return rule === undefined || slot === null ? [] : [[slot, rule] as const];
  });

const rulesFrom = (entries: readonly (readonly [SlotName, SlotRule])[]): SlotRules => Object.fromEntries(entries);

/** The field a list column feeds, on the item or on each file, or `null` when it feeds none. Only the series that holds the list reads its columns. */
export const columnTarget = (plan: ImportPlan, sheet: string, column: ColumnRef): SlotName | null => {
  const holder = seriesHolding(plan, sheet);
  return holder === undefined
    ? null
    : ([...entriesOf(rulesOf(plan, holder.id, 'item')), ...entriesOf(rulesOf(plan, holder.id, 'media'))].find(([, rule]) =>
        rule.sources.some((source) => readsColumn(source, sheet, column)),
      )?.[0] ?? null);
};

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
  const holder = seriesHolding(plan, sheet);
  return holder === undefined ? plan : written(plan, 'mapColumn', holder.id, (now) => mappedColumn(now, holder.id, sheet, column, slot));
};

const mappedColumn = (plan: ImportPlan, level: LevelId, sheet: string, column: ColumnRef, slot: SlotName | null): ImportPlan => {
  const onFiles = levelOf(plan, sheet, column.index) === 'file' && (slot === null || !itemOnly(slot));
  const cleared = withRules(
    withRules(plan, level, 'item', withoutColumn(rulesOf(plan, level, 'item'), sheet, column)),
    level,
    'media',
    withoutColumn(rulesOf(plan, level, 'media'), sheet, column),
  );
  if (slot === null) {
    return cleared;
  }
  const kind = onFiles ? 'media' : 'item';
  // The server refuses a rule that reads a column its list does not map.
  const mapped = withColumnMapped(cleared, sheet, column);
  const rules = rulesOf(mapped, level, kind);
  const source: SourceRef = { from: 'column', sheet, column };
  const rule = rules[slot];
  return withRules(mapped, level, kind, {
    ...rules,
    [slot]: rule === undefined ? { sources: [source], combine: 'first' } : { ...rule, sources: [source, ...rule.sources] },
  });
};

/** Each row's kind read from `column`, whose cells name it. */
export const setRowKindColumn = (plan: ImportPlan, sheet: string, column: ColumnRef): ImportPlan => onSheet(plan, 'setRowKindColumn', sheet, (one) => ({ ...one, rowKind: { column } }));

/** For media rows that do not join by file name: the column naming their item. */
export const setItemKey = (plan: ImportPlan, sheet: string, column: ColumnRef | null): ImportPlan => onSheet(plan, 'setItemKey', sheet, (one) => ({ ...one, itemKey: column }));

export const DEFAULT_COLUMN_OPTIONS: ColumnOptions = { split: null, type: 'literal', lang: null, private: false };

export const columnOptions = (sheet: SheetPlan, column: ColumnRef): ColumnOptions =>
  sheet.columns.find((one) => sameColumn(one.column, column))?.options ?? DEFAULT_COLUMN_OPTIONS;

export const setColumnOptions = (plan: ImportPlan, sheet: string, column: ColumnRef, options: Partial<ColumnOptions>): ImportPlan =>
  onSheet(plan, 'setColumnOptions', sheet, (one) => {
    const next = { ...columnOptions(one, column), ...options };
    const has = one.columns.some((each) => sameColumn(each.column, column));
    return {
      ...one,
      columns: has
        ? one.columns.map((each) => (sameColumn(each.column, column) ? { column, options: next } : each))
        : [...one.columns, { column, options: next }],
    };
  });

/** One field's rule: of the series `level`, for records of `kind`. */
export interface RuleAt {
  readonly level: LevelId;
  readonly kind: RecordKind;
  readonly slot: SlotName;
}

const onRule = (plan: ImportPlan, name: Writer, { level, kind, slot }: RuleAt, edit: (rule: SlotRule) => SlotRule): ImportPlan =>
  written(plan, name, level, (now) => {
    const rule = rulesOf(now, level, kind)[slot];
    return rule === undefined ? now : withRules(now, level, kind, { ...rulesOf(now, level, kind), [slot]: edit(rule) });
  });

export const addRule = (plan: ImportPlan, { level, kind, slot }: RuleAt): ImportPlan =>
  written(plan, 'addRule', level, (now) =>
    rulesOf(now, level, kind)[slot] === undefined ? withRules(now, level, kind, { ...rulesOf(now, level, kind), [slot]: { sources: [], combine: 'first' } }) : now,
  );

export const removeRule = (plan: ImportPlan, { level, kind, slot }: RuleAt): ImportPlan =>
  written(plan, 'removeRule', level, (now) => withRules(now, level, kind, rulesFrom(entriesOf(rulesOf(now, level, kind)).filter(([name]) => name !== slot))));

/** Source `index` moved `by` places (-1 up, 1 down); a move past either end does nothing. */
export const moveSource = (plan: ImportPlan, at: RuleAt, index: number, by: -1 | 1): ImportPlan =>
  onRule(plan, 'moveSource', at, (rule) => {
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

export const removeSource = (plan: ImportPlan, at: RuleAt, index: number): ImportPlan =>
  onRule(plan, 'removeSource', at, (rule) => ({ ...rule, sources: rule.sources.filter((_, place) => place !== index) }));

export const addSource = (plan: ImportPlan, at: RuleAt, source: SourceRef): ImportPlan => onRule(plan, 'addSource', at, (rule) => ({ ...rule, sources: [...rule.sources, source] }));

/** `all` only where the field keeps more than one value: the boundary refuses it elsewhere. */
export const setCombine = (plan: ImportPlan, at: RuleAt, combine: SlotRule['combine']): ImportPlan =>
  combine === 'all' && !isMultiValued(at.slot) ? plan : onRule(plan, 'setCombine', at, (rule) => ({ ...rule, combine }));

export const setDefaults = (plan: ImportPlan, defaults: Partial<Defaults>): ImportPlan =>
  written(plan, 'setDefaults', undefined, (now) => ({ ...now, defaults: { ...now.defaults, ...defaults } }));

/**
 * The `unmatched` flag's policy falls back to `action.unmatched`, so the one
 * answer to it lives there: a policy stored for the kind would hide it.
 */
export const setUnmatched = (plan: ImportPlan, unmatched: ImportPlan['action']['unmatched']): ImportPlan =>
  written(plan, 'setUnmatched', undefined, (now) => {
    const flags = Object.fromEntries(Object.entries(now.flags).filter(([kind]) => kind !== 'unmatched'));
    return { ...now, flags, action: { ...now.action, unmatched } };
  });

export const setFlagPolicy = (plan: ImportPlan, kind: FlagKind, policy: FlagPolicy): ImportPlan =>
  kind === 'unmatched'
    ? setUnmatched(plan, policy === 'leave' ? 'leave' : 'create')
    : written(plan, 'setFlagPolicy', undefined, (now) => ({ ...now, flags: { ...now.flags, [kind]: policy } }));

export const DEFAULT_ANOMALIES: NonNullable<ImportPlan['anomalies']> = { identical: 'keepOne', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' };

export const anomaliesOf = (plan: ImportPlan): NonNullable<ImportPlan['anomalies']> => plan.anomalies ?? DEFAULT_ANOMALIES;

export const setAnomalies = (plan: ImportPlan, answers: Partial<NonNullable<ImportPlan['anomalies']>>): ImportPlan =>
  written(plan, 'setAnomalies', undefined, (now) => ({ ...now, anomalies: { ...anomaliesOf(now), ...answers } }));

/** The rules Fields lists, in every series: the field rules a delete plan cannot hold. */
export const ruleCount = (plan: ImportPlan): number =>
  allSeries(plan).reduce((count, { slots }) => count + entriesOf(slots.item).length + entriesOf(slots.media).length + entriesOf(slots.itemSet).length, 0);

/** The plan with `action`; a delete drops every rule Fields lists, in every series: the boundary refuses field rules on a delete. */
export const setAction = (plan: ImportPlan, action: Action): ImportPlan =>
  written(plan, 'setAction', undefined, (now) => {
    const next = { ...now, action: { ...now.action, action } };
    return action === 'delete' ? withEverySeries(next, (series) => ({ ...series, slots: { item: {}, media: {}, itemSet: {} } })) : next;
  });

export const setMatch = (plan: ImportPlan, kind: RecordKind, match: Partial<MatchRule>): ImportPlan =>
  written(plan, 'setMatch', undefined, (now) => ({
    ...now,
    action: { ...now.action, match: { ...now.action.match, [kind]: { ...now.action.match[kind], ...match } } },
  }));

export const removeDecision = (plan: ImportPlan, index: number): ImportPlan =>
  written(plan, 'removeDecision', undefined, (now) => ({ ...now, decisions: now.decisions.filter((_, at) => at !== index) }));

export const removeDecisions = (plan: ImportPlan): ImportPlan => written(plan, 'removeDecisions', undefined, (now) => ({ ...now, decisions: [] }));

export const setPattern = (plan: ImportPlan, level: LevelId, pattern: string | null, caseFold: boolean): ImportPlan =>
  written(plan, 'setPattern', level, (now) => withSeries(now, level, (series) => ({ ...series, grouping: { pattern, caseFold } })));

/** The free sets after `change`: sets every item joins, which the proposer copies as they are. */
export const setFreeSets = (plan: ImportPlan, change: (sets: readonly SetSource[]) => readonly SetSource[]): ImportPlan =>
  written(plan, 'setFreeSets', undefined, (now) => withSets(now, change));

/** The set the series `level` makes: `null` for none. The proposer works a suggested folder's set out again, so it is owned. */
export const setSeriesSet = (plan: ImportPlan, level: LevelId, set: SetSource | null): ImportPlan =>
  written(plan, 'setSeriesSet', level, (now) => withSeries(now, level, (series) => ({ ...series, set })));

/** The set the set level `id` makes; `null` takes the level away. Only a level kept around a folder written by hand is the plan's to write. */
export const setSetLevel = (plan: ImportPlan, id: LevelId, set: SetSource | null): ImportPlan =>
  written(plan, 'setSetLevel', undefined, (now) => withSetLevel(now, id, set));
