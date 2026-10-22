/**
 * The Import start page (`#/import`): a folder under an archive root, or
 * files from this computer, starts a draft, and the past imports page through the history 50 at a time, each
 * with its status in plain words, a copy, and "Try again" when its run may go
 * again. Every answer is a fixture the server's routes wrote.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';
import type { Handler } from './stub';

const fixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const EMPTY_HISTORY = { total: 0, page: 1, perPage: 50, items: [] };

const archive: Handler = (url) =>
  jsonResponse(fixture(url.searchParams.get('path') === 'Artifacts' ? 'archive-folder.json' : 'archive.json'));

/** `total` history rows, newest first, cut from the fixture's run_failed row. */
const longHistory = (total: number): Handler => {
  const row = fixture('history.json')['items'];
  const template = Array.isArray(row) ? asRecord(row[2], []) : {};
  return (url) => {
    const page = Number(url.searchParams.get('page') ?? '1');
    const perPage = Number(url.searchParams.get('perPage') ?? '25');
    const ids = Array.from({ length: total }, (_, index) => total - index).slice((page - 1) * perPage, page * perPage);
    return jsonResponse({ total, page, perPage, items: ids.map((id) => ({ ...template, id })) });
  };
};

const pastTable = (): HTMLElement => {
  const region = screen.getByRole('region', { name: 'Past imports' });
  return within(region).getByRole('table');
};

afterEach(() => {
  window.location.hash = '';
});

describe('Where are your files', () => {
  it('opens a folder of the root and starts a draft over it, on Structure', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/archive': archive,
        '/api/romeka/imports': (_url, init) =>
          init?.method === 'POST' ? jsonResponse(fixture('import-draft.json')) : jsonResponse(EMPTY_HISTORY),
      }),
    );
    render(<ImportPage param={null} />);
    expect(await screen.findByRole('button', { name: 'Use everything in archive' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Artifacts' }));
    const use = await screen.findByRole('button', { name: 'Use the folder Artifacts' });
    expect(screen.getByText('This folder holds 8 files, not counting the folders inside it.')).toBeInTheDocument();
    expect(screen.getByText('There are no folders inside this one.')).toBeInTheDocument();
    expect(first(requestsFor(fetchMock, '/api/romeka/archive').slice(1)).url.search).toBe('?root=archive&path=Artifacts');
    fireEvent.click(use);
    await waitFor(() => {
      expect(window.location.hash).toBe('#/import/1/simple/structure');
    });
    const post = first(requestsFor(fetchMock, '/api/romeka/imports').filter((request) => request.init?.method === 'POST'));
    expect(requestBody(post)).toEqual({ sources: [{ id: 's1', kind: 'folder', root: 'archive', path: 'Artifacts' }] });
  });

  it('starts a draft over files from this computer, on Your files', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/archive': archive,
        '/api/romeka/imports': (_url, init) =>
          init?.method === 'POST' ? jsonResponse(fixture('import-draft.json')) : jsonResponse(EMPTY_HISTORY),
      }),
    );
    render(<ImportPage param={null} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Start with files from this computer' }));
    await waitFor(() => {
      expect(window.location.hash).toBe('#/import/1/simple/files');
    });
    const post = first(requestsFor(fetchMock, '/api/romeka/imports').filter((request) => request.init?.method === 'POST'));
    expect(requestBody(post)).toEqual({ sources: [{ id: 's1', kind: 'staging' }] });
  });

  it('switches between archive roots', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/archive': (url) =>
          jsonResponse({ ...fixture('archive.json'), roots: ['archive', 'scans'], root: url.searchParams.get('root') ?? 'archive' }),
        '/api/romeka/imports': () => jsonResponse(EMPTY_HISTORY),
      }),
    );
    render(<ImportPage param={null} />);
    const roots = await screen.findByRole('group', { name: 'Folders on the server' });
    expect(within(roots).getByRole('button', { name: 'archive' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(within(roots).getByRole('button', { name: 'scans' }));
    expect(await screen.findByRole('button', { name: 'Use everything in scans' })).toBeInTheDocument();
    expect(requestsFor(fetchMock, '/api/romeka/archive').map((request) => request.url.search)).toEqual([
      '?path=',
      '?root=scans&path=',
    ]);
  });

  it('says how to set up a folder when the server has no archive root', async () => {
    installFetch(
      route({
        '/api/romeka/archive': () => jsonResponse({ roots: [], root: null, path: '', entries: [] }),
        '/api/romeka/imports': () => jsonResponse(EMPTY_HISTORY),
      }),
    );
    render(<ImportPage param={null} />);
    expect(await screen.findByText(/No folder on the server is set up for importing/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Use / })).not.toBeInTheDocument();
    expect(await screen.findByText('No past imports yet. Choose a folder above to start one.')).toBeInTheDocument();
  });
});

