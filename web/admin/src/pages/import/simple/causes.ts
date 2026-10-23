/**
 * What each Check question says and how it is answered (which questions
 * there are is `questions.ts`). Each has a suggested answer, which is what
 * the run does when the person gives none: the flag kind's default policy,
 * or the anomaly's default answer (`docs/import-v2.md`, "Flags" and
 * "Answers to anomalies").
 */
import type { AnomalyAnswers, FlagKind, FlagPolicy, ImportPlan, SlotRule } from '../../../api/import-plan';
import type { PatternState } from '../../../api/import-model';
import type { BucketCount, Grain, ListCandidate } from '../../../api/import-sections';
import { exampleOf } from '../example';
import { allLists, allSeries, onlySeries, withEverySeries } from '../series';
import { readState } from './example-state';
import { withFileRows } from './list-plan';
import { folderSheets } from './list-read';
import type { SimpleMessageKey } from './messages';
import { DEFAULT_POLICY, TITLE, UNTITLED } from './questions';
import type { AnomalyId, CauseId } from './questions';

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
 * A plan of several series asks it of each folder on the Structure screen
 * ("Each row describes"), so Check does not.
 */
export const fileRowsOf = (plan: ImportPlan, candidates: readonly ListCandidate[]): FileRows | null => {
  const one = onlySeries(plan);
  if (one === null) {
    return null;
  }
  for (const sheet of folderSheets(plan, one.id)) {
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

export const DEFAULT_ANOMALIES: AnomalyAnswers = { identical: 'keepOne', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' };

export interface Option {
  readonly value: string;
  readonly label: SimpleMessageKey;
}

export interface CauseWords {
  /** The cause's short name in the list of questions. */
  readonly name: SimpleMessageKey | `problem.${FlagKind}`;
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
    name: 'guided.check.nameNoRow',
    say: 'guided.check.noRow',
    sayOne: 'guided.check.noRowOne',
    why: 'guided.check.noRowWhy',
    options: keepLeave('guided.check.noRowKeep', 'guided.check.leave'),
  },
  identical: {
    name: 'guided.check.nameSame',
    say: 'guided.check.same',
    sayOne: 'guided.check.sameOne',
    why: 'guided.check.sameWhy',
    options: [
      { value: 'keepOne', label: 'guided.check.sameOneKept' },
      { value: 'keepAll', label: 'guided.check.sameBoth' },
    ],
  },
  unmatchedFiles: {
    name: 'guided.check.nameNoFit',
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
    name: 'problem.cells-count',
    say: 'guided.check.cells',
    sayOne: 'guided.check.cellsOne',
    why: 'guided.check.cellsWhy',
    options: [
      { value: 'leave', label: 'guided.check.cellsLeave' },
      { value: 'keep', label: 'guided.check.cellsKeep' },
    ],
  },
  'no-file': { name: 'problem.no-file', say: 'guided.check.noFile', sayOne: 'guided.check.noFileOne', why: 'guided.check.noFileWhy', options: keepLeave('guided.check.noFileKeep', 'guided.check.leave') },
  exists: {
    name: 'problem.exists',
    say: 'guided.check.known',
    sayOne: 'guided.check.knownOne',
    why: 'guided.check.knownWhy',
    options: [
      { value: 'leave', label: 'guided.check.knownLeave' },
      { value: 'keep', label: 'guided.check.knownKeep' },
    ],
  },
  'date-form': { name: 'problem.date-form', say: 'guided.check.date', sayOne: 'guided.check.dateOne', why: 'guided.check.dateWhy', options: keepLeave('guided.check.dateKeep', 'guided.check.dateLeave') },
  // Shown only when Advanced turned the trim off, so the trim must be on offer again.
  'space-at-ends': {
    name: 'problem.space-at-ends',
    say: 'guided.check.other',
    sayOne: 'guided.check.otherOne',
    why: null,
    options: [{ value: 'repair', label: 'guided.check.spacesRepair' }, KEEP, LEAVE],
  },
  'unknown-type': { name: 'problem.unknown-type', say: 'guided.check.type', sayOne: 'guided.check.typeOne', why: 'guided.check.typeWhy', options: [{ value: 'keep', label: 'guided.check.typeKeep' }] },
};

/** A kind with no words of its own: its default first, so the suggestion leads. */
const UNTITLED_WORDS: CauseWords = {
  name: 'guided.check.nameUntitled',
  say: 'guided.check.untitled',
  sayOne: 'guided.check.untitledOne',
  why: null,
  options: [
    { value: 'keep', label: 'guided.check.noTitleKeep' },
    { value: 'identifier', label: 'guided.check.noTitleCode' },
    { value: 'filename', label: 'guided.check.noTitleName' },
  ],
};

/** Every list describes files: a row that names no file of the import makes nothing, so leaving it out is the only answer. */
const rowsDescribeFiles = (plan: ImportPlan): boolean => {
  const lists = allLists(plan);
  return lists.length > 0 && lists.every((sheet) => sheet.rowKind === 'media');
};

const generic = (kind: FlagKind): CauseWords => ({
  name: `problem.${kind}`,
  say: 'guided.check.other',
  sayOne: 'guided.check.otherOne',
  why: null,
  options: DEFAULT_POLICY[kind] === 'leave' ? [LEAVE, KEEP] : [KEEP, LEAVE],
});

const isAnomaly = (id: CauseId): id is AnomalyId => id === 'identical' || id === 'unmatchedFiles' || id === 'itemsWithoutRow';

/**
 * The words of a cause; unmatched files only mention the marked example when
 * the person marked one, and two rows for one item offer first that each row
 * describes one file where the list looks like it does.
 */
export const wordsOf = (id: CauseId, plan: ImportPlan, facts: CheckFacts): CauseWords => {
  const one = onlySeries(plan);
  if (id === 'unmatchedFiles' && (one === null ? 0 : (exampleOf(plan, one.id)?.marks.length ?? 0)) === 0) {
    return { ...ANOMALY_WORDS.unmatchedFiles, say: 'guided.check.noFitPlain', sayOne: 'guided.check.noFitPlainOne' };
  }
  if (id === 'duplicate-row' && facts.fileRows !== null) {
    return { ...generic(id), why: 'guided.check.fileRowsWhy', options: [FILE_ROWS, KEEP, LEAVE] };
  }
  if (id === UNTITLED) {
    return UNTITLED_WORDS;
  }
  if (id === 'no-file' && rowsDescribeFiles(plan)) {
    return { ...(FLAG_WORDS[id] ?? generic(id)), options: [LEAVE] };
  }
  return isAnomaly(id) ? ANOMALY_WORDS[id] : (FLAG_WORDS[id] ?? generic(id));
};

const fromName = (rule: SlotRule | undefined): boolean => rule?.sources.some((source) => source.from === 'file' && source.fact === 'name') === true;
const isCode = (source: SlotRule['sources'][number]): boolean => source.from === 'group' && source.name === 'identifier';
const fromCode = (rule: SlotRule | undefined): boolean => rule?.sources.some(isCode) === true;

/** The untitled question is asked of the whole import, so its answer is the title rule of every series. */
const everyTitle = (plan: ImportPlan, test: (rule: SlotRule | undefined) => boolean): boolean => {
  const series = allSeries(plan);
  return series.length > 0 && series.every((one) => test(one.slots.item[TITLE]));
};

/** Every series titles its items by their code. */
export const titledByCode = (plan: ImportPlan): boolean => everyTitle(plan, fromCode);

const defaultPolicyOf = (plan: ImportPlan, kind: FlagKind): FlagPolicy => {
  if (kind === 'unmatched') {
    return plan.action.unmatched === 'create' ? 'keep' : 'leave';
  }
  return kind === 'no-file' && rowsDescribeFiles(plan) ? 'leave' : DEFAULT_POLICY[kind];
};

/**
 * The answer the plan holds now; the suggested one when the person gave none.
 * An empty title has no flag policy to hold the answer: it is the title rule's
 * last source, and with no rule yet the item code shows as chosen where it fits.
 */
export const answerOf = (plan: ImportPlan, id: CauseId, facts: CheckFacts): string => {
  if (isAnomaly(id)) {
    return (plan.anomalies ?? DEFAULT_ANOMALIES)[id];
  }
  if (id === UNTITLED) {
    if (everyTitle(plan, fromName)) {
      return 'filename';
    }
    if (titledByCode(plan)) {
      return 'identifier';
    }
    return facts.itemCodeFits && !readState(plan).answered.includes(id) ? 'identifier' : 'keep';
  }
  if (id === 'duplicate-row' && facts.fileRows !== null && plan.flags[id] === undefined) {
    return FILE_ROWS.value;
  }
  if (id === 'no-file' && rowsDescribeFiles(plan)) {
    return 'leave';
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
  if (id === UNTITLED) {
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
    // Check counts no proposal, so no roll-up pick: every column goes on the item, where nothing is lost.
    return facts?.fileRows == null ? plan : withFileRows(plan, facts.fileRows.sheet, []);
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
  if (id === UNTITLED) {
    // The item code or the file name is the title rule's last source: it applies where the list gives no title.
    const last = value === 'filename' ? [{ from: 'file' as const, fact: 'name' as const }] : value === 'identifier' ? [{ from: 'group' as const, name: 'identifier' }] : [];
    return withEverySeries(plan, (series) => {
      const rule = series.slots.item[TITLE] ?? { sources: [], combine: 'first' as const };
      const sources = rule.sources.filter((source) => !isCode(source) && !(source.from === 'file' && source.fact === 'name'));
      return { ...series, slots: { ...series.slots, item: { ...series.slots.item, [TITLE]: { ...rule, sources: [...sources, ...last] } } } };
    });
  }
  const policy = asPolicy(value);
  return policy === null ? plan : { ...plan, flags: { ...plan.flags, [id]: policy } };
};

/** Causes whose suggested answer is a plan edit: the run follows it only once it is written. */
const PLAN_EDITS: readonly CauseId[] = [UNTITLED, 'duplicate-row'];

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
      const writes = id === UNTITLED ? answerOf(done.plan, id, facts) === 'identifier' && !titledByCode(done.plan) : answerOf(done.plan, id, facts) === FILE_ROWS.value;
      return writes ? { plan: withAnswer(done.plan, id, suggestedOf(done.plan, id, facts), facts), applied: [...done.applied, id] } : done;
    },
    { plan, applied: [] },
  );
