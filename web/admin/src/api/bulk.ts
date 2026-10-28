/**
 * The bulk change routes (`/api/romeka/bulk`, `crates/omeka-server/src/bulk.rs`): one change
 * applied to every item a browse query matches, or to the ticked items, with an undo.
 *
 * Every answer is parsed here, strictly: a member the client does not know fails the parse, so a
 * server change fails `tests/bulk-api.test.ts` before it reaches a person. A 409 keeps its
 * `code`, which `bulkRefusal` turns into the few cases the panel words differently.
 */
import { ApiError, api } from './client';
import type { ResourceQuery } from './client';
import { asCount, asFlag, list, member, nullable, oneOf, strict } from './import-read';
import { asIdNumber, asJsonValue, asString, brandId, isRecord } from './parse';
import type { FieldChange, ValueOp } from './bulk-values';
import { parseFieldChange, parseValueOps } from './bulk-values';
import type { ClassId, Id, ItemId, ItemSetId, Parser, Path, SiteId, TemplateId, UserId } from './types';

export type BulkId = Id<'BulkId'>;

/** `BulkStatus` in `crates/omeka-db/src/rows/bulk.rs`. */
export const BULK_STATUSES = [
  'previewed',
  'running',
  'stopping',
  'stopped',
  'done',
  'failed',
  'undoing',
  'undo_stopping',
  'undone',
  'partly_undone',
] as const;
export type BulkStatus = (typeof BULK_STATUSES)[number];

/** The states a job works in: the panel reads the run again while it is in one. */
export const isLive = (status: BulkStatus): boolean =>
  status === 'running' || status === 'stopping' || status === 'undoing' || status === 'undo_stopping';

/** `BulkStatus::may_undo`. */
export const mayUndo = (status: BulkStatus): boolean =>
  status === 'done' || status === 'stopped' || status === 'failed' || status === 'partly_undone';

/** Whether Undo can start: the state allows it and a delete's trash was not emptied. */
export const undoable = (run: BulkRun): boolean => mayUndo(run.status) && run.purged === null;

/** `BulkStatus::may_run`: a stopped or failed run goes on with a new click. */
export const mayRun = (status: BulkStatus): boolean =>
  status === 'previewed' || status === 'stopped' || status === 'failed';

/** A field set to a value, or emptied. */
export type Assign<T> = { readonly kind: 'set'; readonly id: T } | { readonly kind: 'clear' };

/** Memberships to add and to remove; the server refuses one id in both. */
export interface Membership<T> {
  readonly add: readonly T[];
  readonly remove: readonly T[];
}

/** What an edit sets on every item. `null` leaves the field alone. */
export interface BulkEdit {
  readonly isPublic: boolean | null;
  readonly owner: UserId | null;
  readonly class: Assign<ClassId> | null;
  readonly template: Assign<TemplateId> | null;
  readonly itemSets: Membership<ItemSetId>;
  readonly sites: Membership<SiteId>;
  /** Value edits (`bulk-values.ts`); empty leaves every value alone. */
  readonly values: readonly ValueOp[];
}

export const NO_EDIT: BulkEdit = {
  isPublic: null,
  owner: null,
  class: null,
  template: null,
  itemSets: { add: [], remove: [] },
  sites: { add: [], remove: [] },
  values: [],
};

/** Whether the edit changes anything: the server refuses one that names nothing. */
export const namesSomething = (edit: BulkEdit): boolean =>
  edit.isPublic !== null ||
  edit.owner !== null ||
  edit.class !== null ||
  edit.template !== null ||
  edit.values.length > 0 ||
  [edit.itemSets.add, edit.itemSets.remove, edit.sites.add, edit.sites.remove].some((ids) => ids.length > 0);

/** An item's describing fields, before or after the edit. */
export interface BulkFields {
  readonly isPublic: boolean;
  readonly owner: UserId | null;
  readonly class: ClassId | null;
  readonly template: TemplateId | null;
  readonly itemSets: readonly ItemSetId[];
  readonly sites: readonly SiteId[];
}

export interface BulkSample {
  readonly id: ItemId;
  readonly title: string | null;
  readonly before: BulkFields;
  readonly after: BulkFields;
  /** Each field whose values change, before and after. */
  readonly values: readonly FieldChange[];
}

export interface BulkPreview {
  readonly id: BulkId;
  /** The items the run visits: the selection, frozen. */
  readonly total: number;
  /** Items that already hold the change. */
  readonly unchanged: number;
  /** Items the caller may not change. */
  readonly denied: number;
  /** Up to five items that change. */
  readonly samples: readonly BulkSample[];
}

