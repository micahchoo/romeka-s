/**
 * The bulk change wire (`src/api/bulk.ts`) against answers the routes gave
 * (`tests/fixtures/bulk/`, written by `crates/omeka-cli/tests/bulk_wire.rs`).
 */
import { describe, expect, it } from 'vitest';
import {
  NO_EDIT,
  bulkRefusal,
  encodeEdit,
  matchingQuery,
  parseBulkPreview,
  parseBulkRun,
  parseBulkRuns,
  parseBulkUndoPreview,
  parseEdit,
  tickedQuery,
} from '../src/api/bulk';
import { ApiError } from '../src/api/client';
import { ParseError, brandId } from '../src/api/parse';
import { browseQuery, parseBrowse } from '../src/pages/items/browse';
import { loadFixture } from './loadFixture';

const withMember = (name: string, key: string, value: unknown): unknown => {
  const raw = loadFixture(name);
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    throw new Error(`${name} is not an object`);
  }
  return { ...Object.fromEntries(Object.entries(raw)), [key]: value };
};

describe('bulk wire', () => {
  it('reads a preview: the frozen count and the items before and after', () => {
    const preview = parseBulkPreview(loadFixture('bulk/preview.json'));
    expect(preview.total).toBe(3);
    expect(preview.unchanged).toBe(0);
    expect(preview.samples[0]).toMatchObject({
      title: 'Letter to Anna',
      before: { isPublic: true, itemSets: [] },
      after: { isPublic: false, itemSets: [1] },
    });
  });

  it('reads a run with its counts, its change and the reason an item failed', () => {
    const run = parseBulkRun(loadFixture('bulk/run-done.json'));
    expect(run.status).toBe('done');
    expect(run.counts).toEqual({ pending: 0, done: 2, unchanged: 0, failed: 1, restored: 0, kept: 0 });
    expect(run).toMatchObject({ kind: 'edit', change: { ...NO_EDIT, isPublic: false, itemSets: { add: [1], remove: [] } } });
    expect(run.failures).toEqual([{ item: 4, reason: 'gone', message: 'This item is no longer in Omeka.' }]);
  });

  it('reads the recent list, which carries no counts', () => {
    const [run] = parseBulkRuns(loadFixture('bulk/runs.json'));
    expect(run?.counts).toBeNull();
    expect(run?.failures).toBeNull();
  });

  it('reads an undo preview and a partly undone run', () => {
    expect(parseBulkUndoPreview(loadFixture('bulk/undo-preview.json'))).toEqual({
      restore: 1,
      edited: 1,
      gone: 0,
      editedItems: [3],
      goneItems: [],
    });
    const undone = parseBulkRun(loadFixture('bulk/run-partly-undone.json'));
    expect(undone.status).toBe('partly_undone');
    expect(undone.counts?.kept).toBe(1);
  });

  it('refuses a member it does not know and a status it does not know', () => {
    expect(() => parseBulkRun(withMember('bulk/run-done.json', 'surprise', 1))).toThrow(ParseError);
    expect(() => parseBulkRun(withMember('bulk/run-done.json', 'status', 'paused'))).toThrow(ParseError);
  });

  it('writes a change the way the server stores it, and reads it back', () => {
    const edit = {
      ...NO_EDIT,
      owner: brandId<'UserId'>(3),
      class: { kind: 'clear' } as const,
      template: { kind: 'set', id: brandId<'TemplateId'>(7) } as const,
      sites: { add: [brandId<'SiteId'>(2)], remove: [] },
    };
    const wire = encodeEdit(edit);
    expect(wire).toEqual({ kind: 'edit', owner: 3, class: null, template: 7, sites: { add: [2], remove: [] } });
    expect(parseEdit(wire)).toEqual(edit);
  });

  it('sends the list filters without the page window, or the ticked ids alone', () => {
    const state = parseBrowse(new URLSearchParams('q=letter&visibility=public&media=none&sort=title&page=3'), 'table');
    expect(matchingQuery(browseQuery(state))).toEqual({ search: 'letter', is_public: 1, has_media: 0 });
    expect(tickedQuery([brandId<'ItemId'>(4), brandId<'ItemId'>(9)])).toEqual({ id: [4, 9] });
  });

  it('tells an expired preview and a refused right from other errors', () => {
    const conflict = (code: string): ApiError => new ApiError(409, null, 'refused', null, { code });
    expect(bulkRefusal(conflict('expired'))).toEqual({ kind: 'expired' });
    expect(bulkRefusal(new ApiError(403, null, 'no', null))).toEqual({ kind: 'denied' });
    const other = conflict('cannotRun');
    expect(bulkRefusal(other)).toEqual({ kind: 'error', error: other });
  });
});
