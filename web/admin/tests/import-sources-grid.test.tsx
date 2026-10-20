/**
 * The Import page's Sources and Grid steps (`pages/import/sources.tsx`,
 * `grid.tsx`) through the real client and parsers against a stubbed fetch:
 * the preview queries each step sends, what it renders from a canned
 * preview, and the plan each edit and bulk fix writes.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { installFetch, jsonResponse, listResponse, requestBody, requestsFor, route } from './stub';
import type { CapturedRequest } from './stub';

const ID = 7;
const PATH = `/api/romeka/imports/${String(ID)}`;
const PREVIEW = `${PATH}/preview`;

const csvPlan = {
  file: 'BHC_006.csv',
  delimiter: ';',
  headerRow: 2,
  windows1252: false,
  trim: false,
  joinColumn: 'Identifier',
  columns: { Title: 'dcterms:title' },
};

const planWith = (csv: unknown): Record<string, unknown> => ({
  pattern: '^(?<identifier>[A-Z]+_[0-9]{3})\\.(?<ext>jpg)$',
  example: null,
  setLevel: 'level1',
  sets: {},
  typeFor: {},
  csv,
  meta: [],
  priority: 'csv',
  edits: {},
  skip: [],
  withoutFiles: [],
  csvRow: {},
  problemRows: 'leave',
});

const detailWith = (plan: Record<string, unknown>): Record<string, unknown> => ({
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
});

const property = (id: number, term: string, label: string): Record<string, unknown> => ({
  '@id': `http://localhost/api/properties/${String(id)}`,
  '@type': ['o:Property'],
  'o:id': id,
  'o:local_name': term.split(':')[1] ?? term,
  'o:label': label,
  'o:comment': null,
  'o:term': term,
  'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
});

const PROPERTIES = [
  property(1, 'dcterms:title', 'Title'),
  property(7, 'dcterms:date', 'Date'),
  property(3, 'dcterms:subject', 'Subject'),
];

const csvPreview = {
  files: ['BHC_006.csv'],
  head: ['Identifier', 'Title', 'Notes'],
  rawLines: ['Bishop Heber College archive', 'Identifier;Title;Notes', 'AF_001;Brass lamp;kept in store'],
  detected: { delimiter: ';', headerRow: 2 },
  cellsLookLike1252: 4,
  cellsWithSpaces: 3,
  rowCountError: null,
  joined: 118,
  rowsWithoutFile: 2,
  itemsWithoutRow: 5,
};

const tagsPreview = [
  { tag: 'DateTimeOriginal', kind: 'exif', found: 80, sample: '2019:03:04 10:12:00' },
  { tag: 'Artist', kind: 'exif', found: 80, sample: 'BHC digitisation team' },
];

const TOTAL = 120;
const KINDS = [
  { kind: 'empty', term: 'dcterms:title', count: 12 },
  { kind: 'space', term: 'dcterms:title', count: 3 },
  { kind: 'nofile', term: null, count: 2 },
  { kind: 'date', term: 'dcterms:date', count: 1 },
];

/** The one canned row whose identifier item 412 already holds. */
const EXISTING_AT = 4;

const identifierAt = (index: number): string => `AF_${String(index + 1).padStart(3, '0')}`;

const rowAt = (index: number): Record<string, unknown> => ({
  identifier: identifierAt(index),
  sets: ['Artifacts'],
  type: 'Base Resource',
  files: [`Artifacts/${identifierAt(index)}.jpg`],
  cells: [
    index === 0
      ? { term: 'dcterms:title', value: '', source: '', problems: ['empty'] }
      : { term: 'dcterms:title', value: `Lamp ${String(index)}`, source: 'csv', problems: [] },
    index === 1
      ? { term: 'dcterms:date', value: 'circa 1950', source: 'csv', problems: ['date'], conflict: { csv: 'circa 1950', files: '2019-03-04' } }
      : { term: 'dcterms:date', value: '1950', source: 'file', problems: [] },
  ],
  extra: ['kept in store'],
  rowProblems: index === 2 ? [{ kind: 'nofile' }] : [],
  skipped: false,
  problems: index < 3 ? 1 : 0,
  existing: index === EXISTING_AT ? { itemId: 412 } : null,
});

