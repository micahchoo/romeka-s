/**
 * The preview (`POST /api/romeka/imports/{id}/preview`): the sections the
 * admin can ask for and the parse of each. The shapes are
 * `crates/omeka-api/src/import/preview.rs` and its submodules; what the
 * request asks of them is `import-queries.ts`, and the sheet and tag views
 * are `import-sheet-views.ts`.
 *
 * An answer holds `sources`, `pattern` and exactly the sections the request
 * named. `parsePreview(include)` refuses an answer without one of them and
 * an answer with one that was not asked for. `hasSections` narrows a parsed
 * preview to the sections that were asked, with no cast.
 */
import {
  asCount,
  asFlag,
  list,
  member,
  nullable,
  oneOf,
  strict,
  wordOr,
} from './import-read';
import { parseAnomaly, parseEntryView, parseFlag, parseNewSet, parseOrphan, parsePatternState, parseRecordView, parseSubjectOf, asRecordId } from './import-model';
import type { Anomaly, EntryView, Flag, NewSet, Orphan, PatternState, RecordId, RecordView, Subject } from './import-model';
import { ACTIONS, FLAG_KINDS, FLAG_POLICIES, RECORD_KINDS } from './import-plan';
import { parseSheetView, parseTagView } from './import-sheet-views';
import type { SheetView, TagView } from './import-sheet-views';
import {
  asSlotName,
  asTerm,
  parseColumnRef,
  parseEdit,
  parseFileRef,
  parseFolderScope,
  parseListFile,
  parsePairRule,
  parseRowRef,
  parseSetFrom,
  parseStructure,
} from './import-plan-read';
import type {
  Action,
  ColumnRef,
  Edit,
  FileRef,
  FlagKind,
  FlagPolicy,
  FolderScope,
  Join,
  LevelId,
  ListFile,
  PairRule,
  RecordKind,
  RowRef,
  SetFrom,
  SlotName,
  Structure,
  Term,
} from './import-plan';
import { ParseError, asRecord, asString } from './parse';
import type { Parser, Path } from './types';
import type { StarterId } from './starters';
import { STARTER_IDS } from './starters';

export interface Paged<T> {
  readonly total: number;
  readonly page: number;
  readonly perPage: number;
  readonly items: readonly T[];
}

const parsePaged =
  <T>(parse: (value: unknown, path: Path) => T) =>
  (raw: unknown, path: Path): Paged<T> => {
    const object = strict(raw, path, ['total', 'page', 'perPage', 'items']);
    return {
      total: member(object, 'total', path, asCount),
      page: member(object, 'page', path, asCount),
      perPage: member(object, 'perPage', path, asCount),
      items: member(object, 'items', path, list(parse)),
    };
  };

const text = (object: Record<string, unknown>, key: string, path: Path): string => member(object, key, path, asString);
const count = (object: Record<string, unknown>, key: string, path: Path): number => member(object, key, path, asCount);
const maybeText = nullable(asString);

/** Files per item, in the buckets the admin shows. */
export type Bucket = '0' | '1' | '2' | '3' | '4-6' | '7+';
export const BUCKETS: readonly Bucket[] = ['0', '1', '2', '3', '4-6', '7+'];
export type MediaKind = 'image' | 'pdf' | 'audio' | 'video' | 'document' | 'other';
export const MEDIA_KINDS: readonly MediaKind[] = ['image', 'pdf', 'audio', 'video', 'document', 'other'];

export type ItemOrigin = 'pattern' | 'row' | 'own';
export const ITEM_ORIGINS: readonly ItemOrigin[] = ['pattern', 'row', 'own'];

/** The facts one item has; a `shapes` group's facts are sent back as the `records` filter `shape`. */
export interface ShapeFacts {
  readonly files: Bucket;
  readonly folders: readonly string[];
  readonly media: readonly MediaKind[];
  /** Why the item exists: the pattern grouped its files, a list row made it, or a file was made an item of its own. */
  readonly origin: ItemOrigin;
  readonly row: boolean;
  /** `null` when the pattern has no `part` group. */
  readonly parts: readonly string[] | null;
  readonly pages: readonly string[] | null;
}

export interface SourceState {
  readonly source: string;
  readonly scanned: string | null;
  readonly files: number;
  readonly sheetFiles: readonly string[];
  readonly error: string | null;
}

