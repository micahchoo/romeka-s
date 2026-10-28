/**
 * The bulk delete's own requests (`crates/omeka-api/src/bulk/trash.rs`): its preview, and an
 * administrator's "Empty the trash". A delete then runs, stops and undoes through `bulk.ts` like
 * any run. The item page's, the media page's and the set place's delete is a bulk delete of that
 * one record, and watches its run with `settled`. This module is imported by the delete chunks and the recent list only, so the
 * panel's first chunk does not carry it.
 */
import { api } from './client';
import { BULK, bulkApi, isLive } from './bulk';
import type { BulkId, BulkQuery, BulkRun, TrashOf } from './bulk';
import { asCount, list, member, nullable, oneOf, strict } from './import-read';
import { asIdNumber, asString, brandId } from './parse';
import type { ItemId, Parser } from './types';

export interface BulkDeletePreview {
  readonly id: BulkId;
  /** The records the run visits: the selection, frozen. */
  readonly total: number;
  /** Records the caller may not delete; the run leaves them. */
  readonly denied: number;
  /** The media that go: those of the items, or the media themselves. */
  readonly media: number;
  /** Their originals' bytes. */
  readonly bytes: number;
  /** How many days the trash keeps them. */
  readonly trashDays: number;
  /** Up to five records that go. */
  readonly samples: readonly { readonly id: ItemId; readonly title: string | null }[];
}

/** What emptying the trash removed: the runs, and the records of each kind. */
export interface Emptied {
  readonly runs: number;
  readonly items: number;
  /** Media trashed alone; an item's media go with the item. */
  readonly media: number;
  readonly itemSets: number;
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
  const object = strict(raw, path, ['runs', 'items', 'media', 'itemSets']);
  return {
    runs: member(object, 'runs', path, asCount),
    items: member(object, 'items', path, asCount),
    media: member(object, 'media', path, asCount),
    itemSets: member(object, 'itemSets', path, asCount),
  };
};

/**
 * The waits before each read of a run someone waits on. A one-item run ends in tens of
 * milliseconds on a debug build, so the first reads come soon; a slow one is read each second.
 */
const SETTLE_WAITS_MS = [50, 100, 200, 400, 800] as const;
const SETTLE_WAIT_LAST_MS = 1000;

const wait = (ms: number): Promise<void> =>
  new Promise((done) => {
    setTimeout(done, ms);
  });

export const bulkDeleteApi = {
  /** Read the run until no job works on it, and answer it. */
  async settled(id: BulkId): Promise<BulkRun> {
    for (let attempt = 0; ; attempt += 1) {
      await wait(SETTLE_WAITS_MS[attempt] ?? SETTLE_WAIT_LAST_MS);
      const run = await bulkApi.get(id);
      if (!isLive(run.status)) {
        return run;
      }
    }
  },
  /** Preview a delete of the items `query` selects, or of the media or sets it names by id. */
  async preview(query: BulkQuery, of: TrashOf = 'items'): Promise<BulkDeletePreview> {
    const change = of === 'items' ? { kind: 'delete' } : { kind: 'delete', of };
    return parseDeletePreview(await api.writeJson('POST', BULK, { query, change }));
  },
  async emptyTrash(): Promise<Emptied> {
    return parseEmptied(await api.writeJson('POST', `${BULK}/trash/purge`, {}));
  },
};
