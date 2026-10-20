/**
 * The preview requests the admin's types can build. Each typed request below
 * is the body in `fixtures/import/requests.json`, and the server's route test
 * (`imports_wire_fixtures.rs`) sends that file to the real route and takes a
 * 200 for every entry. A request member the server does not take, or a type
 * that cannot spell one it does, fails one side or the other.
 */
import { describe, expect, it } from 'vitest';
import { parseImportPlan } from '../src/api/import-plan-read';
import { ImportApi } from '../src/api/import-routes';
import { UNUSED_REASONS } from '../src/api/import-queries';
import type { PreviewOptions, Unused } from '../src/api/import-queries';
import type { Section, ShapeFacts } from '../src/api/import-sections';
import { asRecord, brandId } from '../src/api/parse';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';

interface Request {
  readonly include: readonly Section[];
  readonly options: PreviewOptions;
}

const SHAPE: ShapeFacts = {
  files: '2',
  folders: ['Artifacts'],
  media: ['image'],
  pattern: true,
  row: true,
  parts: ['back', 'front'],
  pages: null,
};

const entriesFor = (reason: Unused): Request => ({ include: ['entries'], options: { entries: { reason } } });

const REQUESTS: readonly Request[] = [
  {
    include: ['records'],
    options: {
      records: {
        kind: 'item',
        page: 2,
        perPage: 5,
        flag: 'date-form',
        slot: 'dcterms:date',
        text: 'lamp',
        folder: 'Artifacts',
        files: '2',
        left: false,
      },
    },
  },
  { include: ['records'], options: { records: { kind: 'media', set: { existing: 1 }, template: { existing: 1 } } } },
  { include: ['records'], options: { records: { kind: 'itemSet', set: { record: 3 }, template: { new: 0 } } } },
  { include: ['records'], options: { records: { set: { new: 0 }, template: 'none', files: '7+', left: true } } },
  { include: ['records'], options: { records: { shape: SHAPE } } },
  { include: ['shapes'], options: { shapes: { page: 1, perPage: 20, text: 'Artifacts' }, seed: 7 } },
  {
    include: ['entries'],
    options: { entries: { page: 1, perPage: 10, flag: 'not-media', text: 'Misc', used: false, folder: 'Misc' } },
  },
  { include: ['entries'], options: { entries: { paired: true } } },
  ...UNUSED_REASONS.map(entriesFor),
  { include: ['identical'], options: { identical: { page: 1, perPage: 2, text: 'loose' } } },
  {
    include: ['unpaired'],
    options: { unpaired: { rows: { page: 1, perPage: 5, text: 'ghost' }, files: { perPage: 5 } } },
  },
  { include: ['values'], options: { values: { kind: 'media', page: 1, perPage: 10 } } },
  { include: ['check', 'flags', 'anomalies', 'orphans', 'sheets', 'tags', 'histogram'], options: { seed: 0 } },
];

describe('the preview requests', () => {
  it('are the bodies the server takes', () => {
    const sent = REQUESTS.map((request) => ({ include: request.include, ...request.options }));
    expect(sent).toEqual(loadFixture('import/requests.json'));
  });

  it('name every unused reason the entries filter takes', () => {
    expect(UNUSED_REASONS).toEqual(['unmatched', 'decision', 'flag', 'parent', 'noFiles', 'noRecord']);
  });

  it('carry a plan to preview without saving it, exactly as the plan was read', async () => {
    const saved = asRecord(loadFixture('import/import-saved.json'), [])['plan'];
    const fetchMock = installFetch(
      route({ '/api/romeka/imports/1/preview': () => jsonResponse(loadFixture('import/preview-histogram.json')) }),
    );
    await new ImportApi({ baseUrl: '' }).preview(brandId<'ImportId'>(1), ['histogram'], { plan: parseImportPlan(saved) });
    const [sent] = requestsFor(fetchMock, '/api/romeka/imports/1/preview');
    expect(sent === undefined ? null : requestBody(sent)).toEqual({ include: ['histogram'], plan: saved });
  });
});