export const parseSourceState: Parser<SourceState> = (raw, path = []) => {
  const object = strict(raw, path, ['source', 'scanned', 'files', 'sheetFiles', 'error']);
  return {
    source: text(object, 'source', path),
    scanned: member(object, 'scanned', path, maybeText),
    files: count(object, 'files', path),
    sheetFiles: member(object, 'sheetFiles', path, list(asString)),
    error: member(object, 'error', path, maybeText),
  };
};

export interface FlagSample {
  readonly subject: Subject;
  /** The record the subject is, or the record its entry lands in. */
  readonly record: RecordId | null;
  readonly detail: string | null;
}

/** One kind, slot and policy: how many flags, and at most three drawn by the request's seed. */
export interface FlagCount {
  readonly kind: FlagKind;
  readonly slot: SlotName | null;
  readonly policy: FlagPolicy | null;
  readonly count: number;
  readonly samples: readonly FlagSample[];
}

const parseFlagCount: Parser<FlagCount> = (raw, path = []) => {
  const object = strict(raw, path, ['kind', 'slot', 'policy', 'count', 'samples']);
  return {
    kind: member(object, 'kind', path, oneOf(FLAG_KINDS)),
    slot: member(object, 'slot', path, nullable(asSlotName)),
    policy: member(object, 'policy', path, nullable(oneOf(FLAG_POLICIES))),
    count: count(object, 'count', path),
    samples: member(
      object,
      'samples',
      path,
      list((value, at) => {
        const sample = strict(value, at, ['subject', 'record', 'detail']);
        return {
          subject: member(sample, 'subject', at, parseSubjectOf),
          record: member(sample, 'record', at, nullable(asRecordId)),
          detail: member(sample, 'detail', at, maybeText),
        };
      }),
    ),
  };
};

export type Does = 'create' | 'leave' | { readonly act: Action };

export interface CheckCount {
  readonly kind: RecordKind;
  readonly does: Does;
  readonly count: number;
}

export interface NewTypeCheck {
  readonly label: string;
  readonly from: 'mapping' | { readonly starter: StarterId };
  /** The fields from the mapping; empty for a starter, whose fields the starter table lists. */
  readonly fields: readonly Term[];
}

export interface NewFieldCheck {
  readonly label: string;
  readonly term: Term | null;
}

/** What a run would do, and whether it may run. */
export interface Check {
  readonly action: Action;
  readonly counts: readonly CheckCount[];
  readonly newSets: readonly NewSet[];
  readonly newTypes: readonly NewTypeCheck[];
  readonly newFields: readonly NewFieldCheck[];
  /** How many flags each policy applies; a policy no flag has is absent. */
  readonly flags: { readonly [policy in FlagPolicy]?: number };
  /** The flags that stop the run. */
  readonly blocking: readonly Flag[];
  readonly withoutTemplate: number;
  readonly defaultOwner: number;
  /** Items the run creates with no title, whatever their item type requires. */
  readonly itemsWithoutTitle: number;
  /** The run needs this as `confirm`. */
  readonly deletes: number;
  readonly canRun: boolean;
  /** The run sends it back; a change since answers 409 `changed`. */
  readonly digest: string;
}

const parseCheckCount: Parser<CheckCount> = (raw, path = []) => {
  const object = strict(raw, path, ['kind', 'does', 'count']);
  const does = wordOr<'create' | 'leave', { readonly act: Action }>(['create', 'leave'], (value, at) => ({
    act: member(strict(value, at, ['act']), 'act', at, oneOf(ACTIONS)),
  }));
  return {
    kind: member(object, 'kind', path, oneOf(RECORD_KINDS)),
    does: member(object, 'does', path, does),
    count: count(object, 'count', path),
  };
};

const parseNewTypeCheck: Parser<NewTypeCheck> = (raw, path = []) => {
  const object = strict(raw, path, ['label', 'from', 'fields']);
  const from = wordOr<'mapping', { readonly starter: StarterId }>(['mapping'], (value, at) => ({
    starter: member(strict(value, at, ['starter']), 'starter', at, oneOf(STARTER_IDS)),
  }));
  return {
    label: text(object, 'label', path),
    from: member(object, 'from', path, from),
    fields: member(object, 'fields', path, list(asTerm)),
  };
};

const parsePolicyCounts = (raw: unknown, path: Path): Check['flags'] => {
  const object = strict(raw, path, [], FLAG_POLICIES);
  const counts: { [policy in FlagPolicy]?: number } = {};
  for (const policy of FLAG_POLICIES) {
    if (object[policy] !== undefined) {
      counts[policy] = member(object, policy, path, asCount);
    }
  }
  return counts;
};

