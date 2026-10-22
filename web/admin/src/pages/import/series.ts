/**
 * The series seam: the one place the import pages reach the plan's tree
 * (`structure`, `plan/structure.rs`) and Simple's state, which v3 keys by
 * level id. Every other import file reads a series through `seriesOf`, its
 * lists through `listOf` and `allLists`, its slot rules through `slotsOf`,
 * the free sets through `setsOf`, and writes each through the matching
 * `with…` (`import-series.test.ts` holds them to it).
 *
 * A screen that edits one series and is given a plan of several is refused
 * (`SeveralSeriesError`) until the structure screen exists. The readers keyed
 * by a list id (`listOf`, `withList`) and the ones that read every series
 * (`allLists`, `withEverySeries`) are right for any number.
 *
 * Sets in v3 are free sets and set levels, never a series' `item.sets` rule.
 * Simple and Advanced still edit one "item sets" rule beside the other item
 * rules, so `slotsOf` shows the first `each` free set as that rule and
 * `withSlots` writes it back there. A set named by its title (`{ title }`)
 * or picked from the install (`{ id }`) is a free set of its own.
 */
import type { ImportPlan, Level, LevelId, Series, SetLevel, SetSource, SheetPlan, SlotRule, Slots, Structure } from '../../api/import-plan';
import type { JsonValue } from '../../api/types';

/** A screen that edits one series was given a plan of several. */
export class SeveralSeriesError extends Error {
  constructor(count: number) {
    super(`This plan has ${String(count)} series. This screen edits a plan of one series; the structure screen edits the others.`);
    this.name = 'SeveralSeriesError';
  }
}

const seriesIn = (levels: readonly Level[]): readonly Series[] =>
  levels.flatMap((level) => (level.kind === 'series' ? [level] : seriesIn(level.levels)));

/** Every series, depth first: the order the server numbers them in. */
export const allSeries = (plan: ImportPlan): readonly Series[] => seriesIn(plan.structure.levels);

/** The series `level`; with no `level`, the plan's one series. */
export const seriesOf = (plan: ImportPlan, level?: LevelId): Series => {
  const all = allSeries(plan);
  if (level === undefined) {
    const [only, ...more] = all;
    if (only === undefined || more.length > 0) {
      throw new SeveralSeriesError(all.length);
    }
    return only;
  }
  const found = all.find((series) => series.id === level);
  if (found === undefined) {
    throw new Error(`The plan has no series "${level}".`);
  }
  return found;
};

const replaced = (levels: readonly Level[], change: (series: Series) => Series): readonly Level[] =>
  levels.map((level) => (level.kind === 'series' ? change(level) : { ...level, levels: replaced(level.levels, change) }));

const withLevels = (plan: ImportPlan, change: (series: Series) => Series): ImportPlan => ({
  ...plan,
  structure: { ...plan.structure, levels: replaced(plan.structure.levels, change) },
});

/**
 * The plan with `change` made to the series `level` (with no `level`, the
 * one series). A change that returns the series it was given leaves the plan
 * itself, so a caller can tell a no-op by identity.
 */
export const withSeries = (plan: ImportPlan, change: (series: Series) => Series, level?: LevelId): ImportPlan => {
  const before = seriesOf(plan, level);
  const after = change(before);
  return after === before ? plan : withLevels(plan, (series) => (series === before ? after : series));
};

/** The plan with `change` made to every series. */
export const withEverySeries = (plan: ImportPlan, change: (series: Series) => Series): ImportPlan => withLevels(plan, change);

/** Every list of every series. Sheet ids are unique across the plan. */
export const allLists = (plan: ImportPlan): readonly SheetPlan[] => allSeries(plan).flatMap((series) => series.lists);

/** The list with this id, in whichever series holds it. */
export const listOf = (plan: ImportPlan, sheet: string): SheetPlan | undefined => allLists(plan).find((one) => one.id === sheet);

