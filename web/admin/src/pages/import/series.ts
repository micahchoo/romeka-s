/**
 * The series seam: the one place the import pages reach a series' members
 * (its pattern, marked example, lists and slot rules). Every other import
 * file reads them through `seriesOf` and writes them through `withSeries`,
 * so the switch of the plan the admin writes from v2 to v3 changes this
 * file, not its readers (`import-series.test.ts` holds them to it).
 *
 * The admin still writes v2. A v2 plan is one series, as the server lifts
 * it (`plan/structure.rs`): id `all`, scope `all`, no set, the plan's
 * `sheets` as its `lists`. One difference: the v2 `item.sets` rule stays in
 * the series' slots here, because Simple and Advanced still edit it there;
 * the lift moves it to a free set.
 */
import type { ImportPlan, LevelId, Series } from '../../api/import-plan';

/** The id of the one series a v2 plan is. */
export const ONE_SERIES: LevelId = 'all';

/** The series `level` of the plan; with one series, `level` may be left out. */
export const seriesOf = (plan: ImportPlan, level: LevelId = ONE_SERIES): Series => {
  if (level !== ONE_SERIES) {
    throw new Error(`The plan has no series "${level}".`);
  }
  return { kind: 'series', id: ONE_SERIES, set: null, scope: 'all', grouping: plan.grouping, example: plan.example, lists: plan.sheets, slots: plan.slots };
};

/**
 * The plan with `change` made to the series `level`. A change that returns
 * the series it was given leaves the plan itself, so a caller can tell a
 * no-op by identity. A v2 plan cannot hold a series' id, scope or set, so
 * a change to them is not written.
 */
export const withSeries = (plan: ImportPlan, change: (series: Series) => Series, level: LevelId = ONE_SERIES): ImportPlan => {
  const before = seriesOf(plan, level);
  const after = change(before);
  if (after === before) {
    return plan;
  }
  return { ...plan, grouping: after.grouping, example: after.example, sheets: after.lists, slots: after.slots };
};
