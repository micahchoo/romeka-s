/**
 * The archive import client (`src/api/imports.ts`, `import-preview.ts`)
 * against the contract in docs/archive-import.md: each route's request and the
 * parse of its answer, through a stubbed fetch.
 */
import { describe, expect, it } from 'vitest';
import { ApiClient } from '../src/api/client';
import {
  createImport,
  deleteImport,
  getArchive,
  getImport,
  getJobLog,
  getLeftOutCsv,
  listImports,
  previewImport,
  runImport,
  saveImportPlan,
  undoImport,
} from '../src/api/imports';
import type { ImportPlan } from '../src/api/imports';
import { ParseError, brandId } from '../src/api/parse';
import { installFetch, jsonResponse, noContent, requestBody, requestsFor, route } from './stub';

const api = new ApiClient({ baseUrl: '' });
const ID = brandId<'ImportId'>(3);

const plan: ImportPlan = {
  pattern: '^(?<identifier>[^/]+)\\.(?<ext>[A-Za-z0-9]+)$',
  example: null,
  setLevel: 'level1',
  sets: { Letters: { use: true, name: 'Letters' } },
  typeFor: { Letters: brandId<'TemplateId'>(4), Photos: { starter: 'photograph', name: 'Photograph' } },
  csv: {
    file: 'items.csv',
    delimiter: ';',
    headerRow: 2,
    windows1252: true,
    trim: true,
    joinColumn: 'Accession No',
    columns: { Title: 'dcterms:title', Donor: { newField: 'Donor' } },
  },
  meta: [{ tag: 'Artist', term: 'dcterms:creator', on: 'item', from: 'part', part: 'front' }],
  priority: 'csv',
  edits: { BHC_001: { 'dcterms:title': 'Lamp' } },
  skip: ['BHC_009'],
  withoutFiles: [],
  csvRow: { BHC_004: 'keep' },
  problemRows: 'leave',
};

const record = {
  id: 3,
  folder: 'BHC_006',
  status: 'done',
  undoable: true,
  created: '2026-10-19T09:00:00',
  finished: '2026-10-19T09:04:00',
  summary: {
    items: 12,
    files: 14,
    thumbnails: 42,
    left: 2,
    failed: 1,
    newSets: ['Artifacts'],
    newTypes: [],
    newFields: ['Donor'],
    itemSetIds: [5, 9],
    existingSets: ['Photographs'],
    failures: [{ identifier: 'BHC_011', reason: 'disk full' }],
    progress: { done: 12, total: 12 },
  },
};