export interface BulkCounts {
  readonly pending: number;
  readonly done: number;
  readonly unchanged: number;
  readonly failed: number;
  readonly restored: number;
  readonly kept: number;
}

export const FAIL_REASONS = [
  'gone',
  'denied',
  'templateRequires',
  'removesRequired',
  'undoDenied',
  'deleteDenied',
  'idTaken',
  'linksLeft',
  'itemGone',
] as const;
export type FailReason = (typeof FAIL_REASONS)[number];

export interface BulkFailure {
  readonly item: ItemId;
  readonly reason: FailReason | null;
  /** The reason in English, a registry pattern the server-message catalogue translates. */
  readonly message: string | null;
}

/** What a delete moves to the trash: items with their media, media alone, or item sets alone. */
export const TRASH_OF = ['items', 'media', 'item_sets'] as const;
export type TrashOf = (typeof TRASH_OF)[number];

/** What a run does: an edit names its change; a delete moves its targets to the trash. */
export type BulkWork =
  | { readonly kind: 'edit'; readonly change: BulkEdit }
  | { readonly kind: 'delete'; readonly of: TrashOf };

export type BulkRun = RunFacts & BulkWork;

interface RunFacts {
  readonly id: BulkId;
  readonly status: BulkStatus;
  readonly owner: UserId | null;
  readonly total: number;
  readonly done: number;
  readonly failed: number;
  /** Read live from the targets; the recent list leaves them out. */
  readonly counts: BulkCounts | null;
  readonly failures: readonly BulkFailure[] | null;
  readonly error: string | null;
  readonly created: string;
  readonly ended: string | null;
  /** When a delete's trash was emptied: from then on nothing can be put back. */
  readonly purged: string | null;
}

export interface BulkUndoPreview {
  readonly restore: number;
  /** Items edited since the run wrote them: kept unless forced. */
  readonly edited: number;
  readonly gone: number;
  readonly editedItems: readonly ItemId[];
  readonly goneItems: readonly ItemId[];
}

const asId =
  <Name extends string>() =>
  (value: unknown, path: Path): Id<Name> =>
    brandId<Name>(asIdNumber(value, path));

const ids = <Name extends string>(): Parser<readonly Id<Name>[]> => list(asId<Name>());

const parseMembership = <Name extends string>(raw: unknown, path: Path): Membership<Id<Name>> => {
  const object = strict(raw, path, ['add', 'remove']);
  return { add: member(object, 'add', path, ids<Name>()), remove: member(object, 'remove', path, ids<Name>()) };
};

const assignOf =
  <Name extends string>() =>
  (object: Record<string, unknown>, key: string, path: Path): Assign<Id<Name>> | null => {
    if (!(key in object)) {
      return null;
    }
    const value = object[key];
    return value === null ? { kind: 'clear' } : { kind: 'set', id: asId<Name>()(value, [...path, key]) };
  };

const EMPTY = { add: [], remove: [] };

/** The stored change, as `BulkChange::to_json` writes it: an absent member leaves the field alone. */
export const parseEdit: Parser<BulkEdit> = (raw, path = []) => {
  const object = strict(raw, path, ['kind'], [
    'isPublic',
    'owner',
    'class',
    'template',
    'itemSets',
    'sites',
    'values',
  ]);
  member(object, 'kind', path, oneOf(['edit']));
  const optional = <T>(key: string, parse: (value: unknown, at: Path) => T): T | null =>
    key in object ? member(object, key, path, parse) : null;
  return {
    isPublic: optional('isPublic', asFlag),
    owner: optional('owner', asId<'UserId'>()),
    class: assignOf<'ClassId'>()(object, 'class', path),
    template: assignOf<'TemplateId'>()(object, 'template', path),
    itemSets: optional('itemSets', parseMembership<'ItemSetId'>) ?? EMPTY,
    sites: optional('sites', parseMembership<'SiteId'>) ?? EMPTY,
    values: optional('values', parseValueOps) ?? [],
  };
};

