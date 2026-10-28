/**
 * The item page's delete moves the item to the trash: the confirm states the media, the records
 * that link to it by field and the trash days, the move is a bulk delete of that item alone, and
 * the Items screen then offers its Undo.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { brandId, isRecord } from '../src/api/parse';
import { TrashItem } from '../src/pages/items/trash-item';
import { TrashedNotice } from '../src/pages/items/trashed-notice';
import { peekTrashed, forgetTrashed } from '../src/pages/items/trashed';
import type { Trashed } from '../src/pages/items/trashed';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';

const ITEM = brandId<'ItemId'>(1);
const RUN = brandId<'BulkId'>(7);

const preview = (overrides: Record<string, unknown> = {}): Record<string, unknown> => ({
  id: 7,
  status: 'previewed',
  kind: 'delete',
  total: 1,
  denied: 0,
  media: 2,
  bytes: 2048,
  trashDays: 30,
  samples: [{ id: 1, title: 'Person A' }],
  ...overrides,
});

const CREATOR_LINKS = { links: [{ kind: 'items', property_id: 2, term: 'dcterms:creator', label: 'Creator', count: 2 }] };

const run = (overrides: Record<string, unknown>): Record<string, unknown> => {
  const done = loadFixture('bulk/delete-done.json');
  return {
    ...(isRecord(done) ? done : {}),
    id: 7,
    query: { id: [1] },
    total: 1,
    done: 1,
    counts: { pending: 0, done: 1, unchanged: 0, failed: 0, restored: 0, kept: 0 },
    ...overrides,
  };
};

/** Answers each read of the run with the next state, the last one from then on. */
const runReads = (states: readonly Record<string, unknown>[]) => {
  let read = 0;
  return () => jsonResponse(states[Math.min(read++, states.length - 1)]);
};

const renderTrash = (onCancel = (): void => undefined) =>
  render(<TrashItem item={ITEM} name="Person A" onCancel={onCancel} />);

afterEach(() => {
  const left = peekTrashed();
  if (left !== null) {
    forgetTrashed(left);
  }
  window.location.hash = '';
});

describe('item trash confirm', () => {
  it('states the media, the links by field and the trash days, then moves the item alone to the trash', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/bulk': () => jsonResponse(preview(), { status: 201 }),
        '/api/romeka/links/1': () => jsonResponse(CREATOR_LINKS),
        '/api/romeka/bulk/7/run': () => jsonResponse({ jobId: 3 }),
        '/api/romeka/bulk/7': runReads([run({ status: 'running', done: 0, counts: { pending: 1, done: 0, unchanged: 0, failed: 0, restored: 0, kept: 0 } }), run({ status: 'done' })]),
      }),
    );
    renderTrash();

    expect(screen.getByRole('status')).toHaveTextContent('Checking what goes with this item…');
    expect(await screen.findByText('Move Person A to the trash?')).toBeInTheDocument();
    expect(screen.getByText('2 media files go too: 2 kB in all.')).toBeInTheDocument();
    expect(screen.getByText('Other records link to it:')).toBeInTheDocument();
    expect(screen.getByRole('listitem')).toHaveTextContent('Creator · 2 items');
    expect(screen.getByText('Their links come back if you restore it.')).toBeInTheDocument();
    expect(screen.getByText(/The trash keeps it for 30 days\./)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Move to the trash' }));
    await screen.findByRole('button', { name: 'Moving to the trash…' });
    await expect.poll(() => window.location.hash, { timeout: 3000 }).toBe('#/items');

    expect(requestBody(first(requestsFor(fetchMock, '/api/romeka/bulk')))).toEqual({
      query: { id: [1] },
      change: { kind: 'delete' },
    });
    expect(peekTrashed()).toEqual({ run: RUN, item: ITEM, name: 'Person A' });
  });

  it('says nothing links to it when nothing does', async () => {
    installFetch(
      route({
        '/api/romeka/bulk': () => jsonResponse(preview({ media: 0, bytes: 0 }), { status: 201 }),
        '/api/romeka/links/1': () => jsonResponse({ links: [] }),
      }),
    );
    renderTrash();
    expect(await screen.findByText('Nothing links to it.')).toBeInTheDocument();
    expect(screen.queryByText(/media file/)).not.toBeInTheDocument();
  });

  it('keeps Move off when it cannot check what the delete takes, and checks again on request', async () => {
    let fail = true;
    installFetch(
      route({
        '/api/romeka/bulk': () =>
          fail ? jsonResponse({ errors: { error: 'The database is busy.' } }, { status: 500 }) : jsonResponse(preview(), { status: 201 }),
        '/api/romeka/links/1': () => jsonResponse(CREATOR_LINKS),
      }),
    );
    renderTrash();
    expect(await screen.findByText('Cannot check what goes with this item, so it cannot move to the trash now.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Move to the trash' })).not.toBeInTheDocument();
    fail = false;
    fireEvent.click(screen.getByRole('button', { name: 'Check again' }));
    expect(await screen.findByRole('button', { name: 'Move to the trash' })).toBeEnabled();
  });

  it('says so when the caller may not delete the item', async () => {
    installFetch(
      route({
        '/api/romeka/bulk': () => jsonResponse(preview({ denied: 1, media: 0, samples: [] }), { status: 201 }),
        '/api/romeka/links/1': () => jsonResponse(CREATOR_LINKS),
      }),
    );
    renderTrash();
    expect(await screen.findByText('You may not delete this item.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Move to the trash' })).not.toBeInTheDocument();
  });

  it('says the item is not available when the preview selects nothing, as for one the caller may not see', async () => {
    installFetch(
      route({
        '/api/romeka/bulk': () => jsonResponse(preview({ total: 0, media: 0, samples: [] }), { status: 201 }),
        '/api/romeka/links/1': () => jsonResponse({ links: [] }),
      }),
    );
    renderTrash();
    expect(await screen.findByText('This record is not available.')).toBeInTheDocument();
    expect(screen.queryByText('Move Person A to the trash?')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Move to the trash' })).not.toBeInTheDocument();
  });

  it('leaves the item in place and says why when the run fails', async () => {
    installFetch(
      route({
        '/api/romeka/bulk': () => jsonResponse(preview(), { status: 201 }),
        '/api/romeka/links/1': () => jsonResponse(CREATOR_LINKS),
        '/api/romeka/bulk/7/run': () => jsonResponse({ jobId: 3 }),
        '/api/romeka/bulk/7': () =>
          jsonResponse(
            run({
              status: 'done',
              done: 0,
              failed: 1,
              counts: { pending: 0, done: 0, unchanged: 0, failed: 1, restored: 0, kept: 0 },
              failures: [{ item: 1, reason: 'deleteDenied', message: 'You may not delete this item.' }],
            }),
          ),
      }),
    );
    renderTrash();
    fireEvent.click(await screen.findByRole('button', { name: 'Move to the trash' }));
    expect(await screen.findByText('Person A was not moved to the trash. It stays as it is.')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('You may not delete this item.');
    expect(window.location.hash).toBe('');
    expect(peekTrashed()).toBeNull();
  });
});

