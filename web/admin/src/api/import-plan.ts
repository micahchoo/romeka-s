/**
 * The import plan types: the rule and the decisions the admin edits and the server
 * stores (`docs/import-v2.md`, "The plan"), and the v3 plan
 * (`plan/v3.rs`). Every shape here is the serde
 * shape of `crates/omeka-api/src/import/plan` and `decisions.rs`, which
 * refuses members it does not know, so the parsers here refuse them too and
 * a parsed plan serialises back to the JSON it came from. The parsers are
 * in `import-plan-read.ts`.
 *
 * Two members differ from the contract's TypeScript sketch, because the
 * server's JSON does: a `move` decision names its file as an entry
 * (`{ file: { source, path } }`), and a value map the plan leaves empty is
 * absent, not `{}`.
 */
import type { StarterId } from './starters';
import { brandId } from './parse';
import type { Id, JsonValue } from './types';

export type ImportId = Id<'ImportId'>;

export type RecordKind = 'item' | 'media' | 'itemSet';
export const RECORD_KINDS: readonly RecordKind[] = ['item', 'media', 'itemSet'];

/** A property or class name, `prefix:local`. */
export type Term = `${string}:${string}`;

export const isTerm = (text: string): text is Term => {
  const colon = text.indexOf(':');
  return (
    colon > 0 &&
    colon < text.length - 1 &&
    !text.includes(':', colon + 1) &&
    !/[\s\p{Cc}]/u.test(text)
  );
};

export const NAMED_SLOTS = [
  'sets',
  'template',
  'class',
  'owner',
  'visibility',
  'sites',
  'mediaOrder',
  'ingester',
  'mediaSource',
  'itemKey',
] as const;
export type NamedSlot = (typeof NAMED_SLOTS)[number];
export type SlotName = NamedSlot | Term;

export const isMultiValued = (slot: SlotName): boolean => slot === 'sets' || slot === 'sites' || isTerm(slot);

export const FLAG_KINDS = [
  'empty-required',
  'date-form',
  'space-at-ends',
  'encoding-suspect',
  'name-encoding',
  'cells-count',
  'no-file',
  'not-media',
  'listed-not-media',
  'format-mismatch',
  'exists',
  'unmatched',
  'many-matches',
  'many-values',
  'link-missing',
  'unknown-type',
  'unknown-set',
  'unknown-kind',
  'duplicate-row',
  'unknown-term',
  'template-missing',
  'type-exists',
  'encoding-unsupported',
] as const;
export type FlagKind = (typeof FLAG_KINDS)[number];

/** The kinds that stop a run. A policy given for one is ignored. */
export const BLOCKING_FLAG_KINDS: readonly FlagKind[] = [
  'unknown-term',
  'template-missing',
  'type-exists',
  'encoding-unsupported',
];

export type FlagPolicy = 'keep' | 'repair' | 'leave';
export const FLAG_POLICIES: readonly FlagPolicy[] = ['keep', 'repair', 'leave'];

export type Visibility = 'public' | 'private';

export type ValueType =
  | 'literal'
  | 'uri'
  | { readonly resource: 'identifier' | 'internalId' }
  | { readonly dataType: string };

export interface FolderSource {
  readonly id: string;
  readonly kind: 'folder';
  readonly root: string;
  readonly path: string;
}

export interface StagingSource {
  readonly id: string;
  readonly kind: 'staging';
}

export type SourcePlan = FolderSource | StagingSource;

export interface Grouping {
  readonly pattern: string | null;
  readonly caseFold: boolean;
}

export type Encoding = 'utf-8' | 'utf-16le' | 'utf-16be' | 'windows-1252' | 'iso-8859-1';
export const ENCODINGS: readonly Encoding[] = ['utf-8', 'utf-16le', 'utf-16be', 'windows-1252', 'iso-8859-1'];
export type Delimiter = 'auto' | ',' | ';' | '\t' | '|' | ':' | ' ';
export const DELIMITERS: readonly Delimiter[] = ['auto', ',', ';', '\t', '|', ':', ' '];
export type Quote = '"' | "'" | 'none';

export interface ReadOptions {
  readonly encoding: 'auto' | Encoding;
  readonly delimiter: Delimiter;
  readonly quote: Quote;
  /** 1-based. */
  readonly headerLine: number;
  readonly trim: boolean;
  /** The workbook sheet to read; absent reads the first. A CSV ignores it. */
  readonly sheetName?: string;
}

