/**
 * What Simple keeps in the plan's `example`, which the server stores and
 * never reads. The marked example itself (`path`, `marks`) is the shape both
 * modes share (`../example.ts`); beside it Simple keeps which answers the
 * person confirmed. Anything that does not read as this shape is dropped, so
 * a hand-edited plan never breaks a screen.
 */
import type { ImportPlan } from '../../../api/import-plan';
import type { JsonValue } from '../../../api/types';
import { exampleOf, patternOf } from '../example';
import type { Mark } from '../example';

export interface SimpleState {
  readonly marks: readonly Mark[];
  /** The pattern under which the person said every group is right; a new pattern asks again. */
  readonly groupsOk: string | null;
  /** The Check questions the person answered or took the suggested answer for. */
  readonly answered: readonly string[];
  /** The list files the person said are not about these items. */
  readonly notLists: readonly string[];
}

const strings = (value: unknown): readonly string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];

export const readState = (plan: ImportPlan): SimpleState => {
  const example = plan.example ?? {};
  const groupsOk = example['groupsOk'];
  return {
    marks: exampleOf(plan)?.marks ?? [],
    groupsOk: typeof groupsOk === 'string' ? groupsOk : null,
    answered: strings(example['answered']),
    notLists: strings(example['notLists']),
  };
};

const marksJson = (marks: readonly Mark[]): JsonValue =>
  marks.map((mark) => (mark.kind === 'number' ? { kind: mark.kind, text: mark.text, sep: mark.sep } : { kind: mark.kind, words: [...mark.words], sep: mark.sep }));

export const withState = (plan: ImportPlan, change: Partial<Omit<SimpleState, 'marks'>>): ImportPlan => {
  const next = { ...readState(plan), ...change };
  return {
    ...plan,
    example: { ...(plan.example ?? {}), groupsOk: next.groupsOk, answered: [...next.answered], notLists: [...next.notLists] },
  };
};

/** The plan read by the marks: their pattern, and the example both modes read. */
export const withMarks = (plan: ImportPlan, marks: readonly Mark[], path: string | null): ImportPlan => {
  const kept = Object.fromEntries(Object.entries(plan.example ?? {}).filter(([key]) => key !== 'path' && key !== 'marks'));
  const place = path ?? exampleOf(plan)?.path ?? null;
  return {
    ...plan,
    grouping: { ...plan.grouping, pattern: patternOf(marks) },
    example: { ...kept, ...(place === null || marks.length === 0 ? {} : { path: place }), marks: marksJson(marks) },
  };
};

/** Whether the plan's pattern is the one the marks make, or one typed in Advanced. */
export const typedElsewhere = (plan: ImportPlan): boolean =>
  plan.grouping.pattern !== null && plan.grouping.pattern !== patternOf(readState(plan).marks);