const parseCheck: Parser<Check> = (raw, path = []) => {
  const object = strict(raw, path, [
    'action',
    'counts',
    'newSets',
    'newTypes',
    'newFields',
    'flags',
    'blocking',
    'withoutTemplate',
    'defaultOwner',
    'itemsWithoutTitle',
    'deletes',
    'canRun',
    'digest',
  ]);
  return {
    action: member(object, 'action', path, oneOf(ACTIONS)),
    counts: member(object, 'counts', path, list(parseCheckCount)),
    newSets: member(object, 'newSets', path, list(parseNewSet)),
    newTypes: member(object, 'newTypes', path, list(parseNewTypeCheck)),
    newFields: member(
      object,
      'newFields',
      path,
      list((value, at) => {
        const field = strict(value, at, ['label', 'term']);
        return { label: text(field, 'label', at), term: member(field, 'term', at, nullable(asTerm)) };
      }),
    ),
    flags: member(object, 'flags', path, parsePolicyCounts),
    blocking: member(object, 'blocking', path, list(parseFlag)),
    withoutTemplate: count(object, 'withoutTemplate', path),
    defaultOwner: count(object, 'defaultOwner', path),
    itemsWithoutTitle: count(object, 'itemsWithoutTitle', path),
    deletes: count(object, 'deletes', path),
    canRun: member(object, 'canRun', path, asFlag),
    digest: text(object, 'digest', path),
  };
};

export interface ShapeGroup {
  readonly facts: ShapeFacts;
  readonly count: number;
  /** Under 2% of the records, or 7 or more files. */
  readonly odd: boolean;
  /** Three records, drawn by the request's seed. */
  readonly samples: readonly RecordView[];
}

export interface Shapes {
  readonly records: number;
  readonly groups: Paged<ShapeGroup>;
}

export interface BucketCount {
  readonly files: Bucket;
  readonly count: number;
}

const parseShapeFacts: Parser<ShapeFacts> = (raw, path = []) => {
  const object = strict(raw, path, ['files', 'folders', 'media', 'origin', 'row', 'parts', 'pages']);
  return {
    files: member(object, 'files', path, oneOf(BUCKETS)),
    folders: member(object, 'folders', path, list(asString)),
    media: member(object, 'media', path, list(oneOf(MEDIA_KINDS))),
    origin: member(object, 'origin', path, oneOf(ITEM_ORIGINS)),
    row: member(object, 'row', path, asFlag),
    parts: member(object, 'parts', path, nullable(list(asString))),
    pages: member(object, 'pages', path, nullable(list(asString))),
  };
};

const parseShapes: Parser<Shapes> = (raw, path = []) => {
  const object = strict(raw, path, ['records', 'groups']);
  const group = (value: unknown, at: Path): ShapeGroup => {
    const each = strict(value, at, ['facts', 'count', 'odd', 'samples']);
    return {
      facts: member(each, 'facts', at, parseShapeFacts),
      count: count(each, 'count', at),
      odd: member(each, 'odd', at, asFlag),
      samples: member(each, 'samples', at, list(parseRecordView)),
    };
  };
  return { records: count(object, 'records', path), groups: member(object, 'groups', path, parsePaged(group)) };
};

export interface IdenticalSet {
  readonly source: string;
  readonly size: number;
  readonly sha256: string;
  readonly paths: readonly string[];
}

export interface IdenticalFiles {
  /** Files that share their bytes with another file, over every set. */
  readonly files: number;
  readonly sets: Paged<IdenticalSet>;
}

const parseIdentical: Parser<IdenticalFiles> = (raw, path = []) => {
  const object = strict(raw, path, ['files', 'sets']);
  const set = (value: unknown, at: Path): IdenticalSet => {
    const each = strict(value, at, ['source', 'size', 'sha256', 'paths']);
    return {
      source: text(each, 'source', at),
      size: count(each, 'size', at),
      sha256: text(each, 'sha256', at),
      paths: member(each, 'paths', at, list(asString)),
    };
  };
  return { files: count(object, 'files', path), sets: member(object, 'sets', path, parsePaged(set)) };
};

export interface UnpairedRow {
  readonly row: RowRef;
  /** The item the row made; none for a row that describes a file. */
  readonly record: RecordId | null;
  readonly title: string | null;
}

export interface UnpairedFile {
  readonly file: FileRef;
  readonly record: RecordId;
}

/**
 * List rows that name no file of the import, and files whose item has no
 * list row, each sorted by name; a row naming a file the install refuses is
 * the `listed-not-media` flag's, not one of these.
 */
