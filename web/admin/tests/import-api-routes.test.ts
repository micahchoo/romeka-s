/**
 * The import client (`src/api/import-routes.ts`): each route's request, the
 * parse of its answer (the fixtures the server's routes wrote), and the 409
 * codes the run, save and undo routes answer with, and the 400 codes of a
 * list round trip in a plan of several series.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { ApiError } from '../src/api/client';
import { CONFLICT_CODES, conflictOf, seriesRefusalOf } from '../src/api/import-conflict';
import type { ImportConflict } from '../src/api/import-conflict';
import { parseImportPlan } from '../src/api/import-plan-read';
import { ImportApi } from '../src/api/import-routes';
import { ParseError, asRecord, brandId } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, noContent, requestBody, requestsFor, route } from './stub';

const FIXTURES = resolve(dirname(fileURLToPath(import.meta.url)), 'fixtures/import');
const api = new ImportApi({ baseUrl: '' });
const ID = brandId<'ImportId'>(1);

const answer = (name: string, status = 200): Response => jsonResponse(loadFixture(`import/${name}`), { status });
const savedPlan = (): unknown => asRecord(loadFixture('import/import-saved.json'), [])['plan'];
const csv = (name: string): string => readFileSync(resolve(FIXTURES, name), 'utf8');
const only = <T,>(values: readonly T[]): T => {
  const [value] = values;
  if (value === undefined || values.length !== 1) {
    throw new Error('expected exactly one request');
  }
  return value;
};

describe('reading', () => {
  it('lists a folder of the archive roots, and the same listing for a nested path', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/archive': (url) => answer(url.searchParams.get('path') === 'Artifacts' ? 'archive-folder.json' : 'archive.json') }));
    const top = await api.archive();
    expect(top.roots).toEqual(['archive']);
    expect(top.entries.map((entry) => entry.kind)).toContain('folder');
    const inside = await api.archive({ root: 'archive', path: 'Artifacts' });
    expect(inside.path).toBe('Artifacts');
    const urls = requestsFor(fetchMock, '/api/romeka/archive').map((request) => request.url.search);
    expect(urls).toEqual(['', '?root=archive&path=Artifacts']);
  });

  it('reads an install with no archive root as an empty listing', async () => {
    installFetch(route({ '/api/romeka/archive': () => jsonResponse({ roots: [], root: null, path: '', entries: [] }) }));
    expect(await api.archive()).toEqual({ roots: [], root: null, path: '', entries: [] });
  });

  it('reads the history, a draft, an import that runs, and one that stopped with failures', async () => {
    installFetch(
      route({
        '/api/romeka/imports': () => answer('history.json'),
        '/api/romeka/imports/1': (url) => answer(url.searchParams.has('failed') ? 'import-run-failed.json' : 'import-running.json'),
        '/api/romeka/imports/2': () => answer('import-draft.json'),
      }),
    );
    const history = await api.history({ page: 1, perPage: 10 });
    expect(history.items.map((row) => row.status)).toEqual(['undoing', 'draft', 'run_failed']);
    expect(history.items[0]).not.toHaveProperty('plan');
    const running = await api.get(ID);
    expect(running.status).toBe('running');
    expect(running.progress).toEqual({ done: 3, total: 9, secondsLeft: 42 });
    expect(running.ranPlan?.version).toBe(2);
    expect((await api.get(brandId<'ImportId'>(2))).summary).toBeNull();
  });

  it('reads the failures of a stopped run and the stop request', async () => {
    installFetch(route({ '/api/romeka/imports/1': () => answer('import-run-failed.json') }));
    const failed = await api.get(ID);
    expect(failed.summary?.stopped).toBe(true);
    expect(failed.summary?.failures[0]).toMatchObject({ record: 'group:BHC_006_AF_001', key: 'importFileUnreadable' });
    installFetch(route({ '/api/romeka/imports/1': () => answer('import-stopping.json') }));
    expect((await api.get(ID)).stopRequested).toBe(true);
  });

  it('reads the starters through the one starter table', async () => {
    installFetch(route({ '/api/romeka/starters': () => answer('../starters.json') }));
    const table = await api.starters();
    expect(table.other.id).toBe('other');
  });

  it('reads the left-out file, the downloaded list and a job log as text', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/imports/1/left.csv': () => new Response(csv('left.csv'), { headers: { 'Content-Type': 'text/csv' } }),
        '/api/romeka/imports/1/template.csv': () => new Response(csv('template.csv'), { headers: { 'Content-Type': 'text/csv' } }),
        '/api/romeka/imports/1/pairs.csv': () => new Response(csv('pairs.csv'), { headers: { 'Content-Type': 'text/csv' } }),
        '/api/romeka/jobs/7/log': () => new Response('record 1 written\n'),
      }),
    );
    expect(await api.leftCsv(ID)).toContain('#reason');
    const list = await api.templateCsv(ID, ['dcterms:title', 'dcterms:date']);
    expect(list.startsWith('\u{feff}Identifier,Files,Title [dcterms:title]')).toBe(true);
    expect(only(requestsFor(fetchMock, '/api/romeka/imports/1/template.csv')).url.search).toBe('?columns=dcterms%3Atitle%2Cdcterms%3Adate');
    expect(await api.jobLog(7)).toBe('record 1 written\n');
    expect(api.pairsUrl(ID)).toBe('/api/romeka/imports/1/pairs.csv');
    const down = csv('pairs.csv');
    expect(down.startsWith('\u{feff}Row [sheet:key],')).toBe(true);
    expect(down.split('\r\n')[0]?.endsWith(',Files')).toBe(true);
    expect(api.templateUrl(ID, ['dcterms:title'])).toBe('/api/romeka/imports/1/template.csv?columns=dcterms%3Atitle');
  });
});

describe('writing', () => {
  it('creates a draft from sources or from a copy', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports': () => answer('import-draft.json', 201) }));
    const draft = await api.create({ sources: [{ id: 's1', kind: 'folder', root: 'archive', path: '' }] });
    expect(draft.revision).toBe(0);
    await api.create({ copyFrom: ID });
    const bodies = requestsFor(fetchMock, '/api/romeka/imports').map(requestBody);
    expect(bodies).toEqual([{ sources: [{ id: 's1', kind: 'folder', root: 'archive', path: '' }] }, { copyFrom: 1 }]);
  });

  it('saves the plan with the revision it was read at, and takes the new revision', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports/1': () => answer('import-saved.json') }));
    const saved = await api.save(ID, parseImportPlan(savedPlan()), 2);
    expect(saved.plan.version).toBe(2);
    const sent = only(requestsFor(fetchMock, '/api/romeka/imports/1'));
    expect(sent.init?.method).toBe('PUT');
    expect(requestBody(sent)['revision']).toBe(2);
    expect(requestBody(sent)['plan']).toEqual(savedPlan());
  });

  it('deletes a draft with DELETE and takes the 204', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports/2': () => noContent() }));
    await expect(api.remove(brandId<'ImportId'>(2))).resolves.toBeUndefined();
    expect(only(requestsFor(fetchMock, '/api/romeka/imports/2')).init?.method).toBe('DELETE');
  });

  it('scans in the request or as a job', async () => {
    installFetch(route({ '/api/romeka/imports/1/scan': () => answer('scan.json') }));
    const scanned = await api.scan(ID);
    expect(scanned.kind).toBe('scanned');
    installFetch(route({ '/api/romeka/imports/1/scan': () => jsonResponse({ jobId: 9 }, { status: 202 }) }));
    expect(await api.scan(ID)).toEqual({ kind: 'job', jobId: 9 });
  });

  it('asks for the sections it names, with their queries, and parses just those', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports/1/preview': () => answer('preview-check.json') }));
    const preview = await api.preview(ID, ['check'], { seed: 2 });
    expect(preview.check.canRun).toBe(true);
    expect(requestBody(only(requestsFor(fetchMock, '/api/romeka/imports/1/preview')))).toEqual({ include: ['check'], seed: 2 });
    await expect(api.preview(ID, ['records'])).rejects.toThrow(ParseError);
  });

  it('runs with the digest it checked, confirms a delete, stops and undoes', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/imports/1/run': () => answer('run.json'),
        '/api/romeka/imports/1/stop': () => jsonResponse({}, { status: 202 }),
        '/api/romeka/imports/1/undo': () => answer('undo.json'),
      }),
    );
    expect(await api.run(ID, { digest: 'abc', confirm: 3 })).toEqual({ jobId: 1 });
    await expect(api.stop(ID)).resolves.toBeUndefined();
    expect(await api.undo(ID)).toEqual({ jobId: 3 });
    await api.undo(ID, 'all');
    await api.undo(ID, [{ kind: 'item', id: 4 }]);
    expect(requestBody(only(requestsFor(fetchMock, '/api/romeka/imports/1/run')))).toEqual({ digest: 'abc', confirm: 3 });
    const undos = requestsFor(fetchMock, '/api/romeka/imports/1/undo').map(requestBody);
    expect(undos).toEqual([{}, { force: 'all' }, { force: [{ kind: 'item', id: 4 }] }]);
  });

  it('uploads a filled list as a multipart form with one file', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports/1/sheets': () => answer('import-sheet-added.json', 201) }));
    const added = await api.addList(ID, new File(['Identifier,Title [dcterms:title]\n'], 'list.csv', { type: 'text/csv' }));
    expect(added.plan.sheets[0]?.file).toBe('lists/1.csv');
    const sent = only(requestsFor(fetchMock, '/api/romeka/imports/1/sheets'));
    expect(sent.init?.method).toBe('POST');
    const form = sent.init?.body;
    expect(form).toBeInstanceOf(FormData);
    expect(form instanceof FormData ? form.get('file') : null).toBeInstanceOf(File);
    expect(new Headers(sent.init?.headers).has('Content-Type')).toBe(false);
  });
});

describe('a plan of several series', () => {
  const refused = (code: string): Response =>
    jsonResponse(
      { errors: { error: 'This plan has several series. Choose the series the list is for.' }, code, series: ['letters', 'photos'] },
      { status: 400 },
    );

  it('names the series of the downloaded list and of the uploaded one', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/imports/1/template.csv': () => new Response(csv('template.csv'), { headers: { 'Content-Type': 'text/csv' } }),
        '/api/romeka/imports/1/sheets': () => answer('import-sheet-added.json', 201),
      }),
    );
    await api.templateCsv(ID, ['dcterms:title'], 'letters');
    expect(only(requestsFor(fetchMock, '/api/romeka/imports/1/template.csv')).url.search).toBe('?columns=dcterms%3Atitle&series=letters');
    expect(api.templateUrl(ID, ['dcterms:title'], 'photos')).toBe('/api/romeka/imports/1/template.csv?columns=dcterms%3Atitle&series=photos');
    await api.addList(ID, new File(['Identifier\n'], 'list.csv', { type: 'text/csv' }), 'photos');
    expect(only(requestsFor(fetchMock, '/api/romeka/imports/1/sheets')).url.search).toBe('?series=photos');
  });

  it.each(['seriesNeeded', 'unknownSeries'] as const)('%s is a typed refusal that carries the series ids', async (code) => {
    installFetch(route({ '/api/romeka/imports/1/sheets': () => refused(code), '/api/romeka/imports/1/template.csv': () => refused(code) }));
    const upload = await api.addList(ID, new File(['Identifier\n'], 'list.csv')).catch((caught: unknown) => caught);
    expect(upload).toMatchObject({ status: 400 });
    expect(seriesRefusalOf(upload)).toEqual({ code, series: ['letters', 'photos'] });
    const download = await api.templateCsv(ID, []).catch((caught: unknown) => caught);
    expect(seriesRefusalOf(download)).toEqual({ code, series: ['letters', 'photos'] });
    expect(conflictOf(download)).toBeNull();
  });

  it('a 400 with no series code is no series refusal', async () => {
    installFetch(route({ '/api/romeka/imports/1': () => answer('error-plan-refused.json', 400) }));
    const error = await api.save(ID, parseImportPlan(savedPlan()), 0).catch((caught: unknown) => caught);
    expect(seriesRefusalOf(error)).toBeNull();
  });
});

describe('the pairs list', () => {
  it('uploads a filled list as a multipart form and reads the cells that made no pair', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports/1/pairs': () => answer('pairs-added.json') }));
    const result = await api.addPairs(ID, new File(['Row [sheet:key],Files\n'], 'pairs.csv', { type: 'text/csv' }));
    expect(result.added).toBe(1);
    expect(result.already).toBe(0);
    expect(result.unmatchedTotal).toBe(2);
    expect(result.unmatched.map((cell) => cell.why)).toEqual(['noItem', 'noRow']);
    expect(result.import.plan.decisions.some((decision) => decision.kind === 'pair')).toBe(true);
    const sent = only(requestsFor(fetchMock, '/api/romeka/imports/1/pairs'));
    expect(sent.init?.method).toBe('POST');
    expect(sent.init?.body).toBeInstanceOf(FormData);
    expect(new Headers(sent.init?.headers).has('Content-Type')).toBe(false);
  });
});

describe('the 409 codes', () => {
  const CASES: readonly (readonly [string, ImportConflict])[] = [
    ['conflict-stale.json', { code: 'stale', revision: 1 }],
    ['conflict-not-draft.json', { code: 'notDraft', status: 'running' }],
    ['conflict-v1-plan.json', { code: 'v1Plan' }],
    ['conflict-type-exists.json', { code: 'typeExists', label: 'Picture' }],
    ['conflict-nothing.json', { code: 'nothing' }],
    ['conflict-confirm.json', { code: 'confirm', deletes: 1 }],
    ['conflict-busy.json', { code: 'busy', importId: 1 }],
    ['conflict-cannot-run.json', { code: 'cannotRun', status: 'done' }],
    ['conflict-cannot-undo.json', { code: 'cannotUndo', status: 'draft' }],
    ['conflict-not-running.json', { code: 'notRunning', status: 'draft' }],
  ];

  it.each(CASES)('%s is a typed conflict with its members', async (name, expected) => {
    installFetch(route({ '/api/romeka/imports/1/run': () => answer(name, 409) }));
    const error = await api.run(ID, { digest: 'x' }).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(ApiError);
    expect(conflictOf(error)).toEqual(expected);
  });

  it('changed carries the digest the run found, and no stored revision', async () => {
    installFetch(route({ '/api/romeka/imports/1/run': () => answer('conflict-changed.json', 409) }));
    const error = await api.run(ID, { digest: 'x' }).catch((caught: unknown) => caught);
    const conflict = conflictOf(error);
    expect(conflict).toMatchObject({ code: 'changed', revision: null });
    expect(conflict?.code === 'changed' ? conflict.digest : null).toMatch(/^[0-9a-f]{64}$/);
  });

  it('blocked carries the flags that stop the run', async () => {
    installFetch(route({ '/api/romeka/imports/1/run': () => answer('conflict-blocked.json', 409) }));
    const error = await api.run(ID, { digest: 'x' }).catch((caught: unknown) => caught);
    const conflict = conflictOf(error);
    expect(conflict?.code).toBe('blocked');
    expect(conflict?.code === 'blocked' ? conflict.blocking[0]?.kind : null).toBe('unknown-term');
  });

  it('every code has a fixture, and an error with no code is a plain ApiError', async () => {
    const covered = new Set([...CASES.map(([, conflict]) => conflict.code), 'blocked', 'changed']);
    expect(covered).toEqual(new Set(CONFLICT_CODES));
    installFetch(route({ '/api/romeka/imports/1': () => answer('error-not-found.json', 404) }));
    const error = await api.get(ID).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ status: 404 });
    expect(conflictOf(error)).toBeNull();
  });

  it('a 400 for a refused plan is an ApiError with the server message, not a conflict', async () => {
    installFetch(route({ '/api/romeka/imports/1': () => answer('error-plan-refused.json', 400) }));
    const error = await api.save(ID, parseImportPlan(savedPlan()), 0).catch((caught: unknown) => caught);
    expect(error).toMatchObject({ status: 400 });
    expect(conflictOf(error)).toBeNull();
  });

  it('a 409 with a code it does not know stays a plain ApiError', async () => {
    installFetch(route({ '/api/romeka/imports/1/run': () => jsonResponse({ errors: { error: 'x' }, code: 'newCode' }, { status: 409 }) }));
    const error = await api.run(ID, { digest: 'x' }).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(ApiError);
    expect(conflictOf(error)).toBeNull();
  });
});
