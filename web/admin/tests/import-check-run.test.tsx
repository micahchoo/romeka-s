/**
 * The Import page's Check and Run steps and the undo banner through the real
 * client and parsers against a stubbed server: the dry run and its warning,
 * starting a run and following it to the end, undoing it after the
 * confirmation, a run whose job died, the two 409 refusals, and the banner
 * on the Import page.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ImportPage } from '../src/pages/import/page';
import { POLL_MS } from '../src/pages/import/use-import-poll';
import { installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';

const ID = 7;
const PATH = `/api/romeka/imports/${String(ID)}`;

const plan = {
  pattern: '^(?<identifier>[A-Z]+_[0-9]{3})_(?<part>[a-z]+)\\.jpg$',
  example: null,
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

const check = {
  create: 3,
  files: 6,
  newSets: ['Letters'],
  existingSets: ['Photographs'],
  newTypes: ['Letter'],
  newFields: ['local:shelfMark'],
  withProblems: 1,
  skipped: 2,
  existing: 4,
  unmatchedFiles: 12,
  unmatchedSample: Array.from({ length: 12 }, (_entry, index) => `loose/scan_${String(index)}.tif`),
  canRun: true,
  openUndo: { id: 6, folder: 'BHC_005', items: 40 },
};

/** The server's side of one import: its status moves as the stub answers. */
interface Server {
  status: 'draft' | 'running' | 'done' | 'failed' | 'undoing' | 'undone';
  done: number;
  runAnswer: number;
  undoAnswer: number;
  itemSetIds: number[];
  existing: number;
  /** When the server found the job dead, or null. */
  died: string | null;
}

const TOTAL = 3;

const summary = (server: Server): Record<string, unknown> => ({
  items: server.done,
  files: server.done * 2,
  thumbnails: server.done * 2,
  left: 2,
  failed: 0,
  existing: server.existing,
  newSets: ['Letters'],
  newTypes: ['Letter'],
  newFields: ['local:shelfMark'],
  itemSetIds: server.itemSetIds,
  existingSets: ['Photographs'],
  progress: { done: server.done, total: TOTAL },
  // The undo counts its own targets (items, files, sets...), not the run's items.
  ...(server.status === 'undoing' ? { undoProgress: { done: 4, total: 11 } } : {}),
});

const detailOf = (server: Server): Record<string, unknown> => ({
  id: ID,
  folder: 'BHC_006',
  status: server.status,
  undoable: server.status === 'done' || server.status === 'failed',
  created: '2026-10-19T09:00:00',
  finished: server.status === 'running' || server.status === 'draft' ? null : '2026-10-19T09:05:00',
  summary: server.status === 'draft' ? null : summary(server),
  plan,
  jobId: server.status === 'draft' ? null : 31,
  undoReport:
    server.status === 'undone'
      ? { items: 3, media: 6, itemSets: 1, templates: 1, fields: 0, kept: [{ kind: 'field', id: 41 }] }
      : null,
  died: server.died,
});

/** One read of the import: a running import moves one item on, an undo ends. */
const advance = (server: Server): void => {
  if (server.status === 'running') {
    server.done += 1;
    if (server.done === TOTAL) {
      server.status = 'done';
    }
  } else if (server.status === 'undoing') {
    server.status = 'undone';
  }
};

const conflict = (): Response => jsonResponse({ errors: { error: 'Conflict' } }, { status: 409 });

const stub = (server: Server, previewCheck: Record<string, unknown> = check) =>
  installFetch(
    route({
      [PATH]: (_url, init) => {
        if (init?.method !== 'PUT' && server.status !== 'draft') {
          const answer = detailOf(server);
          advance(server);
          return jsonResponse(answer);
        }
        return jsonResponse(detailOf(server));
      },
      [`${PATH}/preview`]: () => jsonResponse({ check: previewCheck }),
      [`${PATH}/run`]: () => {
        if (server.runAnswer === 409) {
          return conflict();
        }
        server.status = 'running';
        return jsonResponse({ jobId: 31 });
      },
      [`${PATH}/undo`]: () => {
        if (server.undoAnswer === 409) {
          return conflict();
        }
        server.status = 'undoing';
        return jsonResponse({ jobId: 32 });
      },
      [`${PATH}/left.csv`]: () => new Response(LEFT_CSV, { headers: { 'Content-Type': 'text/csv' } }),
      '/api/romeka/archive': () => jsonResponse({ enabled: true, path: '', entries: [] }),
    }),
  );

const LEFT_CSV = 'dcterms:identifier,#reason\nL_009,skipped\n';

const server = (patch: Partial<Server> = {}): Server => ({
  status: 'draft',
  done: 0,
  runAnswer: 200,
  undoAnswer: 200,
  itemSetIds: [5, 9],
  existing: 0,
  died: null,
  ...patch,
});

const step = (name: string) => <ImportPage param={String(ID)} rest={[String(ID), name]} />;

