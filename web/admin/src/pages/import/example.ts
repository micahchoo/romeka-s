/**
 * The marked example as the plan keeps it (`plan.example`, admin state the
 * server never reads): the file the person marked and the parts they marked
 * in its name. The marks write the pattern (`grouping.pattern`), so the
 * pattern is theirs only while it is still the one the marks write; a pattern
 * typed by hand stays when a mark is removed.
 *
 * A mark is the words the person ticked at one place of the name, or the
 * place's text with each run of digits read as any number. `sep` is the
 * character before the place.
 *
 * Both modes read and write this one example. Simple keeps its answers
 * beside it in the same object (`simple/example-state.ts`), so every change
 * here sets `path` and `marks` and keeps the other keys.
 */
import type { ImportPlan } from '../../api/import-plan';
import { isRecord, isUnknownArray } from '../../api/parse';
import type { JsonValue } from '../../api/types';

export type Mark =
  | { readonly kind: 'words'; readonly words: readonly string[]; readonly sep: string }
  | { readonly kind: 'number'; readonly text: string; readonly sep: string };

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
  if (kind === 'number' && isText(raw['text'])) {
    return { kind, text: raw['text'], sep };
  }
  const words = raw['words'];
  if (kind === 'words' && isUnknownArray(words) && words.length > 0 && words.every(isText)) {
    return { kind, words, sep };
  }
  return null;
};

/** The example's marks, or `null` when the plan holds no example in this shape. */
export const exampleOf = (plan: ImportPlan): MarkedExample | null => {
  const raw = plan.example;
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

const escape = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const partOf = (mark: Mark): string =>
  mark.kind === 'number' ? escape(mark.text).replace(/\d+/g, '\\d+') : mark.words.map(escape).join('|');

/**
 * The pattern matches a file's whole path in its source, so it skips the
 * folders and the extension and reads the name between them: equal names
 * but for the extension are one item, and with marks, names that differ
 * only in a marked place.
 */
const FOLDERS = '^(?:[^/]*/)*';
const EXTENSION = '(?:\\.[^./]*)?$';

/** The pattern the marks write; with no mark, the one that groups by the name alone. */
export const patternOf = (marks: readonly Mark[]): string => {
  if (marks.length === 0) {
    return `${FOLDERS}(?<identifier>[^/]+?)${EXTENSION}`;
  }
  const seps = [...new Set(marks.map((mark) => mark.sep))];
  const sep = seps.length === 1 ? escape(seps[0] ?? '') : `[${seps.map(escape).join('')}]`;
  return `${FOLDERS}(?<identifier>[^/]+?)(?:${sep}(?<part>${marks.map(partOf).join('|')}))?${EXTENSION}`;
};

/** Whether the plan's pattern is still the one its marks write. */
export const patternIsMarked = (plan: ImportPlan, example: MarkedExample): boolean =>
  plan.grouping.pattern === patternOf(example.marks);

const marksJson = (marks: readonly Mark[]): JsonValue =>
  marks.map((mark) => (mark.kind === 'number' ? { kind: mark.kind, text: mark.text, sep: mark.sep } : { kind: mark.kind, words: [...mark.words], sep: mark.sep }));

/**
 * The plan's example with these marks, the other keys kept. With no mark the
 * path and the marks go, and an example left with nothing in it is `null`.
 */
export const exampleWith = (plan: ImportPlan, path: string | null, marks: readonly Mark[]): ImportPlan['example'] => {
  const kept = Object.fromEntries(Object.entries(plan.example ?? {}).filter(([key]) => key !== 'path' && key !== 'marks'));
  if (marks.length === 0) {
    return Object.keys(kept).length === 0 ? null : kept;
  }
  return { ...kept, ...(path === null ? {} : { path }), marks: marksJson(marks) };
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
  const pattern = patternIsMarked(plan, example) ? patternOf(marks) : plan.grouping.pattern;
  return { ...plan, example: exampleWith(plan, example.path, marks), grouping: { ...plan.grouping, pattern } };
};

/** The plan without its example; a pattern the example wrote goes back to grouping by the name alone. */
export const removeExample = (plan: ImportPlan): ImportPlan => {
  const example = exampleOf(plan);
  const marked = example !== null && patternIsMarked(plan, example);
  return { ...plan, example: exampleWith(plan, null, []), grouping: { ...plan.grouping, pattern: marked ? patternOf([]) : plan.grouping.pattern } };
};