export interface Unpaired {
  readonly rows: Paged<UnpairedRow>;
  readonly files: Paged<UnpairedFile>;
  /** How many items the unpaired files are in. */
  readonly items: number;
}

const parseUnpaired: Parser<Unpaired> = (raw, path = []) => {
  const object = strict(raw, path, ['rows', 'files', 'items']);
  const row = (value: unknown, at: Path): UnpairedRow => {
    const each = strict(value, at, ['row', 'record', 'title']);
    return {
      row: member(each, 'row', at, parseRowRef),
      record: member(each, 'record', at, nullable(asRecordId)),
      title: member(each, 'title', at, maybeText),
    };
  };
  const file = (value: unknown, at: Path): UnpairedFile => {
    const each = strict(value, at, ['file', 'record']);
    return { file: member(each, 'file', at, parseFileRef), record: member(each, 'record', at, asRecordId) };
  };
  return { rows: member(object, 'rows', path, parsePaged(row)), files: member(object, 'files', path, parsePaged(file)), items: count(object, 'items', path) };
};

export interface ValueCount {
  /** The value as the first record holding it writes it. */
  readonly text: string;
  /** Records the run writes that hold the value. */
  readonly count: number;
  /** Every spelling that folds to the value, sorted. */
  readonly spellings: readonly string[];
}

export interface SlotValues {
  readonly slot: SlotName;
  readonly values: Paged<ValueCount>;
}

const parseSlotValues: Parser<SlotValues> = (raw, path = []) => {
  const object = strict(raw, path, ['slot', 'values']);
  const value = (each: unknown, at: Path): ValueCount => {
    const one = strict(each, at, ['text', 'count', 'spellings']);
    return { text: text(one, 'text', at), count: count(one, 'count', at), spellings: member(one, 'spellings', at, list(asString)) };
  };
  return { slot: member(object, 'slot', path, asSlotName), values: member(object, 'values', path, parsePaged(value)) };
};

export interface NameWord {
  readonly text: string;
  readonly count: number;
}

/**
 * The words at each place of the name of the request's `path`: its `parts`,
 * how many file `names` have as many parts, and for each part a page of the
 * words found there. Empty when the request names no path.
 */
export interface NameWords {
  readonly parts: readonly string[];
  readonly names: number;
  readonly places: readonly Paged<NameWord>[];
}

const parseNameWords: Parser<NameWords> = (raw, path = []) => {
  const object = strict(raw, path, ['parts', 'names', 'places']);
  const word = (value: unknown, at: Path): NameWord => {
    const each = strict(value, at, ['text', 'count']);
    return { text: text(each, 'text', at), count: count(each, 'count', at) };
  };
  return {
    parts: member(object, 'parts', path, list(asString)),
    names: member(object, 'names', path, asCount),
    places: member(object, 'places', path, list(parsePaged(word))),
  };
};

export interface ColumnMatch {
  readonly index: number;
  readonly header: string;
  /** Cells that exactly equal a group key. */
  readonly groupKeys: number;
  /** Cells that exactly equal a file name. */
  readonly fileNames: number;
  readonly empty: number;
}

export interface SheetColumnMatches {
  readonly sheet: string;
  /** Rows of the sheet: the whole of each column's counts. */
  readonly rows: number;
  readonly columns: readonly ColumnMatch[];
}

const parseColumnMatches: Parser<readonly SheetColumnMatches[]> = (raw, path = []) => {
  const column = (value: unknown, at: Path): ColumnMatch => {
    const each = strict(value, at, ['index', 'header', 'groupKeys', 'fileNames', 'empty']);
    return {
      index: count(each, 'index', at),
      header: text(each, 'header', at),
      groupKeys: count(each, 'groupKeys', at),
      fileNames: count(each, 'fileNames', at),
      empty: count(each, 'empty', at),
    };
  };
  const sheet = (value: unknown, at: Path): SheetColumnMatches => {
    const each = strict(value, at, ['sheet', 'rows', 'columns']);
    return {
      sheet: text(each, 'sheet', at),
      rows: count(each, 'rows', at),
      columns: member(each, 'columns', at, list(column)),
    };
  };
  return list(sheet)(raw, path);
};

/** The column and join that pair the most rows of a list exactly. */
export interface BestColumn {
  readonly index: number;
  readonly header: string;
  readonly on: Join['on'];
  /** Cells that exactly equal a group key (`identifier`) or a file's name or path (`fileName`). */
  readonly paired: number;
  /** The same, with every name fold on. */
  readonly pairedNormalized: number;
  readonly empty: number;
}