describe('Past imports', () => {
  it('shows each status in plain words, a copy for each, and Try again for the stopped run', async () => {
    installFetch(route({ '/api/romeka/archive': archive, '/api/romeka/imports': () => jsonResponse(fixture('history.json')) }));
    render(<ImportPage param={null} />);
    await screen.findByRole('region', { name: 'Past imports' });
    const rows = within(pastTable()).getAllByRole('row').slice(1);
    expect(rows.map((row) => within(row).getAllByRole('cell')[1]?.textContent)).toEqual([
      'Undoing',
      'Not added yet',
      'Stopped before the end',
    ]);
    expect(rows.map((row) => within(row).queryByRole('link', { name: 'Try again' })?.getAttribute('href') ?? null)).toEqual([
      null,
      null,
      '#/import/1/simple/add',
    ]);
    expect(rows.every((row) => within(row).queryByRole('button', { name: 'Make a copy' }) !== null)).toBe(true);
    expect(within(first(rows)).getByRole('link', { name: 'All of archive' })).toHaveAttribute('href', '#/import/6');
  });

  it('says in plain words that an undo kept some items', async () => {
    const kept = fixture('undo-report-kept.json');
    installFetch(
      route({
        '/api/romeka/archive': archive,
        '/api/romeka/imports': () => jsonResponse({ total: 1, page: 1, perPage: 25, items: [kept] }),
      }),
    );
    render(<ImportPage param={null} />);
    await screen.findByRole('region', { name: 'Past imports' });
    const row = within(pastTable()).getAllByRole('row')[1] ?? document.body;
    expect(within(row).getAllByRole('cell')[1]?.textContent).toBe('Partly undone: some items were kept');
  });

  it('copies an import into a new draft and opens it', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/archive': archive,
        '/api/romeka/imports': (_url, init) =>
          init?.method === 'POST' ? jsonResponse(fixture('import-copied.json')) : jsonResponse(fixture('history.json')),
      }),
    );
    render(<ImportPage param={null} />);
    await screen.findByRole('region', { name: 'Past imports' });
    const stopped = within(pastTable()).getAllByRole('row')[3];
    fireEvent.click(within(stopped ?? document.body).getByRole('button', { name: 'Make a copy' }));
    await waitFor(() => {
      expect(window.location.hash).toBe('#/import/5/simple/structure');
    });
    const post = first(requestsFor(fetchMock, '/api/romeka/imports').filter((request) => request.init?.method === 'POST'));
    expect(requestBody(post)).toEqual({ copyFrom: 1 });
  });

  it('shows a refused copy in the row it came from', async () => {
    installFetch(
      route({
        '/api/romeka/archive': archive,
        '/api/romeka/imports': (_url, init) =>
          init?.method === 'POST'
            ? jsonResponse(fixture('conflict-v1-plan.json'), { status: 409 })
            : jsonResponse(fixture('history.json')),
      }),
    );
    render(<ImportPage param={null} />);
    await screen.findByRole('region', { name: 'Past imports' });
    const stopped = within(pastTable()).getAllByRole('row')[3] ?? document.body;
    fireEvent.click(within(stopped).getByRole('button', { name: 'Make a copy' }));
    expect(await within(stopped).findByRole('alert')).toHaveTextContent('made by the old importer');
    expect(window.location.hash).toBe('');
  });

  it('pages the history 50 rows at a time', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/archive': archive, '/api/romeka/imports': longHistory(120) }));
    render(<ImportPage param={null} />);
    expect(await screen.findByText(/Page 1 of 3 \(120 imports\)/)).toBeInTheDocument();
    expect(within(pastTable()).getAllByRole('row')).toHaveLength(51);
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByText(/Page 2 of 3/)).toBeInTheDocument();
    const rows = within(pastTable()).getAllByRole('row').slice(1);
    expect(rows).toHaveLength(50);
    expect(within(first(rows)).getByRole('link', { name: 'All of archive' })).toHaveAttribute('href', '#/import/70');
    expect(requestsFor(fetchMock, '/api/romeka/imports').map((request) => request.url.search)).toEqual([
      '?page=1&perPage=50',
      '?page=2&perPage=50',
    ]);
  });
});
