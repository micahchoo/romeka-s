/**
 * The questions Check asks: one per kind of problem the server counted (a
 * flag kind) and one per anomaly with an answer in the plan (identical
 * files, files the example does not fit, items with no row). Each has a
 * suggested answer, which is what the run does when the person gives none:
 * the flag kind's default policy, or the anomaly's default answer
 * (`docs/import-v2.md`, "Flags" and "Answers to anomalies").
 */
import type { AnomalyAnswers, FlagKind, FlagPolicy, ImportPlan, SlotRule } from '../../../api/import-plan';
import { BLOCKING_FLAG_KINDS, FLAG_KINDS } from '../../../api/import-plan';
import { exampleOf } from '../example';
import type { SimpleMessageKey } from './messages';

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
  'unknown-kind': 'leave',
  'duplicate-row': 'leave',
  'unknown-term': 'leave',
  'template-missing': 'leave',
  'type-exists': 'leave',
  'encoding-unsupported': 'leave',
};

/** Kinds the run handles without a question: the screen lists them under "We took care of". */
export const HANDLED: readonly FlagKind[] = ['space-at-ends', 'not-media'];

export type AnomalyId = 'identical' | 'unmatchedFiles' | 'itemsWithoutRow';
export type CauseId = FlagKind | AnomalyId;