const tick = async (ms: number): Promise<void> => {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
};

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
  window.location.hash = '';
});

describe('Check step', () => {
  it('shows the dry run, the unmatched files, the problem choice and the open undo window', async () => {
    stub(server());
    render(step('check'));
    const table = await screen.findByRole('table', { name: 'What this import will do' });
    const row = (label: string): string => within(table).getByRole('rowheader', { name: label }).closest('tr')?.textContent ?? '';
    expect(row('Items to create')).toBe('Items to create3');
    expect(row('New item sets')).toBe('New item sets1Letters');
    expect(row('Existing item sets that get new items')).toBe('Existing item sets that get new items1Photographs');
    expect(row('New fields')).toBe('New fields1local:shelfMark');
    expect(row('Files that match no item, left out')).toBe('Files that match no item, left out12');
    expect(row('Items that already exist and are skipped')).toBe('Items that already exist and are skipped4');

    const sample = screen.getByRole('heading', { name: 'Files that match no item' }).nextElementSibling;
    expect(sample?.querySelectorAll('li')).toHaveLength(10);
    expect(screen.getByText('And 2 more.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Show the rows with problems in the grid' })).toHaveAttribute(
      'href',
      '#/import/7/grid/problems',
    );
    expect(
      screen.getByText(
        'Starting this import makes the last import (BHC_005, items: 40) permanent. Until then you can undo it.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import items: 3' })).toBeEnabled();
  });

  it('turns the button off with the reason the preview gives', async () => {
    stub(server(), { ...check, canRun: false, openUndo: null });
    render(step('check'));
    expect(await screen.findByRole('button', { name: 'Import items: 3' })).toBeDisabled();
    expect(screen.getByText('The server cannot run this plan yet.')).toBeInTheDocument();
    expect(screen.queryByText(/makes the last import/)).toBeNull();
  });

  it('says every item already exists when a folder is imported again', async () => {
    stub(server(), { ...check, create: 0, files: 0, withProblems: 0, skipped: 0, existing: 9, canRun: false });
    render(step('check'));
    expect(await screen.findByRole('button', { name: 'Import items: 0' })).toBeDisabled();
    expect(screen.getByText('Every item in this plan already exists. There is nothing new to import.')).toBeInTheDocument();
  });
});

