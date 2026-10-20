/**
 * The derived model as a preview shows it (`docs/import-v2.md`, "The
 * derive"): records, the entries of the evidence and where they landed, and
 * every finding. The shapes are `crates/omeka-api/src/import/model.rs`.
 *
 * Rust enums with a payload are `{ "variant": payload }` and unit variants
 * are bare strings. Two things differ from what the contract's prose
 * suggests: the `walk` anomaly's `unusable_names` is snake case (the enum
 * renames its variants, not their members), and a `Subject` that is the plan
 * is the bare string `"plan"`.
 */
import {
  asCount,
  asFlag,
  list,
  member,
  nullable,
  oneOf,
  strict,
  variants,
  wordOr,
} from './import-read';
import { asSlotName, asTerm, parseEntryRef, parseFileRef, parseRowRef, parseValueType } from './import-plan-read';
import type { EntryRef, FileRef, FlagKind, FlagPolicy, RecordKind, RowRef, SlotName, Term, ValueType, Visibility } from './import-plan';
import { FLAG_KINDS, FLAG_POLICIES, RECORD_KINDS } from './import-plan';
import { ParseError, asNumber, asRecord, asString, brandId, isUnknownArray } from './parse';
import type { Id, Parser, Path } from './types';

/** A record's place in the model; the number a preview filter and a flag name it by. */
export type RecordId = Id<'RecordId'>;
export const asRecordId = (value: unknown, path: Path): RecordId => brandId<'RecordId'>(asCount(value, path));

export type GroupKey = { readonly pattern: string } | { readonly own: FileRef };
export type Origin = { readonly group: GroupKey } | { readonly file: FileRef } | { readonly row: RowRef };
export type Parent =
  | { readonly record: RecordId }
  | { readonly existing: number }
  | { readonly missing: string };

export type Link = { readonly existing: number } | { readonly record: RecordId };

/** A value as the derive wrote it; `link` says what a `resource` value points at. */
export interface SlotValue {
  readonly type: ValueType;
  readonly text: string;
  readonly lang: string | null;
  readonly label: string | null;
  readonly isPublic: boolean;
  readonly link: Link | null;
}

export type SetRef =
  | { readonly existing: number }
  | { readonly record: RecordId }
  /** An index into the check's `newSets`. */
  | { readonly new: number };

export type TemplateRef =
  | { readonly existing: number }
  /** An index into the plan's `newTypes`. */
  | { readonly new: number };

export type OwnerRef =
  | { readonly is: 'runner' | number }
  /** A value that names no user: the record gets the plan's default owner. */
  | { readonly unknown: string };

export interface Resolved {
  readonly sets: readonly SetRef[];
  readonly template: TemplateRef | null;
  readonly class: Term | null;
  readonly owner: OwnerRef;
  readonly visibility: Visibility;
  readonly sites: readonly number[];
  readonly mediaOrder: string | null;
  readonly ingester: string | null;
  readonly mediaSource: string | null;
  readonly itemKey: string | null;
}

export type LeftBy =
  /** A `leave` or `move` decision, by its index in the plan. */
  | { readonly decision: number }
  | { readonly flag: FlagKind }
  | 'parent'
  | 'noFiles'
  /** A copy of a file the import keeps (`anomalies.identical: keepOne`). */
  | 'identical'
  /** An item no row of the list joined (`anomalies.itemsWithoutRow: leave`). */
  | 'noRow';

export type Fate = 'create' | { readonly act: number } | { readonly left: LeftBy };

export type Subject =
  | { readonly record: RecordId }
  | { readonly entry: EntryRef }
  | { readonly sheet: string }
  | 'plan';

export interface Flag {
  readonly kind: FlagKind;
  readonly subject: Subject;
  readonly slot: SlotName | null;
  /** `null` for a blocking kind. */
  readonly policy: FlagPolicy | null;
  /** The value, the refused type, or what else the flag names. */
  readonly detail: string | null;
}

export interface ModelRecord {
  readonly id: RecordId;
  readonly kind: RecordKind;
  readonly origin: Origin;
  readonly files: readonly FileRef[];
  readonly rows: readonly RowRef[];
  readonly parent: Parent | null;
  readonly media: readonly RecordId[];
  readonly slots: { readonly [slot in SlotName]?: readonly SlotValue[] };
  readonly resolved: Resolved;
  readonly matches: readonly number[];
  readonly fate: Fate;
}

/** A record with the flags on it, as `records` and the shapes' samples answer it. */
export interface RecordView extends ModelRecord {
  readonly flags: readonly Flag[];
}

