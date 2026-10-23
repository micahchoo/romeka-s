/**
 * What Simple keeps in the plan's `simple`, under the series' id, which the
 * server stores and never reads. The marked example (`path`, `marks`) stays in `example`, the shape
 * both modes share (`../example.ts`); Simple keeps which answers the person
 * confirmed beside it, not inside it. Anything that does not read as this
 * shape is dropped, so a hand-edited plan never breaks a screen. A draft
 * saved before `simple` holds the same keys in `example`; `simpleRecordOf`
 * hands those over until the first save.
 */
import type { ImportPlan } from '../../../api/import-plan';
import { isRecord } from '../../../api/parse';
import { simpleRecordOf, withoutOldAnswers } from '../example';
import { withEverySeries, withSimple } from '../series';

/** A list file the person said is not about these items. `source` is `''` in a draft that predates sources here: any source. */
export interface NotList {
  readonly source: string;
  readonly file: string;
}

export interface SimpleState {
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
  const notLists = kept['notLists'];
  return {
    answered: strings(kept['answered']),
    notLists: Array.isArray(notLists) ? notLists.flatMap((entry) => notListOf(entry) ?? []) : [],
  };
};

export const withState = (plan: ImportPlan, change: Partial<SimpleState>): ImportPlan => {
  const next = { ...readState(plan), ...change };
  const simple = { answered: [...next.answered], notLists: next.notLists.map(({ source, file }) => ({ source, file })) };
  // An old draft's keys inside `example` go with the first save of `simple`.
  return withEverySeries(withSimple(plan, simple), (series) => ({ ...series, example: withoutOldAnswers(series.example) }));
};
