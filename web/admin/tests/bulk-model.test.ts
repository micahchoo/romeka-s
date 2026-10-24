/**
 * The bulk change panel's steps (`src/pages/items/bulk/model.ts`): each action moves the panel
 * only from the steps it belongs to, so a late answer never lands on a step the person left.
 */
import { describe, expect, it } from 'vitest';
import { NO_EDIT, parseBulkPreview, parseBulkRun, parseBulkUndoPreview } from '../src/api/bulk';
import type { BulkRefusal } from '../src/api/bulk';
import { CLOSED, isBusy, isPolling, reduce } from '../src/pages/items/bulk/model';
import type { Action, Panel } from '../src/pages/items/bulk/model';
import { loadFixture } from './loadFixture';

const asObject = (value: unknown): Record<string, unknown> => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('expected an object fixture');
  }
  return Object.fromEntries(Object.entries(value));
};

const preview = parseBulkPreview(loadFixture('bulk/preview.json'));
const done = parseBulkRun(loadFixture('bulk/run-done.json'));
const running = { ...done, status: 'running' as const };
const check = parseBulkUndoPreview(loadFixture('bulk/undo-preview.json'));
const PRIVATE = { ...NO_EDIT, isPublic: false };

const walk = (actions: readonly Action[], from: Panel = CLOSED): Panel => actions.reduce(reduce, from);

const previewed = walk([
  { type: 'openEdit', scope: 'matching' },
  { type: 'draft', draft: { scope: 'matching', edit: PRIVATE } },
  { type: 'previewSent' },
  { type: 'previewed', preview },
]);

describe('bulk panel steps', () => {
  it('goes from the edit through the preview to watching the run', () => {
    expect(previewed).toEqual({ step: 'preview', draft: { scope: 'matching', edit: PRIVATE }, preview, failure: null });
    const starting = reduce(previewed, { type: 'runSent' });
    expect(starting.step).toBe('starting');
    expect(isBusy(starting)).toBe(true);
    const watching = reduce(starting, { type: 'run', run: running });
    expect(watching).toEqual({ step: 'watch', run: running, failure: null });
    expect(isPolling(watching)).toBe(true);
    const ended = reduce(watching, { type: 'run', run: done });
    expect(isPolling(ended)).toBe(false);
  });

  it('keeps the preview on an expired Apply, so Preview again sends the same draft', () => {
    const expired: BulkRefusal = { kind: 'expired' };
    const refused = walk([{ type: 'runSent' }, { type: 'refused', refusal: expired }], previewed);
    expect(refused).toEqual({ ...previewed, failure: expired });
    const again = reduce(refused, { type: 'previewSent' });
    expect(again).toEqual({ step: 'previewing', draft: { scope: 'matching', edit: PRIVATE } });
  });

  it('returns a refused preview to the edit with the draft and the reason', () => {
    const denied: BulkRefusal = { kind: 'denied' };
    const panel = walk(
      [{ type: 'openEdit', scope: 'ticked' }, { type: 'previewSent' }, { type: 'refused', refusal: denied }],
    );
    expect(panel).toEqual({ step: 'edit', draft: { scope: 'ticked', edit: NO_EDIT }, failure: denied });
  });

  it('ignores an answer for a step the panel has left', () => {
    const closed = reduce(previewed, { type: 'close' });
    expect(reduce(closed, { type: 'run', run: running })).toBe(closed);
    expect(reduce(closed, { type: 'previewed', preview })).toBe(closed);
    const other = parseBulkRun({ ...asObject(loadFixture('bulk/run-done.json')), id: 9 });
    const watching: Panel = { step: 'watch', run: done, failure: null };
    expect(reduce(watching, { type: 'run', run: other })).toBe(watching);
  });

  it('asks before an undo and sends the force the person chose', () => {
    const watching: Panel = { step: 'watch', run: done, failure: null };
    const asking = walk([{ type: 'undoCheck', run: done }, { type: 'undoChecked', check }], watching);
    expect(asking).toEqual({ step: 'undoAsk', run: done, check, force: false, failure: null });
    const forced = walk([{ type: 'force', force: true }, { type: 'undoSent' }], asking);
    expect(forced).toEqual({ step: 'undoStarting', run: done, check, force: true });
    expect(reduce(forced, { type: 'run', run: running }).step).toBe('watch');
    expect(reduce(asking, { type: 'back' })).toEqual(watching);
  });

  it('opens the recent list, then a run from it', () => {
    const recent = reduce(CLOSED, { type: 'openRecent' });
    expect(isBusy(recent)).toBe(true);
    const listed = reduce(recent, { type: 'recentLoaded', runs: [done] });
    expect(listed).toEqual({ step: 'recent', runs: [done], failure: null });
    expect(reduce(listed, { type: 'show', run: running })).toEqual({ step: 'watch', run: running, failure: null });
    expect(reduce(listed, { type: 'undoCheck', run: done })).toEqual({ step: 'undoCheck', run: done });
  });
});
