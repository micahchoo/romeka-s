/**
 * The plan store (`src/pages/import/plan-store.ts`): an edit is saved a moment
 * later with the revision the store last read, edits made while a save is out
 * go in the next save, and each refusal the save route answers ends in its own
 * state (`docs/import-v2.md`, "Plan saves carry the plan's revision").
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ImportPlan } from '../src/api/import-plan';
import { ImportApi } from '../src/api/import-routes';
import { parseImportDetail } from '../src/api/import-view';
import type { ImportDetail } from '../src/api/import-view';
import { asRecord } from '../src/api/parse';
import { PlanStore, SAVE_DELAY_MS } from '../src/pages/import/plan-store';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';
import type { Handler } from './stub';

const api = new ImportApi({ baseUrl: '' });
const PATH = '/api/romeka/imports/1';

const draft = (): ImportDetail => parseImportDetail(loadFixture('import/import-draft.json'));
/** The draft as the server answers a save: the plan sent, at the next revision. */
const savedAs = (init: RequestInit | undefined, revision: number): Response => {
  const sent = typeof init?.body === 'string' ? asRecord(JSON.parse(init.body), []) : {};
  return jsonResponse({ ...asRecord(loadFixture('import/import-draft.json'), []), plan: sent['plan'], revision });
};
const conflict = (name: string): Response => jsonResponse(loadFixture(`import/${name}`), { status: 409 });

const caseFold = (on: boolean) => (plan: ImportPlan): ImportPlan => ({ ...plan, grouping: { ...plan.grouping, caseFold: on } });
const sentRevisions = (fetchMock: ReturnType<typeof installFetch>): readonly unknown[] =>
  requestsFor(fetchMock, PATH)
    .filter((request) => request.init?.method === 'PUT')
    .map((request) => requestBody(request)['revision']);

/** Answers each PUT with the next revision, counting from the draft's 0. */
const counting = (): Handler => {
  let revision = 0;
  return route({
    [PATH]: (_url, init) => {
      revision += 1;
      return savedAs(init, revision);
    },
  });
};

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('saving', () => {
  it('saves an edit after the delay with the revision it read, then the next with the answer’s', async () => {
    const fetchMock = installFetch(counting());
    const store = new PlanStore(api, draft());
    store.change(caseFold(true));
    expect(store.snapshot().save.status).toBe('saving');
    expect(sentRevisions(fetchMock)).toEqual([]);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(sentRevisions(fetchMock)).toEqual([0]);
    expect(store.snapshot().save.status).toBe('saved');
    expect(store.snapshot().detail.revision).toBe(1);
    store.change(caseFold(false));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(sentRevisions(fetchMock)).toEqual([0, 1]);
    expect(store.snapshot().plan.grouping.caseFold).toBe(false);
  });

  it('sends the edits made while a save is out in one later save', async () => {
    const gate: { release: (() => void) | null } = { release: null };
    let revision = 0;
    const fetchMock = installFetch(
      route({
        [PATH]: (_url, init) => {
          revision += 1;
          return savedAs(init, revision);
        },
      }),
    );
    const held = fetchMock.getMockImplementation();
    fetchMock.mockImplementationOnce(
      (input, init) =>
        new Promise((resolve) => {
          gate.release = () => {
            resolve(held === undefined ? new Response(null) : held(input, init));
          };
        }),
    );
    const store = new PlanStore(api, draft());
    store.change(caseFold(true));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    store.change(caseFold(false));
    store.change(caseFold(true));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS * 3);
    expect(sentRevisions(fetchMock)).toEqual([0]);
    expect(gate.release).not.toBeNull();
    gate.release?.();
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(sentRevisions(fetchMock)).toEqual([0, 1]);
    expect(store.snapshot().save.status).toBe('saved');
    expect(store.snapshot().detail.revision).toBe(2);
  });

  it('saves a waiting edit at once when the store closes', async () => {
    const fetchMock = installFetch(counting());
    const store = new PlanStore(api, draft());
    store.change(caseFold(true));
    store.close();
    await vi.advanceTimersByTimeAsync(0);
    expect(sentRevisions(fetchMock)).toEqual([0]);
  });
});

describe('refusals', () => {
  it('holds the edits on 409 stale, refuses more, and a reload reads the stored plan', async () => {
    let stored = false;
    const fetchMock = installFetch(
      route({
        [PATH]: (_url, init) => {
          if (init?.method === 'PUT') {
            return conflict('conflict-stale.json');
          }
          stored = true;
          return jsonResponse({ ...asRecord(loadFixture('import/import-draft.json'), []), revision: 1 });
        },
      }),
    );
    const store = new PlanStore(api, draft());
    store.change(caseFold(true));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(store.snapshot().save).toEqual({ status: 'stale', revision: 1 });
    expect(store.snapshot().editable).toBe(false);
    store.change(caseFold(false));
    expect(store.snapshot().plan.grouping.caseFold).toBe(true);
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(sentRevisions(fetchMock)).toEqual([0]);

    await store.reload();
    expect(stored).toBe(true);
    expect(store.snapshot().save.status).toBe('saved');
    expect(store.snapshot().detail.revision).toBe(1);
    expect(store.snapshot().plan.grouping.caseFold).toBe(false);
    expect(store.snapshot().editable).toBe(true);
  });

  it('becomes read-only on 409 notDraft', async () => {
    installFetch(route({ [PATH]: () => conflict('conflict-not-draft.json') }));
    const store = new PlanStore(api, draft());
    store.change(caseFold(true));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(store.snapshot().save).toEqual({ status: 'closed', importStatus: 'running' });
    expect(store.snapshot().editable).toBe(false);
  });

  it('keeps the edit after another failure, and a retry sends it again', async () => {
    let fail = true;
    const fetchMock = installFetch(
      route({
        [PATH]: (_url, init) =>
          fail ? jsonResponse({ errors: { error: 'The database is busy.' } }, { status: 503 }) : savedAs(init, 1),
      }),
    );
    const store = new PlanStore(api, draft());
    store.change(caseFold(true));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    const failed = store.snapshot().save;
    expect(failed.status).toBe('failed');
    expect(failed.status === 'failed' ? failed.error.message : '').toBe('The database is busy.');
    expect(store.snapshot().editable).toBe(true);
    fail = false;
    store.retry();
    await vi.advanceTimersByTimeAsync(0);
    expect(sentRevisions(fetchMock)).toEqual([0, 0]);
    expect(store.snapshot().save.status).toBe('saved');
  });

  it('never saves the plan of an import that has started', async () => {
    const fetchMock = installFetch(route({}));
    const store = new PlanStore(api, parseImportDetail(loadFixture('import/import-done.json')));
    expect(store.snapshot().save).toEqual({ status: 'closed', importStatus: 'done' });
    store.change(caseFold(true));
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS);
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