/** The change as the preview request sends it, which is also the stored form. */
export const encodeEdit = (edit: BulkEdit): Readonly<Record<string, unknown>> => {
  const assign = <T>(value: Assign<T>): T | null => (value.kind === 'set' ? value.id : null);
  const members = <T>(value: Membership<T>) => (value.add.length + value.remove.length === 0 ? {} : value);
  return {
    kind: 'edit',
    ...(edit.isPublic === null ? {} : { isPublic: edit.isPublic }),
    ...(edit.owner === null ? {} : { owner: edit.owner }),
    ...(edit.class === null ? {} : { class: assign(edit.class) }),
    ...(edit.template === null ? {} : { template: assign(edit.template) }),
    ...(edit.itemSets.add.length + edit.itemSets.remove.length === 0 ? {} : { itemSets: members(edit.itemSets) }),
    ...(edit.sites.add.length + edit.sites.remove.length === 0 ? {} : { sites: members(edit.sites) }),
    ...(edit.values.length === 0 ? {} : { values: edit.values }),
  };
};

const parseFields: Parser<BulkFields> = (raw, path = []) => {
  const object = strict(raw, path, ['isPublic', 'owner', 'class', 'template', 'itemSets', 'sites']);
  return {
    isPublic: member(object, 'isPublic', path, asFlag),
    owner: member(object, 'owner', path, nullable(asId<'UserId'>())),
    class: member(object, 'class', path, nullable(asId<'ClassId'>())),
    template: member(object, 'template', path, nullable(asId<'TemplateId'>())),
    itemSets: member(object, 'itemSets', path, ids<'ItemSetId'>()),
    sites: member(object, 'sites', path, ids<'SiteId'>()),
  };
};

const parseSample: Parser<BulkSample> = (raw, path = []) => {
  const object = strict(raw, path, ['id', 'title', 'before', 'after', 'values']);
  return {
    id: member(object, 'id', path, asId<'ItemId'>()),
    title: member(object, 'title', path, nullable(asString)),
    before: member(object, 'before', path, parseFields),
    after: member(object, 'after', path, parseFields),
    values: member(object, 'values', path, list(parseFieldChange)),
  };
};

export const parseBulkPreview: Parser<BulkPreview> = (raw, path = []) => {
  const object = strict(raw, path, ['id', 'status', 'total', 'unchanged', 'denied', 'samples']);
  member(object, 'status', path, oneOf(['previewed']));
  return {
    id: member(object, 'id', path, asId<'BulkId'>()),
    total: member(object, 'total', path, asCount),
    unchanged: member(object, 'unchanged', path, asCount),
    denied: member(object, 'denied', path, asCount),
    samples: member(object, 'samples', path, list(parseSample)),
  };
};

const COUNT_KEYS = ['pending', 'done', 'unchanged', 'failed', 'restored', 'kept'] as const;

const parseCounts: Parser<BulkCounts> = (raw, path = []) => {
  const object = strict(raw, path, COUNT_KEYS);
  const count = (key: (typeof COUNT_KEYS)[number]): number => member(object, key, path, asCount);
  return {
    pending: count('pending'),
    done: count('done'),
    unchanged: count('unchanged'),
    failed: count('failed'),
    restored: count('restored'),
    kept: count('kept'),
  };
};

const parseFailure: Parser<BulkFailure> = (raw, path = []) => {
  const object = strict(raw, path, ['item', 'reason', 'message']);
  return {
    item: member(object, 'item', path, asId<'ItemId'>()),
    reason: member(object, 'reason', path, nullable(oneOf(FAIL_REASONS))),
    message: member(object, 'message', path, nullable(asString)),
  };
};

const RUN_REQUIRED = [
  'id',
  'kind',
  'status',
  'owner',
  'query',
  'change',
  'total',
  'done',
  'failed',
  'jobId',
  'error',
  'created',
  'started',
  'ended',
  'purged',
];

/** A delete's stored change: its kind, and `of` unless it deletes items. */
const parseDelete = (raw: unknown, path: Path): TrashOf => {
  const object = strict(raw, path, ['kind'], ['of']);
  member(object, 'kind', path, oneOf(['delete']));
  return object.of === undefined ? 'items' : member(object, 'of', path, oneOf(TRASH_OF));
};

const parseWork = (object: Record<string, unknown>, path: Path): BulkWork => {
  if (member(object, 'kind', path, oneOf(['edit', 'delete'])) === 'delete') {
    return { kind: 'delete', of: member(object, 'change', path, parseDelete) };
  }
  return { kind: 'edit', change: member(object, 'change', path, parseEdit) };
};