/** `index` is the cell's place in the row from 0 and wins; `header` is shown and checked. */
export interface ColumnRef {
  readonly index: number;
  readonly header: string;
}

export interface ColumnOptions {
  readonly split: string | null;
  readonly type: ValueType;
  readonly lang: string | null;
  readonly private: boolean;
}

export interface ColumnPlan {
  readonly column: ColumnRef;
  readonly options: ColumnOptions;
}

export interface Join {
  readonly on: 'identifier' | 'fileName';
  readonly column: ColumnRef;
  readonly normalize: {
    readonly caseFold: boolean;
    readonly leadingZeros: boolean;
    readonly separators: boolean;
    /** Compare only the last this many places of the names; v3 only. */
    readonly lastPlaces?: number;
  };
}

export interface SheetPlan {
  readonly id: string;
  readonly source: string;
  readonly file: string;
  readonly hash: string | null;
  readonly read: ReadOptions;
  readonly rowKind: RecordKind | { readonly column: ColumnRef };
  readonly join: Join | null;
  readonly itemKey: ColumnRef | null;
  readonly columns: readonly ColumnPlan[];
}

export const TAG_IDS = [
  'DateTimeOriginal',
  'Artist',
  'Copyright',
  'ImageDescription',
  'GPS',
  'Model',
  'IPTC By-line',
  'IPTC Caption-Abstract',
  'IPTC Keywords',
  'IPTC Object Name',
  'IPTC Date Created',
  'IPTC City',
  'IPTC Country',
  'XMP dc:title',
  'XMP dc:description',
  'XMP dc:creator',
  'XMP dc:subject',
  'XMP dc:rights',
  'XMP photoshop:DateCreated',
  'XMP lr:hierarchicalSubject',
  'File name',
  'Folder',
  'Size',
  'Pixel dimensions',
  'Format',
] as const;
export type TagId = (typeof TAG_IDS)[number];

export type FileFact = 'name' | 'path' | 'size' | 'format';
export const FILE_FACTS: readonly FileFact[] = ['name', 'path', 'size', 'format'];

export type SourceRef =
  | { readonly from: 'group'; readonly name: string }
  /** 1 is the top folder under the source. */
  | { readonly from: 'folder'; readonly level: number | 'path' }
  | { readonly from: 'column'; readonly sheet: string; readonly column: ColumnRef }
  | { readonly from: 'tag'; readonly tag: TagId; readonly files: 'first' | 'all' | { readonly part: string } }
  | { readonly from: 'file'; readonly fact: FileFact }
  | { readonly from: 'constant'; readonly value: string };

export interface SlotRule {
  readonly sources: readonly SourceRef[];
  readonly combine: 'first' | 'all';
  /** Absent when empty. A value mapped to `""` turns the slot off for it. */
  readonly map?: Readonly<Record<string, string>>;
}

export type SlotRules = { readonly [slot in SlotName]?: SlotRule };

export interface Slots {
  readonly item: SlotRules;
  readonly media: SlotRules;
  readonly itemSet: SlotRules;
}

export type Action = 'create' | 'append' | 'revise' | 'update' | 'replace' | 'delete';
export const ACTIONS: readonly Action[] = ['create', 'append', 'revise', 'update', 'replace', 'delete'];

export type Scope = 'importSets' | 'all' | { readonly sets: readonly number[] };

export interface MatchRule {
  readonly key: Term | 'internalId';
  readonly scope: Scope;
}

export interface ActionPlan {
  readonly action: Action;
  readonly match: { readonly item: MatchRule; readonly media: MatchRule; readonly itemSet: MatchRule };
  readonly unmatched: 'create' | 'leave';
}

export interface Defaults {
  readonly visibility: Visibility;
  readonly owner: 'runner' | number;
  readonly sites: readonly number[];
  readonly template: number | null;
}

export interface NewType {
  readonly label: string;
  readonly from: 'mapping' | { readonly starter: StarterId };
  readonly class: Term | null;
}

export interface NewTerm {
  readonly label: string;
}

export interface FileRef {
  readonly source: string;
  readonly path: string;
}

