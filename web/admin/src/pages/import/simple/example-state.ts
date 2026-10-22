/**
 * What Simple keeps in the plan's `simple`, which the server stores and never
 * reads. The marked example (`path`, `marks`) stays in `example`, the shape
 * both modes share (`../example.ts`); Simple keeps which answers the person
 * confirmed beside it, not inside it. Anything that does not read as this
 * shape is dropped, so a hand-edited plan never breaks a screen. A draft
 * saved before `simple` holds the same keys in `example`; `simpleRecordOf`
 * hands those over until the first save.
 */
import type { ImportPlan } from '../../../api/import-plan';
import { isRecord } from '../../../api/parse';
import { exampleOf, patternOf, simpleRecordOf, withExample, withoutOldAnswers } from '../example';
import type { Mark } from '../example';
import { seriesOf, withSeries } from '../series';

/** A list file the person said is not about these items. `source` is `''` in a draft that predates sources here: any source. */
export interface NotList {
  readonly source: string;
  readonly file: string;
}

export interface SimpleState {
  readonly marks: readonly Mark[];
  /** The pattern under which the person said every group is right; a new pattern asks again. */
  readonly groupsOk: string | null;
  /** The Check questions the person answered or took the suggested answer for. */
  readonly answered: readonly string[];
  /** The list files the person said are not about these items. */
  readonly notLists: readonly NotList[];
}

const strings = (value: unknown): readonly string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];

const notListOf = (value: unknown): NotList | null => {
  if (typeof value === 'string') {
    return { source: '', file: value };
  }
  return isRecord(value) && typeof value['source'] === 'string' && typeof value['file'] === 'string' ? { source: value['source'], file: value['file'] } : null;
};

/** Whether `entry` turns down the list `file` of `source`. */
export const declines = (entry: NotList, source: string, file: string): boolean => entry.file === file && (entry.source === '' || entry.source === source);

export const readState = (plan: ImportPlan): SimpleState => {
  const kept = simpleRecordOf(plan) ?? {};
  const groupsOk = kept['groupsOk'];
  const notLists = kept['notLists'];
  return {
    marks: exampleOf(plan)?.marks ?? [],
    groupsOk: typeof groupsOk === 'string' ? groupsOk : null,
    answered: strings(kept['answered']),
    notLists: Array.isArray(notLists) ? notLists.flatMap((entry) => notListOf(entry) ?? []) : [],
  };
};

export const withState = (plan: ImportPlan, change: Partial<Omit<SimpleState, 'marks'>>): ImportPlan => {
  const next = { ...readState(plan), ...change };
  const simple = { groupsOk: next.groupsOk, answered: [...next.answered], notLists: next.notLists.map(({ source, file }) => ({ source, file })) };
  // An old draft's keys inside `example` go with the first save of `simple`.
  return withSeries({ ...plan, simple }, (series) => ({ ...series, example: withoutOldAnswers(series.example) }));
};

/** The plan read by the marks: their pattern, and the example both modes read. */
export const withMarks = (plan: ImportPlan, marks: readonly Mark[], path: string | null): ImportPlan =>
  withSeries(withExample(plan, path ?? exampleOf(plan)?.path ?? null, marks), (series) => ({
    ...series,
    grouping: { ...series.grouping, pattern: patternOf(marks) },
  }));

/** Whether the plan's pattern is the one the marks make, or one typed in Advanced. */
export const typedElsewhere = (plan: ImportPlan): boolean => {
  const { pattern } = seriesOf(plan).grouping;
  return pattern !== null && pattern !== patternOf(readState(plan).marks);
};
