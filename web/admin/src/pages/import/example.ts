/**
 * The marked example as the plan keeps it (`plan.example`, admin state the
 * server never reads): the file the person marked and the parts they marked
 * in its name. The marks write the pattern (`grouping.pattern`), so the
 * pattern is theirs only while it is still the one the marks write; a pattern
 * typed by hand stays when a mark is removed.
 *
 * A mark is the words the person ticked at one place of the name, or the
 * place's text with each run of digits read as any number, or with each run
 * of letters read as any letters. `sep` is the character before the place:
 * `''` when the place is glued to a number before it (`010a`).
 *
 * Both modes read and write this one example. Simple keeps its answers in
 * the plan's `simple`, under the series' id (`simple/example-state.ts`). A
 * draft saved before that member holds them in this object, so a change
 * here moves them across.
 */
import type { ImportPlan, Series } from '../../api/import-plan';
import { isRecord, isUnknownArray } from '../../api/parse';
import type { JsonValue } from '../../api/types';
import { onlySeries, seriesOf, simpleOf, withSeries, withSimple } from './series';

export type Mark =
  | { readonly kind: 'words'; readonly words: readonly string[]; readonly sep: string }
  | { readonly kind: 'number' | 'letters'; readonly text: string; readonly sep: string };

export interface MarkedExample {
  /** The marked file's path in its source. */
  readonly path: string | null;
  readonly marks: readonly Mark[];
}

const isText = (value: unknown): value is string => typeof value === 'string';

const markOf = (raw: unknown): Mark | null => {
  if (!isRecord(raw)) {
    return null;
  }
  const { kind, sep } = raw;
  if (!isText(sep)) {
    return null;
  }
  if ((kind === 'number' || kind === 'letters') && isText(raw['text'])) {
    return { kind, text: raw['text'], sep };
  }
  const words = raw['words'];
  if (kind === 'words' && isUnknownArray(words) && words.length > 0 && words.every(isText)) {
    return { kind, words, sep };
  }
  return null;
};

/** The series' example, or `null` when it holds none in this shape. */
const exampleIn = (series: Series | null): MarkedExample | null => {
  const raw = series?.example ?? null;
  if (raw === null) {
    return null;
  }
  const marks = raw['marks'];
  if (!isUnknownArray(marks)) {
    return null;
  }
  const parsed = marks.map(markOf);
  if (!parsed.every((mark): mark is Mark => mark !== null)) {
    return null;
  }
  const path = raw['path'];
  return { path: isText(path) ? path : null, marks: parsed };
};

/** The example's marks, or `null` when the plan holds no example in this shape, or has several series, each with its own. */
export const exampleOf = (plan: ImportPlan): MarkedExample | null => exampleIn(onlySeries(plan));

const escape = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const LETTER = '[\\p{L}\\p{M}]';
const LETTERS = `${LETTER}+`;

const partOf = (mark: Mark): string => {
  switch (mark.kind) {
    case 'number':
      return escape(mark.text).replace(/\d+/g, '\\d+');
    case 'letters':
      return escape(mark.text).replace(new RegExp(LETTERS, 'gu'), LETTERS);
    case 'words':
      return mark.words.map(escape).join('|');
  }
};

/**
 * The pattern matches a file's whole path in its source, so it skips the
 * folders and the extension and reads the name between them: equal names
 * but for the extension are one item, and with marks, names that differ
 * only in a marked place.
 */
const FOLDERS = '^(?:[^/]*/)*';
const EXTENSION = '(?:\\.[^./]*)?$';

/**
 * With a glued mark, the identifier must end in a digit for the part to
 * follow it with no separator. Neither the server's `regex` crate nor the
 * shared groups allow a lookbehind or a second `part` group, so the
 * identifier tries an ending in a digit first, and else ends in any
 * character but a dot and the whole run of letters after it: no letter is
 * left for a glued part ("Rebecca" stays whole), and the run never starts at
 * the extension.
 */
const GLUED_IDENTIFIER = `[^/]*?\\d|[^/]*?[^/.]${LETTER}*`;

/** The separator before the marked place; optional when a mark is glued. */
const sepOf = (marks: readonly Mark[]): string => {
  const seps = [...new Set(marks.map((mark) => mark.sep).filter((sep) => sep !== ''))];
  if (seps.length === 0) {
    return '';
  }
  const sep = seps.length === 1 ? escape(seps[0] ?? '') : `[${seps.map(escape).join('')}]`;
  return marks.some((mark) => mark.sep === '') ? `(?:${sep})?` : sep;
};