/** Of the `of` items that more than one row names, the `groups` whose rows hold different values in a column. */
export interface Varies {
  readonly groups: number;
  readonly of: number;
}

/** The evidence that a list's rows describe files: several rows meet one item. */
export interface Grain {
  readonly groupsWithManyRows: number;
  readonly rowsNamingOtherFilesOfAGroup: number;
  /** Every column of the list, with how often its values differ inside one item. */
  readonly columns: readonly { readonly index: number; readonly header: string; readonly varies: Varies }[];
}

/** Which separator one column's cells hold, read as the plan sheet reads the list. */
export interface ColumnCells {
  readonly index: number;
  readonly header: string;
  /** Of `,`, `;` and `|`, the one the most cells hold, and in how many. */
  readonly separator: { readonly text: string; readonly cells: number } | null;
}

/** Another list that pairs some of the same items. */
export interface ListOverlap {
  readonly source: string;
  readonly file: string;
  readonly sharedTargets: number;
}

/** A list file the scan found, and how well it fits the files. */
export interface ListCandidate {
  readonly source: string;
  readonly file: string;
  /** The plan sheet that reads this file. */
  readonly inPlan: string | null;
  readonly read: 'ok' | { readonly error: string };
  readonly rows: number;
  readonly columns: number;
  readonly best: BestColumn | null;
  /** Set when `best` pairs by file name. */
  readonly grain: Grain | null;
  /** Every column, in sheet order; none when the list did not read. */
  readonly cells: readonly ColumnCells[];
  readonly overlaps: readonly ListOverlap[];
}

const parseGrain = (value: unknown, at: Path): Grain => {
  const each = strict(value, at, ['groupsWithManyRows', 'rowsNamingOtherFilesOfAGroup', 'columns']);
  const column = (raw: unknown, place: Path): Grain['columns'][number] => {
    const one = strict(raw, place, ['index', 'header', 'varies']);
    const varies = (inner: unknown, where: Path): Varies => {
      const both = strict(inner, where, ['groups', 'of']);
      return { groups: count(both, 'groups', where), of: count(both, 'of', where) };
    };
    return { index: count(one, 'index', place), header: text(one, 'header', place), varies: member(one, 'varies', place, varies) };
  };
  return {
    groupsWithManyRows: count(each, 'groupsWithManyRows', at),
    rowsNamingOtherFilesOfAGroup: count(each, 'rowsNamingOtherFilesOfAGroup', at),
    columns: member(each, 'columns', at, list(column)),
  };
};

const parseListCandidate = (raw: unknown, path: Path): ListCandidate => {
  const object = strict(raw, path, ['source', 'file', 'inPlan', 'read', 'rows', 'columns', 'best', 'grain', 'cells', 'overlaps']);
  const best = (value: unknown, at: Path): BestColumn => {
    const each = strict(value, at, ['index', 'header', 'on', 'paired', 'pairedNormalized', 'empty']);
    return {
      index: count(each, 'index', at),
      header: text(each, 'header', at),
      on: member(each, 'on', at, oneOf<Join['on']>(['identifier', 'fileName'])),
      paired: count(each, 'paired', at),
      pairedNormalized: count(each, 'pairedNormalized', at),
      empty: count(each, 'empty', at),
    };
  };
  const cells = (value: unknown, at: Path): ColumnCells => {
    const each = strict(value, at, ['index', 'header', 'separator']);
    const separator = (inner: unknown, where: Path): NonNullable<ColumnCells['separator']> => {
      const both = strict(inner, where, ['text', 'cells']);
      return { text: text(both, 'text', where), cells: count(both, 'cells', where) };
    };
    return {
      index: count(each, 'index', at),
      header: text(each, 'header', at),
      separator: member(each, 'separator', at, nullable(separator)),
    };
  };
  const overlap = (value: unknown, at: Path): ListOverlap => {
    const each = strict(value, at, ['source', 'file', 'sharedTargets']);
    return { source: text(each, 'source', at), file: text(each, 'file', at), sharedTargets: count(each, 'sharedTargets', at) };
  };
  const read = wordOr(['ok'] as const, (value, at): { readonly error: string } => ({
    error: text(strict(value, at, ['error']), 'error', at),
  }));
  return {
    source: text(object, 'source', path),
    file: text(object, 'file', path),
    inPlan: member(object, 'inPlan', path, maybeText),
    read: member(object, 'read', path, read),
    rows: count(object, 'rows', path),
    columns: count(object, 'columns', path),
    best: member(object, 'best', path, nullable(best)),
    grain: member(object, 'grain', path, nullable(parseGrain)),
    cells: member(object, 'cells', path, list(cells)),
    overlaps: member(object, 'overlaps', path, list(overlap)),
  };
};

