/**
 * The Structure screen's model: the proposal (`api/import-proposal.ts`) as
 * the rows of the tree, the totals of the count line and the state of the
 * status line, and each count in words. Every number comes from the
 * proposal as the server counted it; nothing here adds a count of its own
 * but the totals, which sum the series' chosen counts.
 */
import type { FolderScope, ImportPlan, ListFile, PairRule, RecordKind, Series, SetFrom, SetSource, SheetPlan, Structure } from '../../../api/import-plan';
import type { ColumnRollUp, ItemCut, KeptList, KeptSetNode, NodeEvidence, OutsideFolder, OwnNode, Pick, Proposal, RollUp, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { BucketCount, ListCandidate, MediaKind, Varies } from '../../../api/import-sections';
import { useTranslator } from '../../../i18n';
import type { KeyOf, MessageKey, Translator } from '../../../i18n';
import { archiveImport } from '../../../i18n/namespaces/import';
import { importLists } from '../../../i18n/namespaces/import-lists';
import { importStructure } from '../../../i18n/namespaces/import-structure';
import { BUCKET_WORDS } from '../lists/facts-words';
import type { ListsMessageKey } from '../lists/messages';
import type { ImportMessageKey } from '../messages';
import { isAsFound, seriesOfTree, withStructure } from '../series';
import { fileLevelAllowed, levelOf } from './list-plan';

export type StructureMessageKey = KeyOf<typeof importStructure>;
export type StructureTranslator = Translator<MessageKey | ImportMessageKey | ListsMessageKey | StructureMessageKey>;
type T = StructureTranslator['t'];

export const useStructureText = (): StructureTranslator => useTranslator(importStructure, archiveImport, importLists);

/** One line of the tree. Its `key` names it while the proposal changes under it. */
export type TreeNode =
  | { readonly kind: 'archive'; readonly key: 'archive' }
  | { readonly kind: 'every'; readonly key: 'every'; readonly offers: readonly Pick<SetFrom, SetCounts>[] }
  | { readonly kind: 'set'; readonly key: string; readonly node: Extract<NodeEvidence, { kind: 'set' }> }
  | { readonly kind: 'series'; readonly key: string; readonly node: SeriesNode }
  /** A series written by hand, with the lists it reads as written. Its key is the one it had as a suggestion, so it stays selected. */
  | { readonly kind: 'own'; readonly key: string; readonly node: OwnNode; readonly lists: readonly ListFile[] }
  /** A set level the plan keeps around a series written by hand, as the proposal sends it, with that series' folder and lists. */
  | { readonly kind: 'kept'; readonly key: string; readonly node: KeptSetNode; readonly scope: FolderScope; readonly lists: readonly ListFile[] }
  | { readonly kind: 'outside'; readonly key: string; readonly folder: OutsideFolder }
  | { readonly kind: 'unused'; readonly key: string; readonly list: ListCandidate };

const seriesNodes = (proposal: Proposal): readonly SeriesNode[] =>
  proposal.nodes.flatMap((node) => (node.kind === 'series' ? [node] : []));

/** The lists a series written by hand reads, as the proposal holds it. */
const writtenLists = (proposal: Proposal, level: string): readonly ListFile[] =>
  (writtenIn(proposal.structure, level)?.lists ?? []).map(({ source, file }) => ({ source, file }));

/** The series a kept set level holds: the next written series, as the server sends a kept level just before its series. */
const keptAround = (nodes: readonly NodeEvidence[], at: number): Extract<NodeEvidence, { kind: 'own' }> | undefined =>
  nodes.slice(at + 1).find((node): node is Extract<NodeEvidence, { kind: 'own' }> => node.kind === 'own');

/** The archive, the sets over every item when any is offered, the set levels and the series; then the folders not imported and the lists not used. */
export const treeOf = (proposal: Proposal): readonly TreeNode[] => [
  { kind: 'archive', key: 'archive' },
  ...(proposal.topSets.length === 0 ? [] : [{ kind: 'every', key: 'every', offers: proposal.topSets } as const]),
  ...proposal.nodes.flatMap((node, at): readonly TreeNode[] => {
    switch (node.kind) {
      case 'set':
        return [{ kind: 'set', key: `set:${node.level}`, node }];
      case 'kept': {
        const around = keptAround(proposal.nodes, at);
        return around === undefined ? [] : [{ kind: 'kept', key: `set:${node.level}`, node, scope: around.scope.scope, lists: writtenLists(proposal, around.level) }];
      }
      case 'series':
        return [{ kind: 'series', key: `series:${node.level}`, node }];
      case 'own':
        return [{ kind: 'own', key: `series:${node.level}`, node, lists: writtenLists(proposal, node.level) }];
    }
  }),
  ...proposal.outside.map((folder): TreeNode => ({ kind: 'outside', key: `outside:${folder.scope.source}/${folder.scope.folder}`, folder })),
  ...proposal.unattached.map((list): TreeNode => ({ kind: 'unused', key: `unused:${list.source}/${list.file}`, list })),
];

/** The items a series adds: with a list, as the chosen answer to "each row describes" keeps them. */
export const seriesItems = (node: SeriesNode): number => node.grain?.chosen.counts.items ?? node.items.chosen.counts.items;

/** The sets a choice of set makes; none when it is "no set". */
const setsOfChoice = (value: SetFrom | null, counts: SetCounts): number => (value === null ? 0 : counts.sets.length);

/** The series as the proposal holds it, by level id. */
const writtenIn = (structure: Structure, level: string): Series | undefined => seriesOfTree(structure).find((series) => series.id === level);

/** The sets a series written by hand names: one when it names its set; none counted when the set takes its names from the files. */
const writtenSets = (proposal: Proposal, node: OwnNode): number => {
  const set = writtenIn(proposal.structure, node.level)?.set ?? null;
  return set === null || 'each' in set ? 0 : 1;
};

/** The items a node of the tree adds. */
const itemsOf = (node: NodeEvidence): number => {
  switch (node.kind) {
    case 'set':
    case 'kept':
      return 0;
    case 'series':
      return seriesItems(node);
    case 'own':
      return node.items.items;
  }
};

const setsOf = (proposal: Proposal, node: NodeEvidence): number => {
  switch (node.kind) {
    case 'set':
      return node.counts.sets.length;
    case 'kept':
      return node.counts?.sets.length ?? 0;
    case 'series':
      return setsOfChoice(node.set.chosen.value, node.set.chosen.counts);
    case 'own':
      return writtenSets(proposal, node);
  }
};

export interface Totals {
  readonly items: number;
  readonly sets: number;
  readonly notImported: number;
  readonly changes: number;
}

export const totalsOf = (proposal: Proposal): Totals => ({
  items: proposal.nodes.reduce((sum, node) => sum + itemsOf(node), 0),
  sets: proposal.nodes.reduce((sum, node) => sum + setsOf(proposal, node), 0),
  notImported: proposal.outside.length,
  changes: proposal.edits.length,
});

/**
 * A column of a suggested series' first list, when that list's rows describe
 * files and a rule reads the column: where the plan puts it (`level`, read
 * from where its sources are) and what the proposer picks (`rollUp`).
 */
export interface PlacedColumn {
  readonly rollUp: ColumnRollUp;
  /** The list's id in the proposal's structure, which a click writes. */
  readonly sheet: string;
  readonly list: ListFile;
  readonly level: RollUp;
  /** Whether it may go on each file: no slot it feeds is an item's alone. */
  readonly fileAllowed: boolean;
}

/**
 * The file-row list of `node` as the tree shows it (the plan with the
 * proposal's structure) and its placed columns; `null` when its rows describe
 * items or it has no list.
 */
export const fileRowsOf = (plan: ImportPlan, proposal: Proposal, node: SeriesNode): { readonly sheet: SheetPlan; readonly placed: readonly PlacedColumn[] } | null => {
  const list = node.list.chosen.value;
  const sheet = list === null ? undefined : writtenIn(proposal.structure, node.level)?.lists.find((one) => one.source === list.source && one.file === list.file);
  if (list === null || sheet === undefined || sheet.rowKind !== 'media') {
    return null;
  }
  const shown = withStructure(plan, proposal.structure);
  const placed = node.columns.flatMap((rollUp): readonly PlacedColumn[] => {
    const level = levelOf(shown, sheet.id, rollUp.index);
    return level === 'out' ? [] : [{ rollUp, sheet: sheet.id, list, level, fileAllowed: fileLevelAllowed(shown, sheet.id, rollUp.index) }];
  });
  return { sheet, placed };
};

/** A column off the proposer's pick, and the folder whose list holds it. */
export interface ColumnChange {
  readonly node: SeriesNode;
  readonly column: PlacedColumn;
}

/** Every column the plan puts on the level the proposer did not pick. No edit records one: the plan's rules are the change. */
export const columnChangesOf = (plan: ImportPlan, proposal: Proposal): readonly ColumnChange[] =>
  seriesNodes(proposal).flatMap((node) =>
    (fileRowsOf(plan, proposal, node)?.placed ?? []).filter((column) => column.level !== column.rollUp.pick.chosen.value).map((column) => ({ node, column })),
  );

/** How a column's values vary inside the items of two files or more. */
export const variesWords = (varies: Varies, t: T, number: (n: number) => string): string => {
  if (varies.groups === 0) {
    return t('structure.rollUp.same');
  }
  return varies.disjoint === 0
    ? t('structure.rollUp.shares', { count: number(varies.groups), of: number(varies.of) })
    : t('structure.rollUp.apart', { count: number(varies.groups), of: number(varies.of), disjoint: number(varies.disjoint) });
};

export const rollUpWords = (level: RollUp, t: T): string => t(level === 'item' ? 'structure.rollUp.item' : 'structure.rollUp.file');

export const filesWords = (count: number, t: T, number: (n: number) => string): string =>
  count === 1 ? t('structure.rollUp.filesOne') : t('structure.rollUp.files', { count: number(count) });

/**
 * `suggested`: the plan still holds the archive as found (one series over
 * everything) and the proposal differs from it. `inUse`: the plan's structure
 * is the proposal's. `changed`: the person used a structure, and the files
 * now make another one.
 */
export type Status = 'suggested' | 'inUse' | 'changed';

export const statusOf = (plan: ImportPlan, proposal: Proposal): Status => {
  if (proposal.matchesPlan) {
    return 'inUse';
  }
  return isAsFound(plan) ? 'suggested' : 'changed';
};

export const itemsWords = (count: number, t: T, number: (n: number) => string): string =>
  count === 1 ? t('structure.itemsOne') : t('structure.items', { count: number(count) });

const KIND_WORDS: Readonly<Record<MediaKind, readonly [StructureMessageKey, StructureMessageKey]>> = {
  image: ['structure.kind.image', 'structure.kind.imageOne'],
  pdf: ['structure.kind.pdf', 'structure.kind.pdfOne'],
  audio: ['structure.kind.audio', 'structure.kind.audioOne'],
  video: ['structure.kind.video', 'structure.kind.videoOne'],
  document: ['structure.kind.document', 'structure.kind.documentOne'],
  other: ['structure.kind.other', 'structure.kind.otherOne'],
};

/** "13 films, 6 sounds": the files the install accepts, by kind, most first. */
export const kindsWords = (kinds: readonly (readonly [MediaKind, number])[], t: T, number: (n: number) => string): string =>
  kinds.map(([kind, count]) => (count === 1 ? t(KIND_WORDS[kind][1]) : t(KIND_WORDS[kind][0], { count: number(count) }))).join(', ');

/** A folder by its path; the whole source by `whole`. */
export const folderName = (folder: string, whole: string): string => (folder === '' ? whole : folder);

export const listName = (list: ListFile): string => list.file;

/**
 * The rows of a list that name a file. For the chosen list, the engine's own
 * count under the series' join: the rows less those that name no file. A
 * list whose rows are not read by file name has no such count, so it shows
 * the rows that name a file exactly.
 */
export const rowsNamingAFile = (candidate: ListCandidate, node: SeriesNode | null): number => {
  const grain = node?.grain?.chosen.counts;
  return grain === undefined ? (candidate.best?.paired ?? 0) : candidate.rows - grain.rowsNamingNoFile;
};

export const cutWords = (cut: ItemCut, t: T): string => {
  switch (cut.cut) {
    case 'whole':
      return t('structure.cut.whole');
    case 'places':
      return t('structure.cut.places', { places: String(cut.places) });
    case 'folder':
      return t('structure.cut.folder', { depth: String(cut.depth) });
    case 'rows':
      return t('structure.cut.rows', { list: listName(cut.list) });
  }
};

/** "43 with 1 file, 226 with 2 files": the buckets that hold an item. */
export const bucketsWords = (buckets: readonly BucketCount[], t: T, number: (n: number) => string): string =>
  buckets
    .filter((bucket) => bucket.count > 0)
    .map((bucket) => t('structure.bucket', { count: number(bucket.count), files: t(BUCKET_WORDS[bucket.files]) }))
    .join(', ');

/** Where a set takes its names: "folder name", "column Item Type", "part 5 of the name". A name place counts from 1. */
export const setFromWords = (from: SetFrom, t: T): string => {
  switch (from.from) {
    case 'folder':
      return t('structure.from.folder');
    case 'column':
      return t('structure.from.column', { column: from.column.header });
    case 'namePlace':
      return t('structure.from.place', { place: String(from.place + 1) });
    case 'folderLevel':
      return t('structure.from.level', { depth: String(from.depth) });
  }
};

/** A set choice by its name: the one set's title, or how many sets. */
export const setName = (counts: SetCounts, t: T, number: (n: number) => string): string => {
  const [only, ...more] = counts.sets;
  return only !== undefined && more.length === 0 ? only[0] : t('structure.set.several', { count: number(counts.sets.length) });
};

/** A choice of set shows this many of its sets, then how many more. */
export const SETS_SHOWN = 3;

/** "20170616 (11 items) · 20171013 (4 items)": the first sets with their items, then how many more. */
export const setsWords = (counts: SetCounts, t: T, number: (n: number) => string): string => {
  const { sets } = counts;
  const shown = sets.slice(0, SETS_SHOWN).map(([name, items]) => t('structure.split.set', { name, items: itemsWords(items, t, number) }));
  const more = sets.length > SETS_SHOWN ? [t('structure.split.more', { count: number(sets.length - SETS_SHOWN) })] : [];
  return [...shown, ...more].join(' · ');
};

/** A set level by its source: its title, the set in Omeka, or one set per value. */
export const keptSetName = (set: SetSource, t: T): string =>
  'title' in set ? set.title : 'id' in set ? t('structure.set.existing', { id: String(set.id) }) : t('structure.keptSet.each');

/** "folder name · 318 items", or "column Item Type · 690 of 690 rows · 313 items". */
export const setNote = (from: SetFrom, counts: SetCounts, t: T, number: (n: number) => string): string => {
  const items = counts.sets.reduce((sum, [, count]) => sum + count, 0);
  const rows = counts.rows === null ? [] : [t('structure.from.rows', { n: number(counts.rows.n), of: number(counts.rows.of) })];
  return [setFromWords(from, t), ...rows, itemsWords(items, t, number)].join(' · ');
};

export const grainWords = (kind: RecordKind, t: T): string =>
  ({ item: t('structure.grain.item'), media: t('structure.grain.media'), itemSet: t('structure.grain.itemSet') })[kind];

export const pairWords = (rule: PairRule, t: T): string => {
  switch (rule.rule) {
    case 'lastPlaces':
      return t('structure.pair.lastPlaces', { places: String(rule.places) });
    case 'folds':
      return t('structure.pair.folds');
    case 'column':
      return t('structure.pair.column', { column: rule.column.header });
  }
};

/** Every list file the proposal names: each series' choices and the lists not used. */
export const listsIn = (proposal: Proposal): { readonly used: number; readonly unused: number } => {
  const used = new Set(
    seriesNodes(proposal).flatMap((node) =>
      [node.list.chosen, node.list.rival, ...node.list.others].flatMap((pick) => (pick === null || pick.value === null ? [] : [`${pick.value.source}/${pick.value.file}`])),
    ),
  );
  return { used: used.size, unused: proposal.unattached.length };
};

/** A list the person uploaded that the suggestion keeps where it is: the rows it pairs here, or that it does not read now. */
export const keptWords = (kept: KeptList, t: T, number: (n: number) => string): string =>
  kept.rows === null
    ? t('structure.kept.unread', { file: kept.file.file })
    : t('structure.kept.rows', { file: kept.file.file, n: number(kept.rows.n), of: number(kept.rows.of) });
