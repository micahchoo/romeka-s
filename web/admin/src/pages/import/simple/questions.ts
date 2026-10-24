/**
 * Which questions Check asks, counted from the preview: one per kind of
 * problem the server counted (a flag kind), one per anomaly with an answer in
 * the plan (identical files, files the example does not fit, items with no
 * row), and one for items the run adds with no title, whatever their item
 * type requires. Check lists them and the status line counts the open ones,
 * so the two numbers are one count. What each question says and how it is
 * answered is in `causes.ts`.
 */
import type { FlagKind, FlagPolicy, ImportPlan } from '../../../api/import-plan';
import { BLOCKING_FLAG_KINDS, FLAG_KINDS } from '../../../api/import-plan';
import type { PreviewOptions } from '../../../api/import-queries';
import type { FlagCount, FlagSample } from '../../../api/import-sections';
import { allLists } from '../series';
import { readState } from './example-state';

export type AnomalyId = 'identical' | 'unmatchedFiles' | 'itemsWithoutRow';
/** Items to add with no title: the server's count, not a flag, so it asks for every item type. */
export const UNTITLED = 'untitled';
export type CauseId = FlagKind | AnomalyId | typeof UNTITLED;

export const DEFAULT_POLICY: Readonly<Record<FlagKind, FlagPolicy>> = {
  'empty-required': 'keep',
  'date-form': 'keep',
  'space-at-ends': 'repair',
  'encoding-suspect': 'keep',
  'name-encoding': 'keep',
  'cells-count': 'leave',
  'no-file': 'keep',
  'not-media': 'leave',
  'listed-not-media': 'leave',
  'format-mismatch': 'keep',
  exists: 'leave',
  unmatched: 'leave',
  'many-matches': 'leave',
  'many-values': 'keep',
  'link-missing': 'keep',
  'unknown-type': 'keep',
  'unknown-set': 'keep',
  'unknown-kind': 'leave',
  'duplicate-row': 'leave',
  'unknown-term': 'leave',
  'template-missing': 'leave',
  'type-exists': 'leave',
  'encoding-unsupported': 'leave',
  'owner-not-allowed': 'leave',
};

/** Kinds the run handles without a question: the screen lists them under "We took care of". */
const HANDLED: readonly FlagKind[] = ['space-at-ends', 'not-media'];

/** Kinds Check states as a fact: the person has no choice about them, so no card asks. */
const FACTS: readonly FlagKind[] = ['listed-not-media'];

/** Proto order: problems of the list first, then of the files, then of the values. */
const CAUSE_ORDER: readonly CauseId[] = [
  'cells-count',
  'no-file',
  'itemsWithoutRow',
  'identical',
  'unmatchedFiles',
  'exists',
  'date-form',
  UNTITLED,
  'empty-required',
  'unknown-type',
  ...FLAG_KINDS.filter((kind) => !['cells-count', 'no-file', 'exists', 'date-form', 'empty-required', 'unknown-type'].includes(kind)),
];

export const isBlocking = (id: CauseId): boolean => BLOCKING_FLAG_KINDS.some((kind) => kind === id);

export const TITLE = 'dcterms:title';

/** An empty title the item type requires: the untitled question asks it, so no card of its own. */
const titleFlag = (flag: { readonly kind: FlagKind; readonly slot: string | null }): boolean => flag.kind === 'empty-required' && flag.slot === TITLE;

/** A kind the run takes care of under its default policy; with another policy it is a question again. */
const handledBy = (flag: FlagCount): boolean => HANDLED.some((kind) => kind === flag.kind && flag.policy === DEFAULT_POLICY[kind]);

/** The sections of the preview the questions are counted from. */
export const QUESTION_SECTIONS = ['flags', 'identical', 'unpaired', 'entries', 'check'] as const;

/** The query that counts them, with `examples` examples of each anomaly. `entries` counts the files no item takes. */
export const questionQuery = (examples: number): PreviewOptions => ({
  identical: { perPage: examples },
  unpaired: { rows: { perPage: 1 }, files: { perPage: examples } },
  entries: { reason: 'unmatched', perPage: examples },
});

export interface QuestionReads {
  readonly flags: readonly FlagCount[];
  readonly identical: { readonly files: number };
  readonly entries: { readonly total: number };
  readonly unpaired: { readonly items: number };
  readonly check: { readonly itemsWithoutTitle: number };
}

export interface CheckQuestion {
  readonly id: CauseId;
  readonly count: number;
  /** The flag kind a question about flagged records asks about. */
  readonly kind: FlagKind | null;
  /** The server's samples of a flag kind, over all its slots. */
  readonly samples: readonly FlagSample[];
}

/** Every question, in `CAUSE_ORDER`. */
export const questionsOf = (plan: ImportPlan, reads: QuestionReads): CheckQuestion[] => {
  const byKind = new Map<FlagKind, { count: number; samples: FlagSample[] }>();
  for (const flag of reads.flags) {
    if (flag.count === 0 || handledBy(flag) || FACTS.includes(flag.kind) || titleFlag(flag)) {
      continue;
    }
    const kept = byKind.get(flag.kind) ?? { count: 0, samples: [] };
    kept.count += flag.count;
    kept.samples.push(...flag.samples);
    byKind.set(flag.kind, kept);
  }
  const questions: CheckQuestion[] = [...byKind.entries()].map(([kind, found]) => ({ id: kind, count: found.count, kind, samples: found.samples }));
  const anomaly = (id: CauseId, count: number): void => {
    if (count > 0) {
      questions.push({ id, count, kind: null, samples: [] });
    }
  };
  anomaly('identical', reads.identical.files);
  anomaly('unmatchedFiles', reads.entries.total);
  anomaly('itemsWithoutRow', allLists(plan).length > 0 ? reads.unpaired.items : 0);
  anomaly(UNTITLED, reads.check.itemsWithoutTitle);
  return questions.sort((a, b) => CAUSE_ORDER.indexOf(a.id) - CAUSE_ORDER.indexOf(b.id));
};

/** The questions with no answer yet. A blocking one is not open: it must be fixed, not answered. */
export const openOf = <Q extends { readonly id: CauseId }>(plan: ImportPlan, questions: readonly Q[]): Q[] => {
  const { answered } = readState(plan);
  return questions.filter((question) => !isBlocking(question.id) && !answered.includes(question.id));
};

/** Facts Check lists after the questions: kinds the person has no choice about, and kinds the run takes care of. */
export const factsOf = (flags: readonly FlagCount[]): { readonly refused: FlagCount[]; readonly handled: FlagCount[] } => ({
  refused: flags.filter((flag) => flag.count > 0 && FACTS.includes(flag.kind)),
  handled: flags.filter((flag) => flag.count > 0 && handledBy(flag)),
});