/** `n` of `of`: with plain counts, the only number shape in a proposal's evidence. */
export interface Share {
  readonly n: number;
  readonly of: number;
}

/** A choice of the proposer, or an offer, with the counts that show it. */
export interface Pick<T, C> {
  readonly value: T;
  readonly counts: C;
}

/** A choice the proposer made, and the next best choice. */
export interface Choice<T, C> {
  readonly chosen: Pick<T, C>;
  readonly rival: Pick<T, C> | null;
}

/** The files a series or an outside folder takes. */
export interface ScopeCounts {
  readonly scope: FolderScope;
  readonly files: number;
  /** The files the install's media table accepts. */
  readonly media: number;
  /** The accepted files by kind, most first. */
  readonly kinds: readonly (readonly [MediaKind, number])[];
}

/** Where a series' item key ends. */
export type ItemCut =
  | { readonly cut: 'whole' }
  | { readonly cut: 'places'; readonly places: number; readonly rule: 'recurringParts' | 'namesEndHere' | 'sameAsSibling' }
  | { readonly cut: 'folder'; readonly depth: number }
  | { readonly cut: 'rows'; readonly list: ListFile; readonly key: ColumnRef };

export interface CutCounts {
  readonly items: number;
  /** Files in items, of the media in the scope. */
  readonly files: Share;
  readonly perItem: readonly BucketCount[];
  /** Each part value with the items that hold it, most first. */
  readonly parts: readonly (readonly [string, number])[];
  readonly namesEndingHere: number;
  /** Media whose shape the key does not fit: they make no item. */
  readonly otherShapes: number;
}

export interface SetCounts {
  /** Each set with the items it would hold, most first. */
  readonly sets: readonly (readonly [string, number])[];
  /** For a column: the rows that hold a value, of the rows. */
  readonly rows: Share | null;
}

export interface PairCounts {
  readonly rows: Share;
  /** Rows the rule pairs that the stated join does not. */
  readonly more: number;
}

export interface SeriesNode {
  readonly level: LevelId;
  readonly scope: ScopeCounts;
  readonly items: Choice<ItemCut, CutCounts>;
  /** A choice of no list has no list counts. */
  readonly list: Choice<ListFile | null, ListCandidate | null>;
  readonly grain: Choice<RecordKind, Grain> | null;
  readonly set: Choice<SetFrom | null, SetCounts>;
  readonly splits: readonly Pick<SetFrom, SetCounts>[];
  readonly pairing: readonly Pick<PairRule, PairCounts>[];
}

export type NodeEvidence =
  | { readonly kind: 'set'; readonly level: LevelId; readonly from: SetFrom; readonly counts: SetCounts }
  | ({ readonly kind: 'series' } & SeriesNode);

export interface OutsideFolder extends ScopeCounts {
  readonly reason: 'noMedia' | 'edit';
}

/** The structure the server proposes from the scan, after the plan's edits; never stored by the server. */
export interface Proposal {
  readonly structure: Structure;
  readonly nodes: readonly NodeEvidence[];
  /** Sets that would hold every item: offered, never chosen. */
  readonly topSets: readonly Pick<SetFrom, SetCounts>[];
  readonly outside: readonly OutsideFolder[];
  readonly unattached: readonly ListCandidate[];
  /** Each edit of the plan, and whether it still found what it names. */
  readonly edits: readonly { readonly edit: Edit; readonly applied: boolean }[];
  /** Whether `structure` is the plan's own. */
  readonly matchesPlan: boolean;
}

const parseBucketCount = (value: unknown, at: Path): BucketCount => {
  const each = strict(value, at, ['files', 'count']);
  return { files: member(each, 'files', at, oneOf(BUCKETS)), count: count(each, 'count', at) };
};

const parseShare = (raw: unknown, path: Path): Share => {
  const object = strict(raw, path, ['n', 'of']);
  return { n: count(object, 'n', path), of: count(object, 'of', path) };
};

const parsePair =
  <A>(first: (value: unknown, path: Path) => A) =>
  (raw: unknown, path: Path): readonly [A, number] => {
    if (!Array.isArray(raw) || raw.length !== 2) {
      throw new ParseError(path, 'a pair');
    }
    return [first(raw[0], [...path, 0]), asCount(raw[1], [...path, 1])];
  };