/** The plan with `edit` made to the list `sheet`, in whichever series holds it; a plan without it is left as it is. */
export const withList = (plan: ImportPlan, sheet: string, edit: (list: SheetPlan) => SheetPlan): ImportPlan => {
  const holder = allSeries(plan).find((series) => series.lists.some((one) => one.id === sheet));
  return holder === undefined
    ? plan
    : withSeries(plan, (series) => ({ ...series, lists: series.lists.map((one) => (one.id === sheet ? edit(one) : one)) }), holder.id);
};

/** The plan with this whole tree in place of its own: what the structure screen takes from a proposal. */
export const withStructure = (plan: ImportPlan, structure: Structure): ImportPlan => ({ ...plan, structure });

/** The free sets: sets every item joins, after the sets of its tree. */
export const setsOf = (plan: ImportPlan): readonly SetSource[] => plan.structure.freeSets;

/** The set levels, depth first. Read only: the structure screen edits them. */
export const setLevelsOf = (plan: ImportPlan): readonly SetLevel[] => {
  const visit = (levels: readonly Level[]): readonly SetLevel[] =>
    levels.flatMap((level) => (level.kind === 'set' ? [level, ...visit(level.levels)] : []));
  return visit(plan.structure.levels);
};

export const withSets = (plan: ImportPlan, change: (sets: readonly SetSource[]) => readonly SetSource[]): ImportPlan => ({
  ...plan,
  structure: { ...plan.structure, freeSets: change(plan.structure.freeSets) },
});

const ruleAt = (sets: readonly SetSource[]): number => sets.findIndex((set) => 'each' in set);

/** The "item sets" rule Simple and Advanced edit: the first `each` free set, as a slot rule. */
export const setsRuleOf = (plan: ImportPlan): SlotRule | undefined => {
  const set = setsOf(plan)[ruleAt(setsOf(plan))];
  if (set === undefined || !('each' in set)) {
    return undefined;
  }
  const { sources, combine, map } = set.each;
  return map === undefined ? { sources, combine } : { sources, combine, map };
};

const sameRule = (a: SlotRule | undefined, b: SlotRule | undefined): boolean => JSON.stringify(a) === JSON.stringify(b);

/**
 * The plan with this "item sets" rule, in place of the first `each` free set
 * and keeping how it names sets; a new one names them by title. `undefined`
 * removes it.
 */
export const withSetsRule = (plan: ImportPlan, rule: SlotRule | undefined): ImportPlan => {
  if (sameRule(rule, setsRuleOf(plan))) {
    return plan;
  }
  return withSets(plan, (sets) => {
    const at = ruleAt(sets);
    if (rule === undefined) {
      return sets.filter((_, index) => index !== at);
    }
    const was = sets[at];
    const each: SetSource = { each: { ...rule, names: was !== undefined && 'each' in was ? was.each.names : 'title' } };
    return at === -1 ? [...sets, each] : sets.map((set, index) => (index === at ? each : set));
  });
};

/** The series' slot rules, with the "item sets" rule among its item rules. */
export const slotsOf = (plan: ImportPlan, level?: LevelId): Slots => {
  const { slots } = seriesOf(plan, level);
  const sets = setsRuleOf(plan);
  return sets === undefined ? slots : { ...slots, item: { ...slots.item, sets } };
};

/** The plan with these slot rules: the "item sets" rule to the free sets, the rest to the series. */
export const withSlots = (plan: ImportPlan, slots: Slots, level?: LevelId): ImportPlan => {
  const { sets, ...item } = slots.item;
  return withSeries(withSetsRule(plan, sets), (series) => ({ ...series, slots: { ...slots, item } }), level);
};

const isRecord = (value: JsonValue | undefined): value is Readonly<Record<string, JsonValue>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/** Simple's state of the series `level` (with no `level`, the one series), or `null` when it has none. */
export const simpleOf = (plan: ImportPlan, level?: LevelId): Readonly<Record<string, JsonValue>> | null => {
  const kept = plan.simple?.[seriesOf(plan, level).id];
  return isRecord(kept) ? kept : null;
};

/** The plan with Simple's state of the series `level`; the other series' states stay. */
export const withSimple = (plan: ImportPlan, state: Readonly<Record<string, JsonValue>>, level?: LevelId): ImportPlan => ({
  ...plan,
  simple: { ...plan.simple, [seriesOf(plan, level).id]: state },
});
