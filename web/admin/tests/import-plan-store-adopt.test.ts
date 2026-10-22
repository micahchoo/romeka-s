/**
 * `PlanStore.adopt`: the import another route answered (an added list) takes
 * the place of the one in hand only while no edit waits to be saved, so the
 * next save carries the new revision and an unsaved edit is never dropped.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ImportApi } from '../src/api/import-routes';
import { parseImportDetail } from '../src/api/import-detail-read';
import { asRecord } from '../src/api/parse';
import { PlanStore } from '../src/pages/import/plan-store';
import { allLists, seriesOf, withSeries } from '../src/pages/import/series';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';

const api = new ImportApi({ baseUrl: '' });
const draft = () => parseImportDetail(loadFixture('import/import-draft.json'));
const added = () => parseImportDetail({ ...asRecord(loadFixture('import/import-sheet-added.json'), []), id: 1, revision: 3 });

afterEach(() => {
  vi.useRealTimers();
});

describe('PlanStore.adopt', () => {
  it('takes the import a list upload answered when nothing waits to be saved', () => {
    installFetch(() => jsonResponse({}));
    const store = new PlanStore(api, draft());
    const next = added();
    store.adopt(next);
    expect(store.snapshot().detail.revision).toBe(3);
    expect(allLists(store.snapshot().plan).map((sheet) => sheet.id)).toEqual(['list-1']);
    expect(store.snapshot().save).toEqual({ status: 'saved' });
    store.close();
  });

  it('keeps an edit that waits to be saved', () => {
    vi.useFakeTimers();
    installFetch(() => jsonResponse({}));
    const store = new PlanStore(api, draft());
    store.change((plan) => withSeries(plan, 'all', (series) => ({ ...series, grouping: { ...series.grouping, caseFold: true } })));
    store.adopt(added());
    expect(store.snapshot().detail.revision).toBe(0);
    expect(seriesOf(store.snapshot().plan, 'all').grouping.caseFold).toBe(true);
    store.close();
  });
});
