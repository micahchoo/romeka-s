/**
 * The bulk delete wire (`src/api/bulk-delete.ts`, and the delete runs `src/api/bulk.ts` reads)
 * against answers the routes gave (`tests/fixtures/bulk/delete-*.json`, written by
 * `crates/omeka-cli/tests/bulk_wire.rs`).
 */
import { describe, expect, it } from 'vitest';
import { parseBulkRun, parseBulkUndoPreview, undoable } from '../src/api/bulk';
import { parseDeletePreview, parseEmptied } from '../src/api/bulk-delete';
import { ParseError } from '../src/api/parse';
import { loadFixture } from './loadFixture';

describe('bulk delete wire', () => {
  it('reads a delete preview: the items, their media and bytes, the trash days and the first items', () => {
    expect(parseDeletePreview(loadFixture('bulk/delete-preview.json'))).toEqual({
      id: 1,
      total: 2,
      denied: 0,
      media: 0,
      bytes: 0,
      trashDays: 30,
      samples: [
        { id: 1, title: 'Letter to Anna' },
        { id: 2, title: 'Letter to Bob' },
      ],
    });
  });

  it('reads a delete run, which names no edit, and its undo', () => {
    const run = parseBulkRun(loadFixture('bulk/delete-done.json'));
    expect(run.kind).toBe('delete');
    expect(run.counts?.done).toBe(2);
    expect(undoable(run)).toBe(true);
    expect(parseBulkUndoPreview(loadFixture('bulk/delete-undo-preview.json')).restore).toBe(2);
    expect(parseBulkRun(loadFixture('bulk/delete-undone.json')).counts?.restored).toBe(2);
  });

  it('reads what a delete of media or of item sets moves to the trash; an item delete names no kind', () => {
    const media = parseBulkRun(loadFixture('bulk/delete-media-done.json'));
    expect(media.kind === 'delete' ? media.of : null).toBe('media');
    expect(undoable(media)).toBe(true);
    const sets = parseBulkRun(loadFixture('bulk/delete-set-done.json'));
    expect(sets.kind === 'delete' ? sets.of : null).toBe('item_sets');
    const items = parseBulkRun(loadFixture('bulk/delete-done.json'));
    expect(items.kind === 'delete' ? items.of : null).toBe('items');
  });

  it('reads a run whose trash was emptied as one that cannot be undone', () => {
    const run = parseBulkRun(loadFixture('bulk/delete-purged.json'));
    expect(run.status).toBe('done');
    expect(run.purged).toBe('2026-10-24T09:00:00+00:00');
    expect(undoable(run)).toBe(false);
    expect(parseEmptied(loadFixture('bulk/trash-purged.json'))).toEqual({ runs: 1, items: 1, media: 0, itemSets: 0 });
  });

  it('refuses a delete preview that is not one', () => {
    expect(() => parseDeletePreview(loadFixture('bulk/preview.json'))).toThrow(ParseError);
  });
});