const parsePick =
  <T, C>(value: (raw: unknown, path: Path) => T, counts: (raw: unknown, path: Path) => C) =>
  (raw: unknown, path: Path): Pick<T, C> => {
    const object = strict(raw, path, ['value', 'counts']);
    return { value: member(object, 'value', path, value), counts: member(object, 'counts', path, counts) };
  };

const parseChoice = <T, C>(value: (raw: unknown, path: Path) => T, counts: (raw: unknown, path: Path) => C) => {
  const pick = parsePick(value, counts);
  return (raw: unknown, path: Path): Choice<T, C> => {
    const object = strict(raw, path, ['chosen', 'rival']);
    return { chosen: member(object, 'chosen', path, pick), rival: member(object, 'rival', path, nullable(pick)) };
  };
};

const SCOPE_COUNTS = ['scope', 'files', 'media', 'kinds'] as const;

const scopeCounts = (object: Record<string, unknown>, path: Path): ScopeCounts => ({
  scope: member(object, 'scope', path, parseFolderScope),
  files: count(object, 'files', path),
  media: count(object, 'media', path),
  kinds: member(object, 'kinds', path, list(parsePair(oneOf(MEDIA_KINDS)))),
});

const parseItemCut = (raw: unknown, path: Path): ItemCut => {
  const cut = member(asRecord(raw, path), 'cut', path, oneOf(['whole', 'places', 'folder', 'rows'] as const));
  switch (cut) {
    case 'whole':
      strict(raw, path, ['cut']);
      return { cut };
    case 'places': {
      const only = strict(raw, path, ['cut', 'places', 'rule']);
      return {
        cut,
        places: count(only, 'places', path),
        rule: member(only, 'rule', path, oneOf(['recurringParts', 'namesEndHere', 'sameAsSibling'] as const)),
      };
    }
    case 'folder':
      return { cut, depth: count(strict(raw, path, ['cut', 'depth']), 'depth', path) };
    case 'rows': {
      const only = strict(raw, path, ['cut', 'list', 'key']);
      return { cut, list: member(only, 'list', path, parseListFile), key: member(only, 'key', path, parseColumnRef) };
    }
  }
};

const parseCutCounts = (raw: unknown, path: Path): CutCounts => {
  const object = strict(raw, path, ['items', 'files', 'perItem', 'parts', 'namesEndingHere', 'otherShapes']);
  return {
    items: count(object, 'items', path),
    files: member(object, 'files', path, parseShare),
    perItem: member(object, 'perItem', path, list(parseBucketCount)),
    parts: member(object, 'parts', path, list(parsePair(asString))),
    namesEndingHere: count(object, 'namesEndingHere', path),
    otherShapes: count(object, 'otherShapes', path),
  };
};

const parseSetCounts = (raw: unknown, path: Path): SetCounts => {
  const object = strict(raw, path, ['sets', 'rows']);
  return { sets: member(object, 'sets', path, list(parsePair(asString))), rows: member(object, 'rows', path, nullable(parseShare)) };
};

const parsePairCounts = (raw: unknown, path: Path): PairCounts => {
  const object = strict(raw, path, ['rows', 'more']);
  return { rows: member(object, 'rows', path, parseShare), more: count(object, 'more', path) };
};

const parseNodeEvidence = (raw: unknown, path: Path): NodeEvidence => {
  const kind = member(asRecord(raw, path), 'kind', path, oneOf(['set', 'series'] as const));
  if (kind === 'set') {
    const only = strict(raw, path, ['kind', 'level', 'from', 'counts']);
    return {
      kind,
      level: text(only, 'level', path),
      from: member(only, 'from', path, parseSetFrom),
      counts: member(only, 'counts', path, parseSetCounts),
    };
  }
  const only = strict(raw, path, ['kind', 'level', 'scope', 'items', 'list', 'grain', 'set', 'splits', 'pairing']);
  return {
    kind,
    level: text(only, 'level', path),
    scope: member(only, 'scope', path, (value, at) => scopeCounts(strict(value, at, [...SCOPE_COUNTS]), at)),
    items: member(only, 'items', path, parseChoice(parseItemCut, parseCutCounts)),
    list: member(only, 'list', path, parseChoice(nullable(parseListFile), nullable(parseListCandidate))),
    grain: member(only, 'grain', path, nullable(parseChoice(oneOf(RECORD_KINDS), parseGrain))),
    set: member(only, 'set', path, parseChoice(nullable(parseSetFrom), parseSetCounts)),
    splits: member(only, 'splits', path, list(parsePick(parseSetFrom, parseSetCounts))),
    pairing: member(only, 'pairing', path, list(parsePick(parsePairRule, parsePairCounts))),
  };
};