const rowsFor = (query: Record<string, unknown>): Record<string, unknown> => {
  const offset = typeof query['offset'] === 'number' ? query['offset'] : 0;
  const limit = typeof query['limit'] === 'number' ? query['limit'] : 50;
  const kind = KINDS.find((entry) => entry.kind === query['kind'] && (query['term'] === undefined || entry.term === query['term']));
  const filtered = kind?.count ?? (query['problemsOnly'] === true ? 30 : TOTAL);
  return {
    columns: [
      { term: 'dcterms:title', label: 'Title', sources: ['type', 'csv:Title'] },
      { term: 'dcterms:date', label: 'Date', sources: ['tag:DateTimeOriginal'] },
    ],
    unmapped: ['Notes'],
    total: TOTAL,
    withProblems: 30,
    existing: 1,
    filtered,
    kinds: KINDS,
    page: Array.from({ length: Math.max(0, Math.min(limit, filtered - offset)) }, (_entry, index) =>
      rowAt(offset + index),
    ),
  };
};

const stub = (plan: Record<string, unknown>) =>
  installFetch(
    route({
      [PATH]: () => jsonResponse(detailWith(plan)),
      '/api/properties': () => listResponse(PROPERTIES, PROPERTIES.length),
      [PREVIEW]: (_url, init) => {
        const body: unknown = JSON.parse(typeof init?.body === 'string' ? init.body : '{}');
        const sent = isRecord(body) ? body : {};
        const include = Array.isArray(sent['include']) ? sent['include'] : [];
        const rows = isRecord(sent['rows']) ? sent['rows'] : {};
        return jsonResponse({
          ...(include.includes('csv') ? { csv: csvPreview } : {}),
          ...(include.includes('tags') ? { tags: tagsPreview } : {}),
          ...(include.includes('files')
            ? { files: { total: 90, matched: 80, items: 40, groupNames: [], buckets: {}, biggest: null, filtered: 80, page: [] } }
            : {}),
          ...(include.includes('rows') ? { rows: rowsFor(rows) } : {}),
        });
      },
    }),
  );

type FetchMock = ReturnType<typeof stub>;

const previewBodies = (fetchMock: FetchMock): readonly Record<string, unknown>[] =>
  requestsFor(fetchMock, PREVIEW).map(requestBody);

const rowsQueries = (fetchMock: FetchMock): readonly Record<string, unknown>[] =>
  previewBodies(fetchMock).flatMap((body) => (isRecord(body['rows']) ? [body['rows']] : []));

const savedPlans = (fetchMock: FetchMock): readonly Record<string, unknown>[] =>
  requestsFor(fetchMock, PATH)
    .filter((request: CapturedRequest) => request.init?.method === 'PUT')
    .flatMap((request) => {
      const plan = requestBody(request)['plan'];
      return isRecord(plan) ? [plan] : [];
    });

const lastSaved = async (fetchMock: FetchMock): Promise<Record<string, unknown>> => {
  await waitFor(
    () => {
      expect(savedPlans(fetchMock).length).toBeGreaterThan(0);
    },
    { timeout: 3000 },
  );
  const plan = savedPlans(fetchMock).at(-1);
  if (plan === undefined) {
    throw new Error('no saved plan');
  }
  return plan;
};

const renderStep = (step: string) => render(<ImportPage param={String(ID)} rest={[String(ID), step]} />);

afterEach(() => {
  window.location.hash = '';
});

