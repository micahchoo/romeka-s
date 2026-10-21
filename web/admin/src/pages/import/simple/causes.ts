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
import type { PatternState } from '../../../api/import-model';
import type { BucketCount, Grain, ListCandidate } from '../../../api/import-sections';
import { exampleOf } from '../example';
import { readState } from './example-state';
import { withFileRows } from './list-plan';
import { folderSheets } from './list-read';
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

/** Kinds Check states as a fact: the person has no choice about them, so no card asks. */
export const FACTS: readonly FlagKind[] = ['listed-not-media'];

/** What the preview says about the items, for the answers that depend on it. */
export interface CheckFacts {
  /** The pattern has an `identifier` group and some item has more than one file. */
  readonly itemCodeFits: boolean;
  /** A list whose rows look like they describe files, read as one row per item. */
  readonly fileRows: FileRows | null;
}

export interface FileRows {
  readonly sheet: string;
  readonly grain: Grain;
}

/**
 * The first list in use that pairs rows with items by file name, one item a
 * row, while some of its rows name another file of an item a row already
 * names: there "Two rows for one item" is the shape of the list, not a fault.
 */
export const fileRowsOf = (plan: ImportPlan, candidates: readonly ListCandidate[]): FileRows | null => {
  for (const sheet of folderSheets(plan)) {
    const grain = candidates.find((candidate) => candidate.inPlan === sheet.id)?.grain ?? null;
    if (sheet.rowKind === 'item' && sheet.join?.on === 'fileName' && grain !== null && grain.rowsNamingOtherFilesOfAGroup > 0) {
      return { sheet: sheet.id, grain };
    }
  }
  return null;
};

/** Whether the item code fits as the title: the pattern has an `identifier` group and some item has more than one file. */
export const itemCodeFits = (pattern: PatternState, histogram: readonly BucketCount[]): boolean =>
  typeof pattern === 'object' && 'groups' in pattern && pattern.groups.includes('identifier') && histogram.some((bucket) => bucket.files !== '0' && bucket.files !== '1' && bucket.count > 0);

/** The facts both Check and Add read from one preview. */
export const checkFactsOf = (plan: ImportPlan, read: { readonly pattern: PatternState; readonly histogram: readonly BucketCount[]; readonly listCandidates: readonly ListCandidate[] }): CheckFacts => ({
  itemCodeFits: itemCodeFits(read.pattern, read.histogram),
  fileRows: fileRowsOf(plan, read.listCandidates),
});

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
const FILE_ROWS: Option = { value: 'fileRows', label: 'guided.check.fileRows' };

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
      { value: 'identifier', label: 'guided.check.noTitleCode' },
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

/**
 * The words of a cause; unmatched files only mention the marked example when
 * the person marked one, and two rows for one item offer first that each row
 * describes one file where the list looks like it does.
 */
export const wordsOf = (id: CauseId, plan: ImportPlan, facts: CheckFacts): CauseWords => {
  if (id === 'unmatchedFiles' && (exampleOf(plan)?.marks.length ?? 0) === 0) {
    return { ...ANOMALY_WORDS.unmatchedFiles, say: 'guided.check.noFitPlain', sayOne: 'guided.check.noFitPlainOne' };
  }
  if (id === 'duplicate-row' && facts.fileRows !== null) {
    return { ...generic(id), why: 'guided.check.fileRowsWhy', options: [FILE_ROWS, KEEP, LEAVE] };
  }
  return isAnomaly(id) ? ANOMALY_WORDS[id] : (FLAG_WORDS[id] ?? generic(id));
};

const TITLE = 'dcterms:title';
const fromName = (rule: SlotRule | undefined): boolean => rule?.sources.some((source) => source.from === 'file' && source.fact === 'name') === true;
const isCode = (source: SlotRule['sources'][number]): boolean => source.from === 'group' && source.name === 'identifier';
export const fromCode = (rule: SlotRule | undefined): boolean => rule?.sources.some(isCode) === true;

const defaultPolicyOf = (plan: ImportPlan, kind: FlagKind): FlagPolicy =>
  kind === 'unmatched' ? (plan.action.unmatched === 'create' ? 'keep' : 'leave') : DEFAULT_POLICY[kind];