export const parseBulkRun: Parser<BulkRun> = (raw, path = []) => {
  const object = strict(raw, path, RUN_REQUIRED, ['counts', 'failures']);
  member(object, 'query', path, asJsonValue);
  member(object, 'jobId', path, nullable(asCount));
  member(object, 'started', path, nullable(asString));
  return {
    ...parseWork(object, path),
    id: member(object, 'id', path, asId<'BulkId'>()),
    status: member(object, 'status', path, oneOf(BULK_STATUSES)),
    owner: member(object, 'owner', path, nullable(asId<'UserId'>())),
    total: member(object, 'total', path, asCount),
    done: member(object, 'done', path, asCount),
    failed: member(object, 'failed', path, asCount),
    counts: 'counts' in object ? member(object, 'counts', path, parseCounts) : null,
    failures: 'failures' in object ? member(object, 'failures', path, list(parseFailure)) : null,
    error: member(object, 'error', path, nullable(asString)),
    created: member(object, 'created', path, asString),
    ended: member(object, 'ended', path, nullable(asString)),
    purged: member(object, 'purged', path, nullable(asString)),
  };
};

export const parseBulkRuns: Parser<readonly BulkRun[]> = list(parseBulkRun);

export const parseBulkUndoPreview: Parser<BulkUndoPreview> = (raw, path = []) => {
  const object = strict(raw, path, ['restore', 'edited', 'gone', 'editedItems', 'goneItems']);
  return {
    restore: member(object, 'restore', path, asCount),
    edited: member(object, 'edited', path, asCount),
    gone: member(object, 'gone', path, asCount),
    editedItems: member(object, 'editedItems', path, ids<'ItemId'>()),
    goneItems: member(object, 'goneItems', path, ids<'ItemId'>()),
  };
};

export const parseStarted = (raw: unknown): void => {
  member(strict(raw, [], ['jobId']), 'jobId', [], asCount);
};

/** The members of a list query that choose no items (`WINDOW` in `bulk/scope.rs`). */
const WINDOW = new Set(['page', 'per_page', 'sort_by', 'sort_order']);

/** Which items a change reaches, as the preview request names them. */
export type BulkQuery = Readonly<Record<string, string | number | readonly (string | number)[]>>;

/** The items the browse query matches, every page: its filters, snake-cased as the list request sends them. */
export const matchingQuery = (query: ResourceQuery): BulkQuery =>
  Object.fromEntries(
    Object.entries(query).flatMap(([key, value]) => {
      const name = key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
      return value === undefined || WINDOW.has(name) ? [] : [[name, value]];
    }),
  );

/** The ticked items alone: a query of `id` and nothing else. */
export const tickedQuery = (items: readonly ItemId[]): BulkQuery => ({ id: items });

/** The cases the panel words apart from a plain error. */
export type BulkRefusal =
  | { readonly kind: 'expired' }
  | { readonly kind: 'denied' }
  | { readonly kind: 'error'; readonly error: Error };

/** What a refused request means to the panel. */
export const bulkRefusal = (error: Error): BulkRefusal => {
  if (error instanceof ApiError && error.status === 403) {
    return { kind: 'denied' };
  }
  if (error instanceof ApiError && error.status === 409 && isRecord(error.body) && error.body['code'] === 'expired') {
    return { kind: 'expired' };
  }
  return { kind: 'error', error };
};

export const BULK = '/api/romeka/bulk';
const one = (id: BulkId): string => `${BULK}/${String(id)}`;

export const bulkApi = {
  async preview(query: BulkQuery, edit: BulkEdit): Promise<BulkPreview> {
    return parseBulkPreview(await api.writeJson('POST', BULK, { query, change: encodeEdit(edit) }));
  },
  async run(id: BulkId): Promise<void> {
    parseStarted(await api.writeJson('POST', `${one(id)}/run`, {}));
  },
  async stop(id: BulkId): Promise<void> {
    await api.writeJson('POST', `${one(id)}/stop`, {});
  },
  async get(id: BulkId): Promise<BulkRun> {
    return parseBulkRun(await api.requestJson(one(id)));
  },
  async recent(): Promise<readonly BulkRun[]> {
    return parseBulkRuns(await api.requestJson(BULK));
  },
  async undoPreview(id: BulkId): Promise<BulkUndoPreview> {
    return parseBulkUndoPreview(await api.writeJson('POST', `${one(id)}/undo/preview`, {}));
  },
  async undo(id: BulkId, force: boolean): Promise<void> {
    parseStarted(await api.writeJson('POST', `${one(id)}/undo`, { force }));
  },
};
