/**
 * The Import page (`pages/import/`) through the real client and parsers
 * against a stubbed fetch: the seven step tabs and Back/Next, the Folder
 * step's paged and filtered file requests and its optional-part hint, the
 * debounced plan save, and the Item sets step.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';
import type { CapturedRequest } from './stub';

const ID = 7;
const PATH = `/api/romeka/imports/${String(ID)}`;
const PREVIEW = `${PATH}/preview`;

const plan = {
  pattern: '^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>[A-Z]+_[0-9]{3}_[A-Z]+_[0-9]{3})_(?<part>[a-z]+)\\.(?<ext>[A-Za-z0-9]+)$',
  example: {
    path: 'Artifacts/BHC_006_AF_001_front.jpg',
    labels: { d0: 'set', n0: 'identifier', n2: 'identifier', n4: 'identifier', n6: 'identifier', n8: 'part' },
    optional: [],
  },
  setLevel: 'level1',
  sets: {},
  typeFor: {},
  csv: null,
  meta: [],
  priority: 'csv',
  edits: {},
  skip: [],
  withoutFiles: [],
  csvRow: {},
  problemRows: 'leave',
};

const detail = {
  id: ID,
  folder: 'BHC_006',
  status: 'draft',
  undoable: false,
  created: '2026-10-19T09:00:00',
  finished: null,
  summary: null,
  plan,
  jobId: null,
  undoReport: null,
};

const TOTAL = 120;
const MATCHED = 80;

const fileRow = (index: number, matched: boolean): Record<string, unknown> => {
  const path = `Artifacts/BHC_006_AF_${String(index).padStart(3, '0')}_front.jpg`;
  return {
    path,
    matched,
    item: matched ? index + 1 : null,
    itemFiles: matched ? 2 : 0,
    groups: matched ? { set: [0, 9], identifier: [10, 24], part: [25, 30], ext: [31, 34] } : {},
  };
};

const filesFor = (body: Record<string, unknown>): Record<string, unknown> => {
  const query = isRecord(body['files']) ? body['files'] : {};
  const sentPlan = isRecord(body['plan']) ? body['plan'] : {};
  const optional = typeof sentPlan['pattern'] === 'string' && sentPlan['pattern'].includes('(?:_(?<part>');
  const filter = query['filter'];
  const offset = typeof query['offset'] === 'number' ? query['offset'] : 0;
  const limit = typeof query['limit'] === 'number' ? query['limit'] : 50;
  const filtered = filter === 'miss' ? TOTAL - MATCHED : filter === 'match' ? MATCHED : TOTAL;
  const page = Array.from({ length: Math.max(0, Math.min(limit, filtered - offset)) }, (_entry, index) =>
    fileRow(offset + index, filter !== 'miss'),
  );
  return {
    total: TOTAL,
    // With the part optional, 12 files without a part match too.
    matched: optional ? MATCHED + 12 : MATCHED,
    items: 40,
    groupNames: ['set', 'identifier', 'part', 'ext'],
    buckets: { '2': 40 },
    biggest: { identifier: 'BHC_006_AF_001', files: 2 },
    filtered,
    page,
  };
};

const setsPreview = [
  { key: 'Artifacts', name: 'Artifacts', use: true, matched: 6, total: 7, existing: false },
  { key: 'Photographs', name: 'Photographs', use: true, matched: 5, total: 5, existing: true },
];

const stub = () =>
  installFetch(
    route({
      [PATH]: (_url, init) => jsonResponse(init?.method === 'PUT' ? detail : detail),
      [PREVIEW]: (_url, init) => {
        const body: unknown = JSON.parse(typeof init?.body === 'string' ? init.body : '{}');
        const sent = isRecord(body) ? body : {};
        const include = Array.isArray(sent['include']) ? sent['include'] : [];
        return jsonResponse({
          ...(include.includes('files') ? { files: filesFor(sent) } : {}),
          ...(include.includes('sets') ? { sets: setsPreview } : {}),
        });
      },
    }),
  );

const filesOf = (request: CapturedRequest): Record<string, unknown> => {
  const files = requestBody(request)['files'];
  return isRecord(files) ? files : {};
};

/** The table-page requests: the candidate counts ask for no rows. */
const pageRequests = (fetchMock: ReturnType<typeof stub>): readonly Record<string, unknown>[] =>
  requestsFor(fetchMock, PREVIEW)
    .map(filesOf)
    .filter((files) => files['limit'] === 50);

const renderStep = (step: string) =>
  render(<ImportPage param={String(ID)} rest={[String(ID), step]} />);

afterEach(() => {
  window.location.hash = '';
});