/**
 * The answer the plan holds now; the suggested one when the person gave none.
 * An empty title has no flag policy to hold the answer: it is the title rule's
 * last source, and with no rule yet the item code shows as chosen where it fits.
 */
export const answerOf = (plan: ImportPlan, id: CauseId, facts: CheckFacts): string => {
  if (isAnomaly(id)) {
    return (plan.anomalies ?? DEFAULT_ANOMALIES)[id];
  }
  if (id === 'empty-required') {
    const rule = plan.slots.item[TITLE];
    if (fromName(rule)) {
      return 'filename';
    }
    if (fromCode(rule)) {
      return 'identifier';
    }
    return facts.itemCodeFits && !readState(plan).answered.includes(id) ? 'identifier' : 'keep';
  }
  if (id === 'duplicate-row' && facts.fileRows !== null && plan.flags[id] === undefined) {
    return FILE_ROWS.value;
  }
  return plan.flags[id] ?? defaultPolicyOf(plan, id);
};

/**
 * What the run does with no answer: the anomaly's default answer, or the
 * kind's default policy (`unmatched` follows the plan's `action.unmatched`).
 * An empty title is the exception: the run adds no title unless a rule gives
 * one, so the item code is only suggested, and written when the person takes
 * the suggestion. Where the code does not fit, "keep" is suggested.
 */
export const suggestedOf = (plan: ImportPlan, id: CauseId, facts: CheckFacts): string => {
  if (isAnomaly(id)) {
    return DEFAULT_ANOMALIES[id];
  }
  if (id === 'empty-required') {
    return facts.itemCodeFits ? 'identifier' : 'keep';
  }
  if (id === 'duplicate-row' && facts.fileRows !== null) {
    return FILE_ROWS.value;
  }
  return defaultPolicyOf(plan, id);
};

const asPolicy = (value: string): FlagPolicy | null => (value === 'keep' || value === 'leave' || value === 'repair' ? value : null);

export const withAnswer = (plan: ImportPlan, id: CauseId, value: string, facts?: CheckFacts): ImportPlan => {
  if (id === 'duplicate-row' && value === FILE_ROWS.value) {
    return facts?.fileRows == null ? plan : withFileRows(plan, facts.fileRows.sheet, facts.fileRows.grain);
  }
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
    // The item code or the file name is the title rule's last source: it applies where the list gives no title.
    const rule = plan.slots.item[TITLE] ?? { sources: [], combine: 'first' as const };
    const sources = rule.sources.filter((source) => !isCode(source) && !(source.from === 'file' && source.fact === 'name'));
    const last = value === 'filename' ? [{ from: 'file' as const, fact: 'name' as const }] : value === 'identifier' ? [{ from: 'group' as const, name: 'identifier' }] : [];
    const next = { ...rule, sources: [...sources, ...last] };
    return { ...plan, slots: { ...plan.slots, item: { ...plan.slots.item, [TITLE]: next } } };
  }
  const policy = asPolicy(value);
  return policy === null ? plan : { ...plan, flags: { ...plan.flags, [id]: policy } };
};

/** Causes whose suggested answer is a plan edit: the run follows it only once it is written. */
const PLAN_EDITS: readonly CauseId[] = ['empty-required', 'duplicate-row'];

export interface Suggested {
  readonly plan: ImportPlan;
  /** The causes whose suggestion this wrote, in `PLAN_EDITS` order. */
  readonly applied: readonly CauseId[];
}

/**
 * The suggestions among `open` that the run would not follow unless they are
 * written: the item code as the title, each row as one file. Check writes them
 * when the person takes every suggestion; Add writes them for a person who
 * never answered, so Check's "we use its suggested answer" holds there too.
 */
export const withSuggestions = (plan: ImportPlan, open: readonly CauseId[], facts: CheckFacts): Suggested =>
  PLAN_EDITS.filter((id) => open.includes(id)).reduce<Suggested>(
    (done, id) => {
      const writes = id === 'empty-required' ? answerOf(done.plan, id, facts) === 'identifier' && !fromCode(done.plan.slots.item[TITLE]) : answerOf(done.plan, id, facts) === FILE_ROWS.value;
      return writes ? { plan: withAnswer(done.plan, id, suggestedOf(done.plan, id, facts), facts), applied: [...done.applied, id] } : done;
    },
    { plan, applied: [] },
  );
