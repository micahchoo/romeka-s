/**
 * The bulk delete's own requests (`crates/omeka-api/src/bulk/trash.rs`): its preview, and an
 * administrator's "Empty the trash". A delete then runs, stops and undoes through `bulk.ts` like
 * any run. This module is imported by the delete step and the recent list only, so the panel's
 * first chunk does not carry it.
 */
import { api } from './client';
import { BULK } from './bulk';
import type { BulkId, BulkQuery } from './bulk';
import { asCount, list, member, nullable, oneOf, strict } from './import-read';
import { asIdNumber, asString, brandId } from './parse';
import type { ItemId, Parser } from './types';

export interface BulkDeletePreview {
  readonly id: BulkId;
  /** The items the run visits: the selection, frozen. */
  readonly total: number;
  /** Items the caller may not delete; the run leaves them. */
  readonly denied: number;
  /** The media of the items that go. */
  readonly media: number;
  /** Their originals' bytes. */
  readonly bytes: number;
  /** How many days the trash keeps them. */
  readonly trashDays: number;
  /** Up to five items that go. */
  readonly samples: readonly { readonly id: ItemId; readonly title: string | null }[];
}

/** What emptying the trash removed. */
export interface Emptied {
  readonly runs: number;
  readonly items: number;
}

const parseSample = (raw: unknown, path: readonly (string | number)[]) => {
  const object = strict(raw, path, ['id', 'title']);
  return {
    id: brandId<'ItemId'>(member(object, 'id', path, asIdNumber)),
    title: member(object, 'title', path, nullable(asString)),
  };
};

export const parseDeletePreview: Parser<BulkDeletePreview> = (raw, path = []) => {
  const object = strict(raw, path, ['id', 'status', 'kind', 'total', 'denied', 'media', 'bytes', 'trashDays', 'samples']);
  member(object, 'status', path, oneOf(['previewed']));
  member(object, 'kind', path, oneOf(['delete']));
  return {
    id: brandId<'BulkId'>(member(object, 'id', path, asIdNumber)),
    total: member(object, 'total', path, asCount),
    denied: member(object, 'denied', path, asCount),
    media: member(object, 'media', path, asCount),
    bytes: member(object, 'bytes', path, asCount),
    trashDays: member(object, 'trashDays', path, asCount),
    samples: member(object, 'samples', path, list(parseSample)),
  };
};

export const parseEmptied: Parser<Emptied> = (raw, path = []) => {
  const object = strict(raw, path, ['runs', 'items']);
  return { runs: member(object, 'runs', path, asCount), items: member(object, 'items', path, asCount) };
};

export const bulkDeleteApi = {
  async preview(query: BulkQuery): Promise<BulkDeletePreview> {
    return parseDeletePreview(await api.writeJson('POST', BULK, { query, change: { kind: 'delete' } }));
  },
  async emptyTrash(): Promise<Emptied> {
    return parseEmptied(await api.writeJson('POST', `${BULK}/trash/purge`, {}));
  },
};
