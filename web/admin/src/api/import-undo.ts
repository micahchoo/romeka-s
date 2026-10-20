/**
 * What an undo reports: `UndoReport` in
 * `crates/omeka-api/src/import2/undo/report.rs`, which the server keeps in the
 * import's summary and shows as `undoReport`. The fixtures
 * `tests/fixtures/import/undo-report-*.json` come from the undo job itself
 * (`crates/omeka-cli/tests/import_undo_wire.rs`).
 */
import { asCount, asFlag, list, member, nullable, oneOf, strict } from './import-read';
import { ParseError, asString } from './parse';
import type { Parser, Path } from './types';

export const RESOURCE_KINDS = ['item', 'media', 'itemSet', 'resourceTemplate', 'property'] as const;
export type ResourceKind = (typeof RESOURCE_KINDS)[number];

export interface NamedResource {
  readonly kind: ResourceKind;
  readonly id: number;
}

/** Why an undo keeps a resource. `laterImport` names the import that changed it. */
export const TOUCH_REASONS = [
  'edited',
  'linked',
  'mediaAdded',
  'membersAdded',
  'laterImport',
  'inUse',
  'itemKept',
  'memberKept',
] as const;
export type TouchReason = (typeof TOUCH_REASONS)[number];

export interface KeptReason {
  readonly reason: TouchReason;
  /** Only for `laterImport`. */
  readonly import?: number;
  /** The catalogue key of the message, `server.importUndo…`. */
  readonly key: string;
  /** The message in English. */
  readonly detail: string;
}

export interface KeptResource extends NamedResource {
  readonly reasons: readonly KeptReason[];
}

/** Resources by kind. */
export interface KindCounts {
  readonly items: number;
  readonly media: number;
  readonly itemSets: number;
  readonly templates: number;
  readonly properties: number;
}

export interface UndoStopped {
  readonly key: string;
  readonly detail: string;
}

export interface UndoReport {
  readonly progress: { readonly done: number; readonly total: number };
  readonly removed: KindCounts;
  readonly restored: KindCounts;
  readonly kept: readonly KeptResource[];
  /** Already gone when the undo reached them. */
  readonly skipped: readonly NamedResource[];
  /** Restored or deleted although touched, because the request named them. */
  readonly forced: readonly NamedResource[];
  /** A stop was asked for. */
  readonly stopped: boolean;
  readonly error: UndoStopped | null;
}

const parseResource: Parser<NamedResource> = (raw, path = []) => {
  const object = strict(raw, path, ['kind', 'id']);
  return { kind: member(object, 'kind', path, oneOf(RESOURCE_KINDS)), id: member(object, 'id', path, asCount) };
};

const parseReason: Parser<KeptReason> = (raw, path = []) => {
  const object = strict(raw, path, ['reason', 'key', 'detail'], ['import']);
  const reason = member(object, 'reason', path, oneOf(TOUCH_REASONS));
  const base = {
    reason,
    key: member(object, 'key', path, asString),
    detail: member(object, 'detail', path, asString),
  };
  if (reason !== 'laterImport') {
    if (object['import'] !== undefined) {
      throw new ParseError([...path, 'import'], 'absent unless the reason is laterImport');
    }
    return base;
  }
  return { ...base, import: member(object, 'import', path, asCount) };
};

const parseKept: Parser<KeptResource> = (raw, path = []) => {
  const object = strict(raw, path, ['kind', 'id', 'reasons']);
  return {
    kind: member(object, 'kind', path, oneOf(RESOURCE_KINDS)),
    id: member(object, 'id', path, asCount),
    reasons: member(object, 'reasons', path, list(parseReason)),
  };
};

const parseCounts: Parser<KindCounts> = (raw, path = []) => {
  const object = strict(raw, path, ['items', 'media', 'itemSets', 'templates', 'properties']);
  return {
    items: member(object, 'items', path, asCount),
    media: member(object, 'media', path, asCount),
    itemSets: member(object, 'itemSets', path, asCount),
    templates: member(object, 'templates', path, asCount),
    properties: member(object, 'properties', path, asCount),
  };
};

const parseStopped: Parser<UndoStopped> = (raw, path = []) => {
  const object = strict(raw, path, ['key', 'detail']);
  return { key: member(object, 'key', path, asString), detail: member(object, 'detail', path, asString) };
};

/** What an undo would do now, read without writing (`GET …/undo/preview`, `UndoPreview` in `undo/preview.rs`). */
export interface UndoPreview {
  /** What the import made that the undo would delete. */
  readonly remove: KindCounts;
  /** What the import changed that the undo would put back. */
  readonly restore: KindCounts;
  readonly kept: readonly KeptResource[];
  readonly skipped: readonly NamedResource[];
  readonly forced: readonly NamedResource[];
}

export const parseUndoPreview: Parser<UndoPreview> = (raw, path = []) => {
  const object = strict(raw, path, ['remove', 'restore', 'kept', 'skipped', 'forced']);
  return {
    remove: member(object, 'remove', path, parseCounts),
    restore: member(object, 'restore', path, parseCounts),
    kept: member(object, 'kept', path, list(parseKept)),
    skipped: member(object, 'skipped', path, list(parseResource)),
    forced: member(object, 'forced', path, list(parseResource)),
  };
};

export const parseUndoReport: Parser<UndoReport> = (raw, path = []) => {
  const object = strict(raw, path, ['progress', 'removed', 'restored', 'kept', 'skipped', 'forced', 'stopped', 'error']);
  const progress = (value: unknown, at: Path): UndoReport['progress'] => {
    const each = strict(value, at, ['done', 'total']);
    return { done: member(each, 'done', at, asCount), total: member(each, 'total', at, asCount) };
  };
  return {
    progress: member(object, 'progress', path, progress),
    removed: member(object, 'removed', path, parseCounts),
    restored: member(object, 'restored', path, parseCounts),
    kept: member(object, 'kept', path, list(parseKept)),
    skipped: member(object, 'skipped', path, list(parseResource)),
    forced: member(object, 'forced', path, list(parseResource)),
    stopped: member(object, 'stopped', path, asFlag),
    error: member(object, 'error', path, nullable(parseStopped)),
  };
};