describe('Import page steps', () => {
  it('shows the seven steps as tabs, marks the current one, and moves with Next and Back', async () => {
    stub();
    const view = renderStep('folder');
    const tabs = await screen.findByRole('navigation', { name: 'Import steps' });
    const links = within(tabs).getAllByRole('link');
    expect(links.map((link) => link.textContent)).toEqual([
      '1. Folder',
      '2. Item sets',
      '3. Item types',
      '4. Sources',
      '5. Grid',
      '6. Check',
      '7. Run',
    ]);
    expect(links.map((link) => link.getAttribute('href'))).toEqual(
      ['folder', 'sets', 'types', 'sources', 'grid', 'check', 'run'].map((slug) => `#/import/7/${slug}`),
    );
    expect(within(tabs).getByRole('link', { name: '1. Folder' })).toHaveAttribute('aria-current', 'step');
    expect(screen.getByRole('button', { name: 'Back' })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(window.location.hash).toBe('#/import/7/sets');

    view.rerender(<ImportPage param={String(ID)} rest={[String(ID), 'check']} />);
    expect(await screen.findByText('A dry run. Nothing is written yet.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(window.location.hash).toBe('#/import/7/grid');

    view.rerender(<ImportPage param={String(ID)} rest={[String(ID), 'run']} />);
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
  });

  it('refuses a step that does not exist', async () => {
    stub();
    renderStep('nowhere');
    expect(await screen.findByRole('link', { name: 'No such step. Go to the first step.' })).toHaveAttribute(
      'href',
      '#/import/7/folder',
    );
  });
});

describe('Folder step', () => {
  it('asks the server for 50 files, the next 50, and a new first page per filter', async () => {
    const fetchMock = stub();
    renderStep('folder');
    expect(await screen.findByText('Files that match: 80 of 120. Items they make: 40.')).toBeInTheDocument();
    const firstPage = pageRequests(fetchMock)[0];
    expect(firstPage).toEqual({ filter: 'all', offset: 0, limit: 50 });
    expect(requestBody(requestsFor(fetchMock, PREVIEW)[0] ?? { url: new URL('http://x'), init: undefined })['plan']).toMatchObject({
      pattern: plan.pattern,
    });
    expect(screen.getByText('Showing 50 of 120.')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Show 50 more' }));
    expect(await screen.findByText('Showing 100 of 120.')).toBeInTheDocument();
    expect(pageRequests(fetchMock).at(-1)).toEqual({ filter: 'all', offset: 50, limit: 50 });

    fireEvent.click(screen.getByRole('button', { name: 'No match (40)' }));
    await waitFor(() => {
      expect(pageRequests(fetchMock).at(-1)).toEqual({ filter: 'miss', offset: 0, limit: 50 });
    });
    expect(await screen.findAllByText('No match')).not.toHaveLength(0);
    expect(screen.queryByText(/^Showing/)).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: '2 files: 40 items' }));
    await waitFor(() => {
      expect(pageRequests(fetchMock).at(-1)).toEqual({ filter: 'match', perItem: 2, offset: 0, limit: 50 });
    });
  });

  it('colours each group of a path from the preview offsets', async () => {
    stub();
    renderStep('folder');
    const cell = await screen.findByRole('button', { name: 'Artifacts/BHC_006_AF_000_front.jpg' });
    expect(within(cell).getByText('BHC_006_AF_000')).toHaveClass('import-tag-identifier');
    expect(within(cell).getByText('front')).toHaveClass('import-tag-part');
  });

  it('offers the part as optional with the count a candidate preview found, and saves the choice', async () => {
    const fetchMock = stub();
    renderStep('folder');
    expect(await screen.findByText('Some files have no part.')).toBeInTheDocument();
    expect(screen.getByText('Files that would also match: 12')).toBeInTheDocument();
    const candidate = requestsFor(fetchMock, PREVIEW).find((request) => filesOf(request)['limit'] === 0);
    expect(candidate === undefined ? null : filesOf(candidate)).toEqual({ filter: 'match', offset: 0, limit: 0 });

    fireEvent.click(screen.getByRole('button', { name: 'Make it optional' }));
    expect(await screen.findByText('The part is optional.')).toBeInTheDocument();
    expect(screen.getByText('{set}/…/{identifier}[_{part}].{ext}')).toBeInTheDocument();
    await waitFor(
      () => {
        expect(requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'PUT')).toHaveLength(1);
      },
      { timeout: 3000 },
    );
    const put = requestsFor(fetchMock, PATH).find((request) => request.init?.method === 'PUT');
    const saved = put === undefined ? {} : requestBody(put)['plan'];
    expect(saved).toMatchObject({ example: { optional: ['part'] } });
    expect(isRecord(saved) ? saved['pattern'] : null).toContain('(?:_(?<part>[\\p{L}\\p{M}]+))?');
  });

  it('marks a chip: choosing Page for the last name part rewrites the shape', async () => {
    stub();
    renderStep('folder');
    const parts = await screen.findByRole('group', { name: 'Parts of the example file name' });
    fireEvent.click(within(parts).getByRole('button', { name: /front/ }));
    const choice = screen.getByRole('group', { name: '“front” is' });
    expect(within(choice).getAllByRole('button').map((button) => button.textContent)).toEqual([
      'Identifier',
      'Part',
      'Page',
      'Nothing',
    ]);
    act(() => {
      fireEvent.click(within(choice).getByRole('button', { name: 'Page' }));
    });
    expect(screen.getByText('{set}/…/{identifier}_{page}.{ext}')).toBeInTheDocument();
  });
});

describe('Item sets step', () => {
  it('lists the folders the preview found, with the reuse badge for an existing set', async () => {
    stub();
    renderStep('sets');
    const table = await screen.findByRole('table');
    expect(within(table).getByText('Adds to the existing set')).toBeInTheDocument();
    expect(within(table).getByText('New item set')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Use Artifacts' }));
    expect(screen.getByText('Skipped. Its files are left out: 7.')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'The regex group “set”' })).toBeEnabled();
  });
});