describe('Grid step', () => {
  it('asks for 50 rows, shows the problem kinds, the bad cells in words and the page range, and pages', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    expect(await screen.findByText('Rows 1–50 of 120')).toBeInTheDocument();
    expect(rowsQueries(fetchMock)[0]).toEqual({ problemsOnly: false, offset: 0, limit: 50 });
    expect(previewBodies(fetchMock)[0]).toMatchObject({ include: ['rows'], plan: { csv: { file: 'BHC_006.csv' } } });

    expect(screen.getByText('Rows: 120. Rows with problems: 30.')).toBeInTheDocument();
    expect(screen.getByText('Title is empty, but it is required. Rows: 12')).toBeInTheDocument();
    expect(screen.getByText('The spreadsheet row matches no file. Rows: 2')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^circa 1950/ })).toHaveTextContent('Not a date');
    expect(screen.getByRole('button', { name: /^Empty/ })).toHaveTextContent('Required');
    expect(screen.getByText('No file matches this row.')).toBeInTheDocument();
    const table = screen.getByRole('table');
    expect(within(table).getAllByRole('columnheader')).toHaveLength(8);
    expect(within(table).getByRole('rowheader', { name: 'AF_001' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(await screen.findByText('Rows 51–100 of 120')).toBeInTheDocument();
    expect(rowsQueries(fetchMock).at(-1)).toEqual({ problemsOnly: false, offset: 50, limit: 50 });
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(await screen.findByText('Rows 101–120 of 120')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Next page' })).toBeDisabled();
  });

  it('marks a row an item already holds, links to that item, and can show only such rows', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    expect(await screen.findByText('Rows that already exist: 1.')).toBeInTheDocument();
    const row = within(screen.getByRole('rowheader', { name: 'AF_005' }).closest('tr') ?? document.body);
    expect(row.getByText('Already exists. Not imported again.')).toBeInTheDocument();
    expect(row.getByRole('link', { name: 'Open item 412' })).toHaveAttribute('href', '#/items/412');
    expect(row.queryByRole('button', { name: 'Skip row' })).toBeNull();
    const other = within(screen.getByRole('rowheader', { name: 'AF_004' }).closest('tr') ?? document.body);
    expect(other.queryByRole('link', { name: /^Open item/ })).toBeNull();

    fireEvent.click(screen.getByRole('checkbox', { name: 'Show only rows that already exist' }));
    await waitFor(() => {
      expect(rowsQueries(fetchMock).at(-1)).toEqual({ problemsOnly: false, existingOnly: true, offset: 0, limit: 50 });
    });
  });

  it('filters by a kind, by problems only and by identifier, each from the first page', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    await screen.findByText('Rows 1–50 of 120');
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    await screen.findByText('Rows 51–100 of 120');

    const kind = screen.getByText('Title is empty, but it is required. Rows: 12').closest('li');
    fireEvent.click(within(kind ?? document.body).getByRole('button', { name: 'Show them' }));
    expect(await screen.findByText('Rows 1–12 of 12')).toBeInTheDocument();
    expect(rowsQueries(fetchMock).at(-1)).toEqual({
      problemsOnly: false,
      kind: 'empty',
      term: 'dcterms:title',
      offset: 0,
      limit: 50,
    });
    expect(screen.getByText(/^Only: Title is empty/)).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: 'Show all rows' })[0] ?? document.body);
    fireEvent.click(screen.getByRole('checkbox', { name: 'Show only rows with problems' }));
    expect(await screen.findByText('Rows 1–30 of 30')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'Find identifier' }), { target: { value: ' AF_01 ' } });
    await waitFor(() => {
      expect(rowsQueries(fetchMock).at(-1)).toEqual({ problemsOnly: true, query: 'AF_01', offset: 0, limit: 50 });
    });
  });

  it('applies a bulk fix to every row of the kind, read 200 at a time, and saves it in the plan', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    const kind = (await screen.findByText('The spreadsheet row matches no file. Rows: 2')).closest('li');
    fireEvent.click(within(kind ?? document.body).getByRole('button', { name: 'Import them without files' }));
    await waitFor(() => {
      expect(rowsQueries(fetchMock)).toContainEqual({ problemsOnly: false, kind: 'nofile', offset: 0, limit: 200 });
    });
    expect(await lastSaved(fetchMock)).toMatchObject({ withoutFiles: ['AF_001', 'AF_002'] });

    const spaces = screen.getByText('Title has spaces at the start or end. Rows: 3').closest('li');
    fireEvent.click(within(spaces ?? document.body).getByRole('button', { name: 'Remove the spaces' }));
    await waitFor(
      () => {
        expect(savedPlans(fetchMock).at(-1)).toMatchObject({ csv: { trim: true }, withoutFiles: ['AF_001', 'AF_002'] });
      },
      { timeout: 3000 },
    );
  });

  it('fills the empty cells of a field with the identifier in the value', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    const kind = (await screen.findByText('Title is empty, but it is required. Rows: 12')).closest('li');
    fireEvent.click(within(kind ?? document.body).getByRole('button', { name: 'Fill them' }));
    fireEvent.change(await screen.findByRole('textbox', { name: /becomes the identifier of each row/ }), {
      target: { value: 'Untitled {identifier}' },
    });
    // The canned rows have one empty title among the 12 the kind selects.
    fireEvent.click(await screen.findByRole('button', { name: 'Fill the empty cells: 1' }));
    expect(rowsQueries(fetchMock)).toContainEqual({ problemsOnly: false, kind: 'empty', term: 'dcterms:title', offset: 0, limit: 200 });
    expect(await lastSaved(fetchMock)).toMatchObject({ edits: { AF_001: { 'dcterms:title': 'Untitled AF_001' } } });
  });

  it('edits a cell, resolves a conflict with the files, and skips a row', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    fireEvent.click(await screen.findByRole('button', { name: /^circa 1950/ }));
    expect(screen.getByText('The spreadsheet says “circa 1950”. The files say “2019-03-04”.')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Not a date');
    fireEvent.click(screen.getByRole('button', { name: 'Use the files' }));
    await waitFor(() => {
      expect(previewBodies(fetchMock).at(-1)).toMatchObject({ plan: { edits: { AF_002: { 'dcterms:date': '2019-03-04' } } } });
    });

    fireEvent.click(screen.getByRole('button', { name: 'Lamp 3' }));
    fireEvent.change(screen.getByRole('textbox', { name: /AF_004 · Title/ }), { target: { value: 'Brass lamp' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    const row = screen.getByRole('rowheader', { name: 'AF_003' }).closest('tr');
    fireEvent.click(within(row ?? document.body).getByRole('button', { name: 'Skip row' }));
    expect(await lastSaved(fetchMock)).toMatchObject({
      edits: { AF_002: { 'dcterms:date': '2019-03-04' }, AF_004: { 'dcterms:title': 'Brass lamp' } },
      skip: ['AF_003'],
    });
  });

  it('maps an unmapped spreadsheet column to a new local field', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('grid');
    fireEvent.click(await screen.findByRole('button', { name: 'Map…' }));
    expect(screen.getByRole('heading', { name: 'Spreadsheet column “Notes”' })).toBeInTheDocument();
    expect(await screen.findByRole('option', { name: 'Subject (dcterms:subject)' })).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: /for this archive only/ }), { target: { value: 'Storage note' } });
    fireEvent.click(screen.getByRole('button', { name: 'Make field' }));
    expect(await lastSaved(fetchMock)).toMatchObject({
      csv: { columns: { Title: 'dcterms:title', Notes: { newField: 'Storage note' } } },
    });
  });
});