/** The pattern the marks write; with no mark, the one that groups by the name alone. */
export const patternOf = (marks: readonly Mark[]): string => {
  if (marks.length === 0) {
    return `${FOLDERS}(?<identifier>[^/]+?)${EXTENSION}`;
  }
  const identifier = marks.some((mark) => mark.sep === '') ? GLUED_IDENTIFIER : '[^/]+?';
  return `${FOLDERS}(?<identifier>${identifier})(?:${sepOf(marks)}(?<part>${marks.map(partOf).join('|')}))?${EXTENSION}`;
};

/** Whether the plan's pattern is still the one its marks write. */
export const patternIsMarked = (plan: ImportPlan, example: MarkedExample): boolean =>
  seriesOf(plan).grouping.pattern === patternOf(example.marks);

/** The keys Simple's answers had inside `example`, before `plan.simple`. */
const LEGACY_KEYS = ['groupsOk', 'answered', 'notLists'] as const;

/** Simple's answers for the series as the plan holds them: its state in `simple`, or else the old keys of `example`, or `null`. */
export const simpleRecordOf = (plan: ImportPlan): Readonly<Record<string, JsonValue>> | null => {
  const kept = simpleOf(plan);
  if (kept !== null) {
    return kept;
  }
  const example = onlySeries(plan)?.example ?? null;
  const old = Object.fromEntries(LEGACY_KEYS.flatMap((key) => (example?.[key] === undefined ? [] : [[key, example[key]] as const])));
  return Object.keys(old).length === 0 ? null : old;
};

/** The plan's example without Simple's old answers; `null` when nothing else is in it. */
export const withoutOldAnswers = (example: Series['example']): Series['example'] => {
  const rest = Object.fromEntries(Object.entries(example ?? {}).filter(([key]) => !LEGACY_KEYS.some((legacy) => legacy === key)));
  return Object.keys(rest).length === 0 ? null : rest;
};

/** Whether `example` holds something besides Simple's old answers, which is what Advanced reports as kept. */
export const exampleKept = (plan: ImportPlan): boolean => {
  const { example } = seriesOf(plan);
  return example !== null && Object.keys(example).some((key) => !LEGACY_KEYS.some((legacy) => legacy === key));
};

const marksJson = (marks: readonly Mark[]): JsonValue =>
  marks.map((mark) => (mark.kind === 'words' ? { kind: mark.kind, words: [...mark.words], sep: mark.sep } : { kind: mark.kind, text: mark.text, sep: mark.sep }));

/**
 * The plan with this example: the path and the marks, nothing else. With no
 * mark the example is `null`. Simple's answers an old draft kept inside it
 * move to `simple` first.
 */
export const withExample = (plan: ImportPlan, path: string | null, marks: readonly Mark[]): ImportPlan => {
  const simple = simpleRecordOf(plan);
  const moved: ImportPlan = simple === null ? plan : withSimple(plan, simple);
  return withSeries(moved, (series) => ({ ...series, example: marks.length === 0 ? null : { ...(path === null ? {} : { path }), marks: marksJson(marks) } }));
};

/**
 * The plan without mark `index`. The pattern follows the marks when it was
 * theirs; the last mark gone, the example goes too and the pattern groups
 * by the name alone.
 */
export const removeMark = (plan: ImportPlan, index: number): ImportPlan => {
  const example = exampleOf(plan);
  if (example === null || example.marks[index] === undefined) {
    return plan;
  }
  const marks = example.marks.filter((_, at) => at !== index);
  const pattern = patternIsMarked(plan, example) ? patternOf(marks) : seriesOf(plan).grouping.pattern;
  return withSeries(withExample(plan, example.path, marks), (series) => ({ ...series, grouping: { ...series.grouping, pattern } }));
};

/** The plan without its example; a pattern the example wrote goes back to grouping by the name alone. */
export const removeExample = (plan: ImportPlan): ImportPlan => {
  const example = exampleOf(plan);
  const marked = example !== null && patternIsMarked(plan, example);
  return withSeries(withExample(plan, null, []), (series) => ({ ...series, grouping: { ...series.grouping, pattern: marked ? patternOf([]) : series.grouping.pattern } }));
};

/**
 * One series of several with one more mark, on the file `path`: its example
 * and the pattern the marks write. The Structure screen writes it by hand.
 */
export const withMarkOn = (series: Series, path: string, mark: Mark): Series => {
  const marks = [...(exampleIn(series)?.marks ?? []), mark];
  return { ...series, example: { path, marks: marksJson(marks) }, grouping: { ...series.grouping, pattern: patternOf(marks) } };
};