describe('trashed notice', () => {
  const trashed: Trashed = { run: RUN, item: ITEM, name: 'Person A' };

  it('undoes the delete and links to the item that is back', async () => {
    let restored = 0;
    const fetchMock = installFetch(
      route({
        '/api/romeka/bulk/7/undo': () => jsonResponse({ jobId: 4 }),
        '/api/romeka/bulk/7': runReads([
          run({ status: 'undoing' }),
          run({ status: 'undone', counts: { pending: 0, done: 0, unchanged: 0, failed: 0, restored: 1, kept: 0 } }),
        ]),
      }),
    );
    render(
      <TrashedNotice
        trashed={trashed}
        onRestored={() => {
          restored += 1;
        }}
      />,
    );
    expect(screen.getByText('Deleted Person A.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(await screen.findByRole('link', { name: 'Person A is back.' }, { timeout: 3000 })).toHaveAttribute('href', '#/items/1');
    expect(restored).toBe(1);
    expect(requestBody(first(requestsFor(fetchMock, '/api/romeka/bulk/7/undo')))).toEqual({ force: false });
  });

  it('states the links an undo left out', async () => {
    installFetch(
      route({
        '/api/romeka/bulk/7/undo': () => jsonResponse({ jobId: 4 }),
        '/api/romeka/bulk/7': () =>
          jsonResponse(
            run({
              status: 'undone',
              counts: { pending: 0, done: 0, unchanged: 0, failed: 0, restored: 1, kept: 0 },
              failed: 1,
              failures: [
                {
                  item: 1,
                  reason: 'linksLeft',
                  message: 'The item is back, but links to it from items edited since the delete were left out.',
                },
              ],
            }),
          ),
      }),
    );
    render(<TrashedNotice trashed={trashed} onRestored={() => undefined} />);
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(await screen.findByRole('link', { name: 'Person A is back.' }, { timeout: 3000 })).toBeInTheDocument();
    expect(
      screen.getByText('The item is back, but links to it from items edited since the delete were left out.'),
    ).toBeInTheDocument();
  });

  it('says the item stays in the trash when the undo cannot put it back', async () => {
    installFetch(
      route({
        '/api/romeka/bulk/7/undo': () => jsonResponse({ jobId: 4 }),
        '/api/romeka/bulk/7': () =>
          jsonResponse(
            run({
              status: 'partly_undone',
              counts: { pending: 0, done: 1, unchanged: 0, failed: 0, restored: 0, kept: 0 },
              failed: 1,
              failures: [{ item: 1, reason: 'idTaken', message: 'Another record took the item id.' }],
            }),
          ),
      }),
    );
    render(<TrashedNotice trashed={trashed} onRestored={() => undefined} />);
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(await screen.findByText('Person A stays in the trash.', {}, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.getByText('Another record took the item id.')).toBeInTheDocument();
  });
});