describe('Sources step', () => {
  it('chooses the spreadsheet, takes the detected delimiter and header row, and shows the repairs and the join', async () => {
    const fetchMock = stub(planWith(null));
    renderStep('sources');
    expect(await screen.findByText(/^No spreadsheet\./)).toBeInTheDocument();
    expect(previewBodies(fetchMock)[0]).toMatchObject({
      include: ['csv', 'tags', 'files', 'rows'],
      files: { limit: 0 },
      rows: { limit: 0 },
    });

    fireEvent.click(screen.getByRole('radio', { name: 'BHC_006.csv' }));
    await waitFor(() => {
      expect(screen.getByRole('radio', { name: 'Semicolon (detected)' })).toBeChecked();
    });
    expect(screen.getByRole('radio', { name: 'Line 2 (detected)' })).toBeChecked();
    expect(screen.getByText('header row')).toBeInTheDocument();
    expect(within(screen.getByLabelText('The first lines of the spreadsheet')).getByText('Identifier;Title;Notes').tagName).toBe('MARK');
    expect(screen.getByRole('checkbox', { name: 'Read as Windows-1252. Cells that look wrong: 4' })).not.toBeChecked();

    fireEvent.change(screen.getByRole('combobox', { name: /Match rows to items by the column/ }), {
      target: { value: 'Identifier' },
    });
    expect(screen.getByText('Rows that match an item: 118. Rows that match no file: 2. Items with no row: 5.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Remove spaces at the start and end. Cells with spaces: 3' }));
    await waitFor(() => {
      expect(previewBodies(fetchMock).at(-1)).toMatchObject({
        plan: { csv: { file: 'BHC_006.csv', delimiter: ';', headerRow: 2, joinColumn: 'Identifier', trim: true } },
      });
    });
  });

  it('maps a tag to a field the user picks, on the item, from all files', async () => {
    const fetchMock = stub(planWith(csvPlan));
    renderStep('sources');
    const field = await screen.findByRole('combobox', { name: 'Field for Artist' });
    expect(screen.getAllByText('80 of 80')).toHaveLength(2);
    expect(screen.getByRole('combobox', { name: 'Where Artist goes' })).toBeDisabled();
    await within(field).findByRole('option', { name: 'Subject (dcterms:subject)' });
    fireEvent.change(field, { target: { value: 'dcterms:subject' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Which file gives the item value of Artist' }), {
      target: { value: 'all' },
    });
    fireEvent.click(screen.getByRole('radio', { name: 'Use the files' }));
    expect(await lastSaved(fetchMock)).toMatchObject({
      meta: [{ tag: 'Artist', term: 'dcterms:subject', on: 'item', from: 'all' }],
      priority: 'files',
    });
  });
});