export const DEFAULT_ANOMALIES: AnomalyAnswers = { identical: 'keepOne', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' };

/** Proto order: problems of the list first, then of the files, then of the values. */
export const CAUSE_ORDER: readonly CauseId[] = [
  'cells-count',
  'no-file',
  'itemsWithoutRow',
  'identical',
  'unmatchedFiles',
  'exists',
  'date-form',
  'empty-required',
  'unknown-type',
  ...FLAG_KINDS.filter((kind) => !['cells-count', 'no-file', 'exists', 'date-form', 'empty-required', 'unknown-type'].includes(kind)),
];

export interface Option {
  readonly value: string;
  readonly label: SimpleMessageKey;
}

export interface CauseWords {
  readonly say: SimpleMessageKey;
  readonly sayOne: SimpleMessageKey;
  readonly why: SimpleMessageKey | null;
  readonly options: readonly Option[];
}

const keepLeave = (keep: SimpleMessageKey, leave: SimpleMessageKey): readonly Option[] => [
  { value: 'keep', label: keep },
  { value: 'leave', label: leave },
];

const KEEP: Option = { value: 'keep', label: 'guided.check.keep' };
const LEAVE: Option = { value: 'leave', label: 'guided.check.leave' };

const ANOMALY_WORDS: Readonly<Record<AnomalyId, CauseWords>> = {
  itemsWithoutRow: {
    say: 'guided.check.noRow',
    sayOne: 'guided.check.noRowOne',
    why: 'guided.check.noRowWhy',
    options: keepLeave('guided.check.noRowKeep', 'guided.check.leave'),
  },
  identical: {
    say: 'guided.check.same',
    sayOne: 'guided.check.sameOne',
    why: 'guided.check.sameWhy',
    options: [
      { value: 'keepOne', label: 'guided.check.sameOneKept' },
      { value: 'keepAll', label: 'guided.check.sameBoth' },
    ],
  },
  unmatchedFiles: {
    say: 'guided.check.noFit',
    sayOne: 'guided.check.noFitOne',
    why: null,
    options: [
      { value: 'leave', label: 'guided.check.leave' },
      { value: 'own', label: 'guided.check.noFitOwn' },
    ],
  },
};

const FLAG_WORDS: Readonly<Partial<Record<FlagKind, CauseWords>>> = {
  'cells-count': {
    say: 'guided.check.cells',
    sayOne: 'guided.check.cellsOne',
    why: 'guided.check.cellsWhy',
    options: [
      { value: 'leave', label: 'guided.check.cellsLeave' },
      { value: 'keep', label: 'guided.check.cellsKeep' },
    ],
  },
  'no-file': { say: 'guided.check.noFile', sayOne: 'guided.check.noFileOne', why: 'guided.check.noFileWhy', options: keepLeave('guided.check.noFileKeep', 'guided.check.leave') },
  exists: {
    say: 'guided.check.known',
    sayOne: 'guided.check.knownOne',
    why: 'guided.check.knownWhy',
    options: [
      { value: 'leave', label: 'guided.check.knownLeave' },
      { value: 'keep', label: 'guided.check.knownKeep' },
    ],
  },
  'date-form': { say: 'guided.check.date', sayOne: 'guided.check.dateOne', why: 'guided.check.dateWhy', options: keepLeave('guided.check.dateKeep', 'guided.check.dateLeave') },
  'empty-required': {
    say: 'guided.check.noTitle',
    sayOne: 'guided.check.noTitleOne',
    why: null,
    options: [
      { value: 'keep', label: 'guided.check.noTitleKeep' },
      { value: 'filename', label: 'guided.check.noTitleName' },
    ],
  },
  // Shown only when Advanced turned the trim off, so the trim must be on offer again.
  'space-at-ends': {
    say: 'guided.check.other',
    sayOne: 'guided.check.otherOne',
    why: null,
    options: [{ value: 'repair', label: 'guided.check.spacesRepair' }, KEEP, LEAVE],
  },
  'unknown-type': { say: 'guided.check.type', sayOne: 'guided.check.typeOne', why: 'guided.check.typeWhy', options: [{ value: 'keep', label: 'guided.check.typeKeep' }] },
};

/** A kind with no words of its own: its default first, so the suggestion leads. */
const generic = (kind: FlagKind): CauseWords => ({
  say: 'guided.check.other',
  sayOne: 'guided.check.otherOne',
  why: null,
  options: DEFAULT_POLICY[kind] === 'leave' ? [LEAVE, KEEP] : [KEEP, LEAVE],
});

export const isBlocking = (id: CauseId): boolean => BLOCKING_FLAG_KINDS.some((kind) => kind === id);

const isAnomaly = (id: CauseId): id is AnomalyId => id === 'identical' || id === 'unmatchedFiles' || id === 'itemsWithoutRow';

/** The words of a cause; unmatched files only mention the marked example when the person marked one. */
export const wordsOf = (id: CauseId, plan: ImportPlan): CauseWords => {
  if (id === 'unmatchedFiles' && (exampleOf(plan)?.marks.length ?? 0) === 0) {
    return { ...ANOMALY_WORDS.unmatchedFiles, say: 'guided.check.noFitPlain', sayOne: 'guided.check.noFitPlainOne' };
  }
  return isAnomaly(id) ? ANOMALY_WORDS[id] : (FLAG_WORDS[id] ?? generic(id));
};

const TITLE = 'dcterms:title';
const fromName = (rule: SlotRule | undefined): boolean => rule?.sources.some((source) => source.from === 'file' && source.fact === 'name') === true;

const defaultPolicyOf = (plan: ImportPlan, kind: FlagKind): FlagPolicy =>
  kind === 'unmatched' ? (plan.action.unmatched === 'create' ? 'keep' : 'leave') : DEFAULT_POLICY[kind];

/** The answer the plan holds now; the suggested one when the person gave none. */
export const answerOf = (plan: ImportPlan, id: CauseId): string => {
  if (isAnomaly(id)) {
    return (plan.anomalies ?? DEFAULT_ANOMALIES)[id];
  }
  if (id === 'empty-required') {
    return fromName(plan.slots.item[TITLE]) ? 'filename' : 'keep';
  }
  return plan.flags[id] ?? defaultPolicyOf(plan, id);
};

/**
 * What the run does with no answer: the anomaly's default answer, or the
 * kind's default policy (`unmatched` follows the plan's `action.unmatched`).
 * "Keep" is the default for an empty title: the file name is an offer.
 */
export const suggestedOf = (plan: ImportPlan, id: CauseId): string => {
  if (isAnomaly(id)) {
    return DEFAULT_ANOMALIES[id];
  }
  if (id === 'empty-required') {
    return 'keep';
  }
  return defaultPolicyOf(plan, id);
};

const asPolicy = (value: string): FlagPolicy | null => (value === 'keep' || value === 'leave' || value === 'repair' ? value : null);

export const withAnswer = (plan: ImportPlan, id: CauseId, value: string): ImportPlan => {
  if (isAnomaly(id)) {
    const answers = plan.anomalies ?? DEFAULT_ANOMALIES;
    const next: AnomalyAnswers =
      id === 'identical'
        ? { ...answers, identical: value === 'keepAll' ? 'keepAll' : 'keepOne' }
        : id === 'unmatchedFiles'
          ? { ...answers, unmatchedFiles: value === 'own' ? 'own' : 'leave' }
          : { ...answers, itemsWithoutRow: value === 'leave' ? 'leave' : 'keep' };
    return { ...plan, anomalies: next };
  }
  if (id === 'empty-required') {
    // "Use the file name" is the title rule's last source: the file name where the list gives no title.
    const rule = plan.slots.item[TITLE] ?? { sources: [], combine: 'first' as const };
    const sources = rule.sources.filter((source) => !(source.from === 'file' && source.fact === 'name'));
    const next = { ...rule, sources: value === 'filename' ? [...sources, { from: 'file' as const, fact: 'name' as const }] : sources };
    return { ...plan, slots: { ...plan.slots, item: { ...plan.slots.item, [TITLE]: next } } };
  }
  const policy = asPolicy(value);
  return policy === null ? plan : { ...plan, flags: { ...plan.flags, [id]: policy } };
};