export interface EntryState {
  readonly entry: EntryRef;
  readonly record: RecordId | null;
  readonly left: LeftBy | null;
  /** A file the pattern matched no `identifier` in. */
  readonly unmatched: boolean;
  /** A file with a sniffed media type: the admin offers it as an example. */
  readonly example: boolean;
}

export interface EntryView extends EntryState {
  readonly flags: readonly Flag[];
}

export interface NewSet {
  readonly title: string;
  readonly visibility: Visibility;
  readonly isOpen: boolean;
}

export type Anomaly =
  | { readonly spanFolders: { readonly record: RecordId; readonly folders: readonly string[] } }
  | { readonly largeGroup: { readonly record: RecordId; readonly files: number; readonly median: number } }
  | { readonly outsideScope: { readonly record: RecordId; readonly key: string; readonly ids: readonly number[] } }
  | { readonly unmappedColumns: { readonly sheet: string; readonly columns: readonly (readonly [number, string])[] } }
  | {
      readonly decisionOverlap: {
        readonly record: RecordId;
        readonly slot: SlotName;
        readonly decisions: readonly number[];
      };
    }
  | { readonly unusedEntries: { readonly entries: readonly EntryRef[] } }
  | {
      readonly walk: {
        readonly source: string;
        readonly symlinks: number;
        readonly dot: number;
        readonly special: number;
        readonly unreadable: readonly string[];
        readonly unusable_names: readonly string[];
        readonly collisions: readonly string[];
        readonly decoded: readonly string[];
      };
    }
  | { readonly sheetChanged: { readonly sheet: string; readonly stored: string; readonly now: string } }
  | { readonly headerNotes: { readonly sheet: string; readonly notes: readonly string[] } }
  | { readonly sheetUnread: { readonly sheet: string; readonly detail: string } };

export type OrphanReason = 'missingEntry' | 'wrongTarget' | 'lineKeyChanged' | 'noGroup';
export const ORPHAN_REASONS: readonly OrphanReason[] = ['missingEntry', 'wrongTarget', 'lineKeyChanged', 'noGroup'];

export interface Orphan {
  /** The decision's index in the plan. */
  readonly decision: number;
  readonly reason: OrphanReason;
  /** The entry the reason is about, not the whole decision. */
  readonly entry: EntryRef;
}

/** Why the file pattern groups nothing, or the group names it writes. */
export type PatternState =
  | { readonly groups: readonly string[] }
  | 'missing'
  | 'noIdentifier'
  | { readonly syntax: string };

const text = (object: Record<string, unknown>, key: string, path: Path): string => member(object, key, path, asString);
const maybeText = nullable(asString);
const count = (object: Record<string, unknown>, key: string, path: Path): number => member(object, key, path, asCount);

const parseLink = variants<Link>({
  existing: (value, path) => ({ existing: asCount(value, path) }),
  record: (value, path) => ({ record: asRecordId(value, path) }),
});

export const parseSlotValue: Parser<SlotValue> = (raw, path = []) => {
  const object = strict(raw, path, ['type', 'text', 'lang', 'label', 'isPublic', 'link']);
  return {
    type: member(object, 'type', path, parseValueType),
    text: text(object, 'text', path),
    lang: member(object, 'lang', path, maybeText),
    label: member(object, 'label', path, maybeText),
    isPublic: member(object, 'isPublic', path, asFlag),
    link: member(object, 'link', path, nullable(parseLink)),
  };
};

const parseGroupKey = variants<GroupKey>({
  pattern: (value, path) => ({ pattern: asString(value, path) }),
  own: (value, path) => ({ own: parseFileRef(value, path) }),
});

const parseOrigin = variants<Origin>({
  group: (value, path) => ({ group: parseGroupKey(value, path) }),
  file: (value, path) => ({ file: parseFileRef(value, path) }),
  row: (value, path) => ({ row: parseRowRef(value, path) }),
});

const parseParent = variants<Parent>({
  record: (value, path) => ({ record: asRecordId(value, path) }),
  existing: (value, path) => ({ existing: asCount(value, path) }),
  missing: (value, path) => ({ missing: asString(value, path) }),
});

const parseSetRef = variants<SetRef>({
  existing: (value, path) => ({ existing: asCount(value, path) }),
  record: (value, path) => ({ record: asRecordId(value, path) }),
  new: (value, path) => ({ new: asCount(value, path) }),
});

const parseTemplateRef = variants<TemplateRef>({
  existing: (value, path) => ({ existing: asCount(value, path) }),
  new: (value, path) => ({ new: asCount(value, path) }),
});

const parseOwnerRef = variants<OwnerRef>({
  is: (value, path) => ({ is: wordOr<'runner', number>(['runner'], asCount)(value, path) }),
  unknown: (value, path) => ({ unknown: asString(value, path) }),
});