export interface RowRef {
  readonly sheet: string;
  /** The key column's value plus `#n`, or the line when the sheet has no key column. */
  readonly key: string;
}

export type FileEntryRef = { readonly file: FileRef };
export type EntryRef = FileEntryRef | { readonly row: RowRef };

export type MoveTo = 'own' | 'leave' | { readonly with: FileEntryRef };

export interface DecisionValue {
  readonly type: ValueType;
  readonly text: string;
  readonly lang: string | null;
  readonly label: string | null;
}

/** What a `many` decision does to every entry it names. */
export type Change =
  | { readonly kind: 'move'; readonly to: MoveTo }
  | { readonly kind: 'leave' }
  | { readonly kind: 'set'; readonly target: RecordKind; readonly slot: SlotName; readonly values: readonly DecisionValue[] }
  | { readonly kind: 'match'; readonly target: RecordKind; readonly resource: number | null }
  | { readonly kind: 'flag'; readonly flag: FlagKind; readonly slot: SlotName | null; readonly policy: FlagPolicy };

export type Decision =
  | { readonly kind: 'move'; readonly file: FileEntryRef; readonly to: MoveTo }
  | { readonly kind: 'leave'; readonly entry: EntryRef }
  | {
      readonly kind: 'set';
      readonly entry: EntryRef;
      readonly target: RecordKind;
      readonly slot: SlotName;
      readonly values: readonly DecisionValue[];
    }
  | { readonly kind: 'match'; readonly entry: EntryRef; readonly target: RecordKind; readonly resource: number | null }
  | {
      readonly kind: 'flag';
      readonly entry: EntryRef;
      readonly flag: FlagKind;
      readonly slot: SlotName | null;
      readonly policy: FlagPolicy;
    }
  | { readonly kind: 'many'; readonly entries: readonly EntryRef[]; readonly change: Change }
  | { readonly kind: 'pair'; readonly row: RowRef; readonly file: FileRef };

/** How the derive answers the anomalies the Sources screen shows. */
export interface AnomalyAnswers {
  readonly identical: 'keepOne' | 'keepAll';
  readonly unmatchedFiles: 'leave' | 'own';
  readonly itemsWithoutRow: 'keep' | 'leave';
}

export interface ImportPlan {
  readonly version: 2;
  readonly sources: readonly SourcePlan[];
  readonly grouping: Grouping;
  /** Admin state: the server keeps it and never reads it. */
  readonly example: Readonly<Record<string, JsonValue>> | null;
  readonly sheets: readonly SheetPlan[];
  readonly slots: Slots;
  readonly action: ActionPlan;
  readonly defaults: Defaults;
  readonly flags: { readonly [kind in FlagKind]?: FlagPolicy };
  readonly decisions: readonly Decision[];
  readonly newTypes: readonly NewType[];
  readonly newTerms: readonly NewTerm[];
  /** Absent until the person answers one; the server then reads the defaults. */
  readonly anomalies?: AnomalyAnswers;
  /** Admin state: what Simple's screens keep. The server keeps it and never reads it; absent until Simple saves one. */
  readonly simple?: Readonly<Record<string, JsonValue>>;
}

/**
 * The import plan, version 3 (`plan/v3.rs`): v2's `grouping`, `example`,
 * `sheets` and `slots` move into the series of `structure`. The admin still
 * writes v2; this is what it reads when it switches.
 */
export interface ImportPlanV3 {
  readonly version: 3;
  readonly sources: readonly SourcePlan[];
  readonly structure: Structure;
  readonly action: ActionPlan;
  readonly defaults: Defaults;
  readonly flags: ImportPlan['flags'];
  readonly decisions: readonly Decision[];
  readonly newTypes: readonly NewType[];
  readonly newTerms: readonly NewTerm[];
  readonly anomalies?: AnomalyAnswers;
  /** Admin state keyed by level id. The server keeps it and never reads it. */
  readonly simple?: Readonly<Record<string, JsonValue>>;
  /** The person's edits on the proposal, in the order made; absent when there are none. The server replays them. */
  readonly edits?: readonly Edit[];
}

/** Unique in the plan; `simple` keys on it. A string, as a sheet id is. */
export type LevelId = string;

export interface Structure {
  readonly levels: readonly Level[];
  /** Sets every item of the import also joins, after its tree sets. */
  readonly freeSets: readonly SetSource[];
}

