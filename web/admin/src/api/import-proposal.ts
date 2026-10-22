/**
 * The `proposal` section of the preview (`crates/omeka-api/src/import/preview.rs`):
 * the structure the server proposes and the counts that show each choice.
 *
 * `import-sections.ts` keeps the section unparsed (`RawProposal`), so this
 * parser is not in the chunk that every import route loads. A screen that
 * shows the proposal calls `parseProposal` on it.
 */
import { asCount, asFlag, list, member, nullable, oneOf, strict } from './import-read';
import { parseColumnRef } from './import-plan-read';
import { parseEdit, parseFolderScope, parseListFile, parsePairRule, parseSetFrom, parseStructure } from './import-plan-tree-read';
import type { ColumnRef, Edit, FolderScope, LevelId, ListFile, PairRule, RecordKind, SetFrom, Structure } from './import-plan';
import { RECORD_KINDS } from './import-plan';
import { MEDIA_KINDS, parseBucketCount, parseGrain, parseListCandidate } from './import-sections';
import type { BucketCount, Grain, ListCandidate, MediaKind } from './import-sections';
import { ParseError, asRecord, asString } from './parse';
import type { Path } from './types';

const text = (object: Record<string, unknown>, key: string, path: Path): string => member(object, key, path, asString);
const count = (object: Record<string, unknown>, key: string, path: Path): number => member(object, key, path, asCount);

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

/** A choice the proposer made, the next best choice, and every other choice it counted; an edit may ask for any of them. */
export interface Choice<T, C> {
  readonly chosen: Pick<T, C>;
  readonly rival: Pick<T, C> | null;
  readonly others: readonly Pick<T, C>[];
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
  | { readonly cut: 'places'; readonly places: number; readonly rule: 'recurringParts' | 'namesEndHere' | 'sameAsSibling' | 'recurringInItem' }
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

/** A list's grain evidence, and what one answer makes of the series. */
export interface GrainCounts extends Grain {
  /** The list's rows that name no file with this answer. */
  readonly rowsNamingNoFile: number;
  /** The items the series keeps with this answer. */
  readonly items: number;
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
  readonly grain: Choice<RecordKind, GrainCounts> | null;
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

/** Why an edit is not applied: what it names is not in the scan, its choice is not offered, or a later edit answers its question. */
export const NOT_APPLIED = ['gone', 'notOffered', 'replaced'] as const;
export type NotApplied = (typeof NOT_APPLIED)[number];

/** An edit of the plan; `reason` is null exactly when it is applied. */
export interface EditResult {
  readonly edit: Edit;
  readonly applied: boolean;
  readonly reason: NotApplied | null;
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
  readonly edits: readonly EditResult[];
  /** Whether `structure` is the plan's own. */
  readonly matchesPlan: boolean;
}

const parseGrainCounts = (raw: unknown, path: Path): GrainCounts => {
  const object = asRecord(raw, path);
  const grain = Object.fromEntries(Object.entries(object).filter(([key]) => key !== 'rowsNamingNoFile' && key !== 'items'));
  return { ...parseGrain(grain, path), rowsNamingNoFile: count(object, 'rowsNamingNoFile', path), items: count(object, 'items', path) };
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
    const object = strict(raw, path, ['chosen', 'rival', 'others']);
    return {
      chosen: member(object, 'chosen', path, pick),
      rival: member(object, 'rival', path, nullable(pick)),
      others: member(object, 'others', path, list(pick)),
    };
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
        rule: member(only, 'rule', path, oneOf(['recurringParts', 'namesEndHere', 'sameAsSibling', 'recurringInItem'] as const)),
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
    grain: member(only, 'grain', path, nullable(parseChoice(oneOf(RECORD_KINDS), parseGrainCounts))),
    set: member(only, 'set', path, parseChoice(nullable(parseSetFrom), parseSetCounts)),
    splits: member(only, 'splits', path, list(parsePick(parseSetFrom, parseSetCounts))),
    pairing: member(only, 'pairing', path, list(parsePick(parsePairRule, parsePairCounts))),
  };
};

/** A proposal, from the preview's `RawProposal` or any JSON; refuses what the server's serde types refuse. */
export const parseProposal = (raw: unknown, path: Path = ['proposal']): Proposal => {
  const object = strict(raw, path, ['structure', 'nodes', 'topSets', 'outside', 'unattached', 'edits', 'matchesPlan']);
  const outside = (value: unknown, at: Path): OutsideFolder => {
    const each = strict(value, at, [...SCOPE_COUNTS, 'reason']);
    return { ...scopeCounts(each, at), reason: member(each, 'reason', at, oneOf(['noMedia', 'edit'] as const)) };
  };
  const edit = (value: unknown, at: Path): EditResult => {
    const each = strict(value, at, ['edit', 'applied', 'reason']);
    return {
      edit: member(each, 'edit', at, parseEdit),
      applied: member(each, 'applied', at, asFlag),
      reason: member(each, 'reason', at, nullable(oneOf(NOT_APPLIED))),
    };
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