const asVisibility = oneOf<Visibility>(['public', 'private']);

const parseResolved: Parser<Resolved> = (raw, path = []) => {
  const object = strict(raw, path, [
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
  ]);
  return {
    sets: member(object, 'sets', path, list(parseSetRef)),
    template: member(object, 'template', path, nullable(parseTemplateRef)),
    class: member(object, 'class', path, nullable(asTerm)),
    owner: member(object, 'owner', path, parseOwnerRef),
    visibility: member(object, 'visibility', path, asVisibility),
    sites: member(object, 'sites', path, list(asCount)),
    mediaOrder: member(object, 'mediaOrder', path, maybeText),
    ingester: member(object, 'ingester', path, maybeText),
    mediaSource: member(object, 'mediaSource', path, maybeText),
    itemKey: member(object, 'itemKey', path, maybeText),
  };
};

const asFlagKind = oneOf(FLAG_KINDS);

const parseLeftBy = wordOr<'parent' | 'noFiles' | 'identical' | 'noRow', LeftBy>(
  ['parent', 'noFiles', 'identical', 'noRow'],
  variants<LeftBy>({
    decision: (value, path) => ({ decision: asCount(value, path) }),
    flag: (value, path) => ({ flag: asFlagKind(value, path) }),
  }),
);

const parseFate = wordOr<'create', Fate>(
  ['create'],
  variants<Fate>({
    act: (value, path) => ({ act: asCount(value, path) }),
    left: (value, path) => ({ left: parseLeftBy(value, path) }),
  }),
);

const parseSubject = wordOr<'plan', Subject>(
  ['plan'],
  variants<Subject>({
    record: (value, path) => ({ record: asRecordId(value, path) }),
    entry: (value, path) => ({ entry: parseEntryRef(value, path) }),
    sheet: (value, path) => ({ sheet: asString(value, path) }),
  }),
);

export const parseSubjectOf = parseSubject;

export const parseFlag: Parser<Flag> = (raw, path = []) => {
  const object = strict(raw, path, ['kind', 'subject', 'slot', 'policy', 'detail']);
  return {
    kind: member(object, 'kind', path, asFlagKind),
    subject: member(object, 'subject', path, parseSubject),
    slot: member(object, 'slot', path, nullable(asSlotName)),
    policy: member(object, 'policy', path, nullable(oneOf(FLAG_POLICIES))),
    detail: member(object, 'detail', path, maybeText),
  };
};

const RECORD_KEYS = ['id', 'kind', 'origin', 'files', 'rows', 'parent', 'media', 'slots', 'resolved', 'matches', 'fate'];

const parseSlots = (raw: unknown, path: Path): ModelRecord['slots'] => {
  const slots: { [slot in SlotName]?: readonly SlotValue[] } = {};
  for (const [name, values] of Object.entries(asRecord(raw, path))) {
    slots[asSlotName(name, [...path, name])] = list(parseSlotValue)(values, [...path, name]);
  }
  return slots;
};

const recordFields = (object: Record<string, unknown>, path: Path): ModelRecord => ({
  id: member(object, 'id', path, asRecordId),
  kind: member(object, 'kind', path, oneOf(RECORD_KINDS)),
  origin: member(object, 'origin', path, parseOrigin),
  files: member(object, 'files', path, list(parseFileRef)),
  rows: member(object, 'rows', path, list(parseRowRef)),
  parent: member(object, 'parent', path, nullable(parseParent)),
  media: member(object, 'media', path, list(asRecordId)),
  slots: member(object, 'slots', path, parseSlots),
  resolved: member(object, 'resolved', path, parseResolved),
  matches: member(object, 'matches', path, list(asNumber)),
  fate: member(object, 'fate', path, parseFate),
});

export const parseRecordView: Parser<RecordView> = (raw, path = []) => {
  const object = strict(raw, path, [...RECORD_KEYS, 'flags']);
  return { ...recordFields(object, path), flags: member(object, 'flags', path, list(parseFlag)) };
};

const entryFields = (object: Record<string, unknown>, path: Path): EntryState => ({
  entry: member(object, 'entry', path, parseEntryRef),
  record: member(object, 'record', path, nullable(asRecordId)),
  left: member(object, 'left', path, nullable(parseLeftBy)),
  unmatched: member(object, 'unmatched', path, asFlag),
  example: member(object, 'example', path, asFlag),
});