export type Level = SetLevel | Series;

/** Puts every item of every series under it into its set. */
export interface SetLevel {
  readonly kind: 'set';
  readonly id: LevelId;
  readonly set: SetSource;
  readonly levels: readonly Level[];
}

/** A leaf: the files in its scope, grouped by its own pattern, described by its own lists and slots. */
export interface Series {
  readonly kind: 'series';
  readonly id: LevelId;
  readonly set: SetSource | null;
  readonly scope: SeriesScope;
  /** The pattern matches each path relative to the scope folder that holds it. */
  readonly grouping: Grouping;
  readonly example: Readonly<Record<string, JsonValue>> | null;
  readonly lists: readonly SheetPlan[];
  /** Never an `item.sets` rule: memberships are levels and free sets. */
  readonly slots: Slots;
}

export type SeriesScope = 'all' | readonly FolderScope[];

/** `folder: ""` is the whole source. */
export interface FolderScope {
  readonly source: string;
  readonly folder: string;
}

/** A list file by where it is. Edits and the proposal name lists this way, never by sheet id. */
export interface ListFile {
  readonly source: string;
  readonly file: string;
}

export type JoinNormalize = Join['normalize'];

/** Where a set takes its name or names from (`plan/edits.rs`). */
export type SetFrom =
  | { readonly from: 'folder'; readonly scope: FolderScope }
  | { readonly from: 'column'; readonly list: ListFile; readonly column: ColumnRef }
  | { readonly from: 'namePlace'; readonly scope: FolderScope; readonly place: number }
  | { readonly from: 'folderLevel'; readonly scope: FolderScope; readonly depth: number };

/** How a list's rows find their files. `lastPlaces` is offered; the plan cannot state it yet. */
export type PairRule =
  | { readonly rule: 'folds'; readonly normalize: JoinNormalize }
  | { readonly rule: 'column'; readonly column: ColumnRef; readonly on: Join['on'] }
  | { readonly rule: 'lastPlaces'; readonly places: number };

/** The cut an `items` edit asks for. The proposer works the shape out again. */
export type CutEdit =
  | { readonly cut: 'whole' }
  | { readonly cut: 'places'; readonly places: number }
  | { readonly cut: 'folder'; readonly depth: number };

/** One act of the person on a proposal, named by what it changes in the archive, never by a level id. */
export type Edit =
  | { readonly edit: 'leaveOut'; readonly scope: FolderScope }
  | { readonly edit: 'keep'; readonly scope: FolderScope }
  | { readonly edit: 'items'; readonly scope: FolderScope; readonly cut: CutEdit }
  | { readonly edit: 'list'; readonly scope: FolderScope; readonly list: ListFile | null }
  | { readonly edit: 'grain'; readonly list: ListFile; readonly describes: RecordKind }
  | { readonly edit: 'addSet'; readonly from: SetFrom }
  | { readonly edit: 'dropSet'; readonly from: SetFrom }
  | { readonly edit: 'pair'; readonly list: ListFile; readonly rule: PairRule }
  | { readonly edit: 'title'; readonly from: SetFrom; readonly title: string };

/** The shape says what the text means: an install id, a title, or one set per value. */
export type SetSource = SetById | SetByTitle | { readonly each: EachSet };

export interface SetById {
  readonly id: number;
}

export interface SetByTitle {
  readonly title: string;
  /** Written as `dcterms:description` when the run makes the set. */
  readonly description: string | null;
}

export interface EachSet {
  readonly sources: readonly SourceRef[];
  readonly combine: 'first' | 'all';
  /** Absent when empty. A value mapped to `""` adds no set. */
  readonly map?: Readonly<Record<string, string>>;
  readonly names: SetNames;
}

/** `text` is v2's reading (an id that exists, else a record key, else a title); only the lift writes it. */
export type SetNames = 'title' | 'id' | 'key' | 'text';
export const SET_NAMES: readonly SetNames[] = ['title', 'id', 'key', 'text'];

export const importIdFromParam = (param: string): ImportId | null => {
  if (!/^\d+$/.test(param)) {
    return null;
  }
  const value = Number.parseInt(param, 10);
  return Number.isSafeInteger(value) && value > 0 ? brandId<'ImportId'>(value) : null;
};