describe('archive import client', () => {
  it('lists a folder of the archive by its path', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/archive': () =>
          jsonResponse({ enabled: true, path: 'BHC_006', entries: [{ name: 'Artifacts', kind: 'dir', files: 7 }, { name: 'a.csv', kind: 'file' }] }),
      }),
    );
    const listing = await getArchive(api, 'BHC_006');
    expect(listing.entries).toEqual([{ name: 'Artifacts', kind: 'dir', files: 7 }, { name: 'a.csv', kind: 'file' }]);
    expect(requestsFor(fetchMock, '/api/romeka/archive')[0]?.url.searchParams.get('path')).toBe('BHC_006');
  });

  it('reads the history, and refuses a status the contract does not name', async () => {
    installFetch(route({ '/api/romeka/imports': () => jsonResponse([record, { ...record, id: 2, status: 'draft', summary: null, finished: null }]) }));
    const rows = await listImports(api);
    expect(rows.map((row) => [row.id, row.status, row.summary?.items ?? null])).toEqual([
      [3, 'done', 12],
      [2, 'draft', null],
    ]);
    expect(rows[0]?.summary).toMatchObject({ failed: 1, itemSetIds: [5, 9], existingSets: ['Photographs'] });
    installFetch(route({ '/api/romeka/imports': () => jsonResponse([{ ...record, status: 'paused' }]) }));
    await expect(listImports(api)).rejects.toThrow(ParseError);
  });

  it('creates a draft from a folder and saves its plan whole', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/imports': () => jsonResponse({ ...record, status: 'draft', plan, jobId: null, undoReport: null }),
        '/api/romeka/imports/3': () => jsonResponse({ ...record, status: 'draft', plan, undoReport: null }),
      }),
    );
    const created = await createImport(api, 'BHC_006');
    expect(created.plan).toEqual(plan);
    expect(created.jobId).toBeNull();
    const post = requestsFor(fetchMock, '/api/romeka/imports')[0];
    expect(post === undefined ? null : requestBody(post)).toEqual({ folder: 'BHC_006' });

    await saveImportPlan(api, ID, plan);
    const put = requestsFor(fetchMock, '/api/romeka/imports/3')[0];
    expect(put?.init?.method).toBe('PUT');
    const wire: unknown = JSON.parse(JSON.stringify(plan));
    expect(put === undefined ? null : requestBody(put)).toEqual({ plan: wire });
  });

  it('parses the sections a preview asked for and leaves the rest out', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/imports/3/preview': () =>
          jsonResponse({
            error: { key: 'patternSyntax', detail: 'unclosed group' },
            files: {
              total: 2,
              matched: 1,
              items: 1,
              groupNames: ['identifier'],
              buckets: { '1': 1 },
              biggest: null,
              filtered: 2,
              page: [
                { path: 'a/BHC_1.jpg', matched: true, item: 1, itemFiles: 1, groups: { identifier: [2, 7] } },
                { path: 'a/x.txt', matched: false, item: null, itemFiles: 0, groups: {} },
              ],
            },
          }),
      }),
    );
    const preview = await previewImport(api, ID, {
      include: ['files'],
      files: { filter: 'all', offset: 0, limit: 50 },
    });
    expect(preview.error).toEqual({ key: 'patternSyntax', detail: 'unclosed group' });
    expect(preview.files?.page[0]?.groups).toEqual({ identifier: [2, 7] });
    expect(preview.sets).toBeUndefined();
    const sent = requestsFor(fetchMock, '/api/romeka/imports/3/preview')[0];
    expect(sent === undefined ? null : requestBody(sent)).toEqual({
      include: ['files'],
      files: { filter: 'all', offset: 0, limit: 50 },
    });
  });

  it('starts a run and an undo, reads a job log as text, and deletes a draft', async () => {
    installFetch(
      route({
        '/api/romeka/imports/3/run': () => jsonResponse({ jobId: 41 }),
        '/api/romeka/imports/3/undo': () => jsonResponse({ jobId: 42 }),
        '/api/romeka/jobs/41/log': () => new Response('item BHC_001 created\n', { headers: { 'Content-Type': 'text/plain' } }),
        '/api/romeka/imports/3': () => noContent(),
      }),
    );
    expect(await runImport(api, ID)).toEqual({ jobId: 41 });
    expect(await undoImport(api, ID)).toEqual({ jobId: 42 });
    expect(await getJobLog(api, 41)).toBe('item BHC_001 created\n');
    await expect(deleteImport(api, ID)).resolves.toBeUndefined();
  });

  it('reads the undo report on the import, null until an undo ends', async () => {
    const report = { items: 3, media: 4, itemSets: 1, templates: 0, fields: 0, kept: [{ kind: 'field', id: 41 }] };
    const detail = { ...record, plan, jobId: 41 };
    installFetch(route({ '/api/romeka/imports/3': () => jsonResponse({ ...detail, undoReport: null }) }));
    expect((await getImport(api, ID)).undoReport).toBeNull();
    installFetch(route({ '/api/romeka/imports/3': () => jsonResponse({ ...detail, status: 'undone', undoReport: report }) }));
    expect((await getImport(api, ID)).undoReport).toEqual(report);
    installFetch(route({ '/api/romeka/imports/3': () => jsonResponse({ ...detail, undoReport: { ...report, kept: [{ kind: 'item', id: 1 }] } }) }));
    await expect(getImport(api, ID)).rejects.toThrow(ParseError);
  });

  it('reads the left-out rows as the CSV text the server writes', async () => {
    const csv = 'dcterms:identifier,#reason\nBHC_011,failed: disk full\n';
    installFetch(route({ '/api/romeka/imports/3/left.csv': () => new Response(csv, { headers: { 'Content-Type': 'text/csv' } }) }));
    expect(await getLeftOutCsv(api, ID)).toBe(csv);
  });

  it('deletes a draft with DELETE, takes the 204, and rejects a 409', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/imports/3': () => noContent() }));
    await expect(deleteImport(api, ID)).resolves.toBeUndefined();
    expect(requestsFor(fetchMock, '/api/romeka/imports/3').map((request) => request.init?.method)).toEqual(['DELETE']);
    installFetch(route({ '/api/romeka/imports/3': () => jsonResponse({ errors: { error: 'Only a draft can be deleted.' } }, { status: 409 }) }));
    await expect(deleteImport(api, ID)).rejects.toMatchObject({ status: 409 });
  });
});