describe('Run step', () => {
  it('saves the plan, posts the run, and reads the import every second until it is done', async () => {
    const state = server();
    const fetchMock = stub(state);
    const view = render(step('check'));
    const start = await screen.findByRole('button', { name: 'Import items: 3' });
    await waitFor(() => { expect(start).toBeEnabled(); });
    fireEvent.click(start);

    await waitFor(() => { expect(window.location.hash).toBe('#/import/7/run'); });
    const writes = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'PUT');
    expect(writes).toHaveLength(1);
    expect(requestBody(writes[0] ?? { url: new URL('http://x'), init: undefined })['plan']).toMatchObject({ problemRows: 'leave' });
    expect(requestsFor(fetchMock, `${PATH}/run`)).toHaveLength(1);

    view.rerender(step('run'));
    expect(await screen.findByText('Importing. Items done: 1 of 3.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Job #31 and its log' })).toHaveAttribute('href', '#/jobs/31');
    await tick(POLL_MS);
    expect(await screen.findByText('Importing. Items done: 2 of 3.')).toBeInTheDocument();
    await tick(POLL_MS);
    expect(await screen.findByText(/^Done\. Items: 3\. Files: 6\. Thumbnails: 6\. Rows left out: 2\.$/)).toBeInTheDocument();
    expect(screen.getByText('You can undo this import until the next import starts.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Download the left-out rows (CSV)' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open the item sets' })).toHaveAttribute('href', '#/item-sets');

    const reads = requestsFor(fetchMock, PATH).filter((request) => request.init?.method !== 'PUT').length;
    await tick(POLL_MS * 3);
    expect(requestsFor(fetchMock, PATH).filter((request) => request.init?.method !== 'PUT')).toHaveLength(reads);
  });

  it('undoes after the confirmation and shows what the undo kept', async () => {
    const fetchMock = stub(server({ status: 'done', done: TOTAL }));
    render(step('run'));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo this import' }));

    const panel = screen.getByRole('group', { name: 'Undo the import of BHC_006' });
    expect(within(panel).getByText('The items and their files, with the thumbnails. Items: 3. Files: 6.')).toBeInTheDocument();
    expect(within(panel).getByText('Their places in these existing item sets, which stay: Photographs.')).toBeInTheDocument();
    expect(within(panel).getByText('The new item sets, if nothing else was added to them: Letters.')).toBeInTheDocument();
    expect(requestsFor(fetchMock, `${PATH}/undo`)).toHaveLength(0);
    fireEvent.click(within(panel).getByRole('button', { name: 'Delete the imported items (3)' }));

    await waitFor(() => { expect(requestsFor(fetchMock, `${PATH}/undo`)).toHaveLength(1); });
    expect(await screen.findByText('Undoing the import. Done: 4 of 11.')).toBeInTheDocument();
    await tick(POLL_MS);
    expect(
      await screen.findByText(
        'Undone. Deleted items: 3. Files: 6. Item sets: 1. Item types: 1. Fields: 0.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('These stay, because something else uses them:')).toBeInTheDocument();
    expect(screen.getByText('Field #41')).toBeInTheDocument();
  });

  it('links the one item set the run filled, and saves the left-out rows the server lists', async () => {
    const createObjectURL = vi.fn<(blob: Blob) => string>(() => 'blob:left');
    Object.assign(URL, { createObjectURL, revokeObjectURL: vi.fn() });
    const fetchMock = stub(server({ status: 'done', done: TOTAL, itemSetIds: [5] }));
    render(step('run'));
    expect(await screen.findByRole('link', { name: 'Open the item set' })).toHaveAttribute('href', '#/item-sets/5');

    fireEvent.click(screen.getByRole('button', { name: 'Download the left-out rows (CSV)' }));
    await waitFor(() => { expect(createObjectURL).toHaveBeenCalledTimes(1); });
    expect(requestsFor(fetchMock, `${PATH}/left.csv`)).toHaveLength(1);
    expect(requestsFor(fetchMock, `${PATH}/preview`)).toHaveLength(0);
    expect(await createObjectURL.mock.calls[0]?.[0].text()).toBe(LEFT_CSV);
  });

  it('says an import whose job died stopped early and can be undone', async () => {
    stub(server({ status: 'failed', done: 1, died: '2026-10-19T09:03:00+00:00' }));
    render(step('run'));
    expect(
      await screen.findByText('The import stopped before it finished. You can undo what it made.', { exact: false }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo this import' })).toBeInTheDocument();
  });

  it('keeps the job log pointer for a run that failed on its own', async () => {
    stub(server({ status: 'failed', done: 1 }));
    render(step('run'));
    expect(
      await screen.findByText('The import stopped with an error. The job log says why.', { exact: false }),
    ).toBeInTheDocument();
  });

  it('counts the items that already existed and were skipped', async () => {
    stub(server({ status: 'done', done: TOTAL, existing: 4 }));
    render(step('run'));
    expect(await screen.findByText('4 items already existed and were skipped.')).toBeInTheDocument();
  });

  it('says why the server refused a run with 409', async () => {
    stub(server({ runAnswer: 409 }));
    render(step('check'));
    const start = await screen.findByRole('button', { name: 'Import items: 3' });
    await waitFor(() => { expect(start).toBeEnabled(); });
    fireEvent.click(start);
    expect(
      await screen.findByText('Another import is running or being undone. Wait until it ends, then start this one.'),
    ).toBeInTheDocument();
    expect(window.location.hash).toBe('');
  });

  it('says the undo window has closed when the server refuses an undo with 409', async () => {
    stub(server({ status: 'done', done: TOTAL, undoAnswer: 409 }));
    render(step('run'));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo this import' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete the imported items (3)' }));
    expect(
      await screen.findByText(
        'This import can no longer be undone. A later import started, so its undo window has closed.',
      ),
    ).toBeInTheDocument();
  });
});

describe('Undo banner', () => {
  const history = (undoable: boolean) => [
    {
      id: 6,
      folder: 'BHC_005',
      status: 'done',
      undoable,
      created: '2026-10-18T09:00:00',
      finished: '2026-10-18T09:05:00',
      summary: { ...summary(server({ done: 40 })), items: 40 },
    },
  ];

  const stubHome = (undoable: boolean) =>
    installFetch(
      route({
        '/api/romeka/imports': () => jsonResponse(history(undoable)),
        '/api/romeka/archive': () => jsonResponse({ enabled: true, path: '', entries: [] }),
      }),
    );

  it('names the import that can still be undone and opens the confirmation', async () => {
    stubHome(true);
    render(<ImportPage param={null} />);
    expect(
      await screen.findByText(
        /^Your last import \(BHC_005, items: 40\) can still be undone\. Starting a new import makes it permanent\./,
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Undo it' }));
    expect(screen.getByRole('group', { name: 'Undo the import of BHC_005' })).toBeInTheDocument();
  });

  it('stays away when no import holds the undo window', async () => {
    stubHome(false);
    render(<ImportPage param={null} />);
    expect(await screen.findByRole('heading', { name: 'Earlier imports' })).toBeInTheDocument();
    expect(screen.queryByText(/can still be undone/)).toBeNull();
  });

  it('reads the history once for the banner and the table', async () => {
    const fetchMock = stubHome(true);
    render(<ImportPage param={null} />);
    expect(await screen.findByRole('heading', { name: 'Earlier imports' })).toBeInTheDocument();
    expect(screen.getByText(/can still be undone/)).toBeInTheDocument();
    expect(requestsFor(fetchMock, '/api/romeka/imports')).toHaveLength(1);
  });
});