const parseProposal = (raw: unknown, path: Path): Proposal => {
  const object = strict(raw, path, ['structure', 'nodes', 'topSets', 'outside', 'unattached', 'edits', 'matchesPlan']);
  const outside = (value: unknown, at: Path): OutsideFolder => {
    const each = strict(value, at, [...SCOPE_COUNTS, 'reason']);
    return { ...scopeCounts(each, at), reason: member(each, 'reason', at, oneOf(['noMedia', 'edit'] as const)) };
  };
  const edit = (value: unknown, at: Path): Proposal['edits'][number] => {
    const each = strict(value, at, ['edit', 'applied']);
    return { edit: member(each, 'edit', at, parseEdit), applied: member(each, 'applied', at, asFlag) };
  };
  return {
    structure: member(object, 'structure', path, parseStructure),
    nodes: member(object, 'nodes', path, list(parseNodeEvidence)),
    topSets: member(object, 'topSets', path, list(parsePick(parseSetFrom, parseSetCounts))),
    outside: member(object, 'outside', path, list(outside)),
    unattached: member(object, 'unattached', path, list(parseListCandidate)),
    edits: member(object, 'edits', path, list(edit)),
    matchesPlan: member(object, 'matchesPlan', path, asFlag),
  };
};

export interface SectionShapes {
  readonly entries: Paged<EntryView>;
  readonly sheets: readonly SheetView[];
  readonly tags: readonly TagView[];
  readonly records: Paged<RecordView>;
  readonly flags: readonly FlagCount[];
  readonly anomalies: readonly Anomaly[];
  readonly orphans: readonly Orphan[];
  readonly check: Check;
  readonly shapes: Shapes;
  readonly histogram: readonly BucketCount[];
  readonly identical: IdenticalFiles;
  readonly unpaired: Unpaired;
  readonly values: readonly SlotValues[];
  readonly nameWords: NameWords;
  readonly columnMatches: readonly SheetColumnMatches[];
  readonly listCandidates: readonly ListCandidate[];
  readonly proposal: Proposal;
}

export type Section = keyof SectionShapes;

const PARSERS: { readonly [S in Section]: (value: unknown, path: Path) => SectionShapes[S] } = {
  entries: parsePaged(parseEntryView),
  sheets: list(parseSheetView),
  tags: list(parseTagView),
  records: parsePaged(parseRecordView),
  flags: list(parseFlagCount),
  anomalies: list(parseAnomaly),
  orphans: list(parseOrphan),
  check: parseCheck,
  shapes: parseShapes,
  histogram: list(parseBucketCount),
  identical: parseIdentical,
  unpaired: parseUnpaired,
  values: list(parseSlotValues),
  nameWords: parseNameWords,
  columnMatches: parseColumnMatches,
  listCandidates: list(parseListCandidate),
  proposal: parseProposal,
};

export const SECTIONS: readonly Section[] = Object.keys(PARSERS).filter(
  (name): name is Section => name in PARSERS,
);

/** Every source's scan state, the pattern's state, and the sections the request named. */
export type Preview = { readonly sources: readonly SourceState[]; readonly pattern: PatternState } & {
  readonly [S in Section]?: SectionShapes[S];
};

export type PreviewWith<S extends Section> = Preview & { readonly [K in S]-?: SectionShapes[K] };

/** Whether `preview` holds every section in `include`; `parsePreview` already made sure it does. */
export const hasSections = <S extends Section>(
  preview: Preview,
  include: readonly S[],
): preview is PreviewWith<S> => include.every((section) => preview[section] !== undefined);

export const parsePreview =
  <S extends Section>(include: readonly S[]): Parser<PreviewWith<S>> =>
  (raw, path = []) => {
    const object = strict(raw, path, ['sources', 'pattern', ...include]);
    const preview: { -readonly [K in keyof Preview]: Preview[K] } = {
      sources: member(object, 'sources', path, list(parseSourceState)),
      pattern: member(object, 'pattern', path, parsePatternState),
    };
    for (const section of include) {
      Object.assign(preview, { [section]: member(object, section, path, PARSERS[section]) });
    }
    if (!hasSections(preview, include)) {
      throw new ParseError(path, 'every section the request named');
    }
    return preview;
  };