export const parseEntryView: Parser<EntryView> = (raw, path = []) => {
  const object = strict(raw, path, ['entry', 'record', 'left', 'unmatched', 'example', 'flags']);
  return { ...entryFields(object, path), flags: member(object, 'flags', path, list(parseFlag)) };
};

export const parseNewSet: Parser<NewSet> = (raw, path = []) => {
  const object = strict(raw, path, ['title', 'visibility', 'isOpen']);
  return {
    title: text(object, 'title', path),
    visibility: member(object, 'visibility', path, asVisibility),
    isOpen: member(object, 'isOpen', path, asFlag),
  };
};

const strings = (object: Record<string, unknown>, key: string, path: Path): readonly string[] =>
  member(object, key, path, list(asString));

const parseColumnTuple = (raw: unknown, path: Path): readonly [number, string] => {
  if (!isUnknownArray(raw) || raw.length !== 2) {
    throw new ParseError(path, 'a pair of a column index and a header');
  }
  return [asCount(raw[0], [...path, 0]), asString(raw[1], [...path, 1])];
};

export const parseAnomaly = variants<Anomaly>({
  spanFolders: (raw, path) => {
    const object = strict(raw, path, ['record', 'folders']);
    return { spanFolders: { record: member(object, 'record', path, asRecordId), folders: strings(object, 'folders', path) } };
  },
  largeGroup: (raw, path) => {
    const object = strict(raw, path, ['record', 'files', 'median']);
    return {
      largeGroup: {
        record: member(object, 'record', path, asRecordId),
        files: count(object, 'files', path),
        median: count(object, 'median', path),
      },
    };
  },
  outsideScope: (raw, path) => {
    const object = strict(raw, path, ['record', 'key', 'ids']);
    return {
      outsideScope: {
        record: member(object, 'record', path, asRecordId),
        key: text(object, 'key', path),
        ids: member(object, 'ids', path, list(asCount)),
      },
    };
  },
  unmappedColumns: (raw, path) => {
    const object = strict(raw, path, ['sheet', 'columns']);
    return {
      unmappedColumns: { sheet: text(object, 'sheet', path), columns: member(object, 'columns', path, list(parseColumnTuple)) },
    };
  },
  decisionOverlap: (raw, path) => {
    const object = strict(raw, path, ['record', 'slot', 'decisions']);
    return {
      decisionOverlap: {
        record: member(object, 'record', path, asRecordId),
        slot: member(object, 'slot', path, asSlotName),
        decisions: member(object, 'decisions', path, list(asCount)),
      },
    };
  },
  unusedEntries: (raw, path) => {
    const object = strict(raw, path, ['entries']);
    return { unusedEntries: { entries: member(object, 'entries', path, list(parseEntryRef)) } };
  },
  walk: (raw, path) => {
    const object = strict(raw, path, [
      'source',
      'symlinks',
      'dot',
      'special',
      'unreadable',
      'unusable_names',
      'collisions',
      'decoded',
    ]);
    return {
      walk: {
        source: text(object, 'source', path),
        symlinks: count(object, 'symlinks', path),
        dot: count(object, 'dot', path),
        special: count(object, 'special', path),
        unreadable: strings(object, 'unreadable', path),
        unusable_names: strings(object, 'unusable_names', path),
        collisions: strings(object, 'collisions', path),
        decoded: strings(object, 'decoded', path),
      },
    };
  },
  sheetChanged: (raw, path) => {
    const object = strict(raw, path, ['sheet', 'stored', 'now']);
    return { sheetChanged: { sheet: text(object, 'sheet', path), stored: text(object, 'stored', path), now: text(object, 'now', path) } };
  },
  headerNotes: (raw, path) => {
    const object = strict(raw, path, ['sheet', 'notes']);
    return { headerNotes: { sheet: text(object, 'sheet', path), notes: strings(object, 'notes', path) } };
  },
  sheetUnread: (raw, path) => {
    const object = strict(raw, path, ['sheet', 'detail']);
    return { sheetUnread: { sheet: text(object, 'sheet', path), detail: text(object, 'detail', path) } };
  },
});

export const parseOrphan: Parser<Orphan> = (raw, path = []) => {
  const object = strict(raw, path, ['decision', 'reason', 'entry']);
  return {
    decision: count(object, 'decision', path),
    reason: member(object, 'reason', path, oneOf(ORPHAN_REASONS)),
    entry: member(object, 'entry', path, parseEntryRef),
  };
};

export const parsePatternState = wordOr<'missing' | 'noIdentifier', PatternState>(
  ['missing', 'noIdentifier'],
  variants<PatternState>({
    groups: (value, path) => ({ groups: list(asString)(value, path) }),
    syntax: (value, path) => ({ syntax: asString(value, path) }),
  }),
);
