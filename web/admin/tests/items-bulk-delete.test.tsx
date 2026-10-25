/**
 * The Items list's bulk delete, through the real typed client and parsers: a stubbed fetch plays
 * the bulk routes with the answers `crates/omeka-cli/tests/bulk_wire.rs` recorded.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { brandId } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { User } from '../src/api/types';
import { ItemsList } from '../src/pages/items/list';
import { loadFixture } from './loadFixture';

const OPTION_FIXTURES: Readonly<Record<string, string>> = {
  '/api/users': 'users-page.json',
  '/api/resource_classes': 'resource-classes-page.json',
  '/api/item_sets': 'item-sets-page.json',
  '/api/resource_templates': 'resource-templates-page.json',
  '/api/sites': 'sites-page.json',
  '/api/properties': 'properties-page.json',
};

const json = (body: unknown, status = 200, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json', ...headers } });

const asObject = (value: unknown): Record<string, unknown> => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('expected an object');
  }
  return Object.fromEntries(Object.entries(value));
};

const done = asObject(loadFixture('bulk/delete-done.json'));
const running = { ...done, status: 'running', counts: { ...asObject(done['counts']), pending: 1, done: 1 } };
const undone = loadFixture('bulk/delete-undone.json');

/** A run as the recent list answers it, without the live counts. */
const listed = (run: Record<string, unknown>, id: number): Record<string, unknown> =>
  Object.fromEntries(Object.entries({ ...run, id }).filter(([key]) => key !== 'counts' && key !== 'failures'));

interface Sent {
  readonly method: string;
  readonly path: string;
  readonly body: unknown;
}

/** The list and option routes, and the bulk routes answering each run read with the next of `reads`. */
const installServer = (reads: readonly unknown[], runs: readonly unknown[] = []): Sent[] => {
  const sent: Sent[] = [];
  let read = 0;
  const handle = (method: string, path: string): Response => {
    if (path === '/api/items') {
      return json(loadFixture('items-page.json'), 200, { 'Omeka-S-Total-Results': '60' });
    }
    const fixture = OPTION_FIXTURES[path];
    if (fixture !== undefined) {
      return json(loadFixture(fixture));
    }
    if (path === '/api/romeka/bulk') {
      return method === 'POST' ? json(loadFixture('bulk/delete-preview.json'), 201) : json(runs);
    }
    if (path === '/api/romeka/bulk/trash/purge') {
      return json(loadFixture('bulk/trash-purged.json'));
    }
    if (path === '/api/romeka/bulk/1/run' || path === '/api/romeka/bulk/1/undo') {
      return json({ jobId: 1 });
    }
    if (path === '/api/romeka/bulk/1/undo/preview') {
      return json(loadFixture('bulk/delete-undo-preview.json'));
    }
    if (path === '/api/romeka/bulk/1') {
      const answer = reads[Math.min(read, reads.length - 1)];
      read += 1;
      return json(answer);
    }
    return json({ errors: { error: `Unexpected request to ${path}` } }, 404);
  };
  vi.stubGlobal(
    'fetch',
    vi.fn<typeof fetch>((input, init) => {
      const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url, window.location.href);
      const method = init?.method ?? 'GET';
      const body: unknown = typeof init?.body === 'string' ? JSON.parse(init.body) : null;
      sent.push({ method, path: url.pathname, body });
      return Promise.resolve(handle(method, url.pathname));
    }),
  );
  return sent;
};

const person = (role: string): User => ({
  id: brandId<'UserId'>(1),
  apiUrl: 'http://localhost/api/users/1',
  types: ['o:User'],
  name: 'Person',
  email: 'person@example.com',
  created: '2026-10-16T09:00:00+00:00',
  role,
  isActive: true,
  localeSetting: null,
  lessons: { hidden: false, seen: [] },
  adminLocale: null,
});

const openList = async (): Promise<void> => {
  window.history.replaceState(null, '', '/#/items');
  render(<ItemsList />);
  await screen.findByRole('link', { name: 'Fixture Item One' });
};

const itemReads = (sent: readonly Sent[]): number => sent.filter((entry) => entry.path === '/api/items').length;

describe('items bulk delete', () => {
  beforeEach(() => {
    window.localStorage.clear();
    setSession(person('editor'));
  });

  afterEach(() => {
    setSession(null);
  });

  it('previews what goes, needs the count typed, moves the items to the trash, and undoes it', async () => {
    const sent = installServer([running, done, undone]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Delete items…' }));
    const panel = await screen.findByRole('region', { name: 'Delete items' });
    expect(await within(panel).findByRole('radio', { name: 'All 60 items' })).toBeChecked();

    fireEvent.click(within(panel).getByRole('button', { name: 'Preview' }));
    expect(await within(panel).findByText('2 items go to the trash.')).toBeInTheDocument();
    const preview = sent.find((entry) => entry.method === 'POST' && entry.path === '/api/romeka/bulk');
    expect(preview?.body).toEqual({ query: {}, change: { kind: 'delete' } });
    expect(within(panel).getByRole('link', { name: 'Letter to Anna' })).toHaveAttribute('href', '#/items/1');
    expect(
      within(panel).getByText('They stay in the trash for 30 days. Until then, Undo puts them back with their media and files.'),
    ).toBeInTheDocument();

    const move = within(panel).getByRole('button', { name: 'Move 2 items to the trash' });
    const typed = within(panel).getByRole('textbox', { name: 'To confirm, type the number of items, 2' });
    expect(move).toBeDisabled();
    fireEvent.change(typed, { target: { value: '3' } });
    expect(move).toBeDisabled();
    fireEvent.submit(move);
    expect(sent.some((entry) => entry.path === '/api/romeka/bulk/1/run')).toBe(false);
    fireEvent.change(typed, { target: { value: '2' } });
    expect(move).toBeEnabled();

    const before = itemReads(sent);
    fireEvent.click(move);
    expect(await within(panel).findByRole('progressbar', { name: 'Moving items to the trash: 1 of 2' })).toBeInTheDocument();
    expect(await within(panel).findByText('2 items moved to the trash.', {}, { timeout: 4000 })).toBeInTheDocument();
    await waitFor(() => {
      expect(itemReads(sent)).toBe(before + 1);
    });

    fireEvent.click(within(panel).getByRole('button', { name: 'Undo' }));
    expect(await within(panel).findByText('Undo puts 2 items back as they were before this change.')).toBeInTheDocument();
    fireEvent.click(within(panel).getByRole('button', { name: 'Undo the change' }));
    expect(await within(panel).findByText('Undone. 2 items are back as they were.')).toBeInTheDocument();
    expect(sent.find((entry) => entry.path === '/api/romeka/bulk/1/undo')?.body).toEqual({ force: false });
  });

  it('offers an author her ticked items only', async () => {
    setSession(person('author'));
    installServer([]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Delete items…' }));
    const panel = await screen.findByRole('region', { name: 'Delete items' });
    expect(await within(panel).findByRole('radio', { name: 'All 60 items' })).toBeDisabled();
    expect(within(panel).getByText('Your role can delete ticked items only. Tick the items to delete.')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Preview' })).toBeDisabled();
  });

  it('shows deletes in the recent list, no Undo once their trash is emptied, and lets an administrator empty it', async () => {
    setSession(person('global_admin'));
    const purged = asObject(loadFixture('bulk/delete-purged.json'));
    const sent = installServer([], [listed(done, 1), listed(purged, 2)]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Recent bulk changes' }));
    const panel = await screen.findByRole('region', { name: 'Recent bulk changes' });
    const rows = await within(panel).findAllByRole('row', { name: /Move to the trash/ });
    expect(rows).toHaveLength(2);
    const [inTrash, emptied] = rows;
    if (inTrash === undefined || emptied === undefined) {
      throw new Error('two delete rows');
    }
    expect(within(inTrash).getByRole('button', { name: 'Undo' })).toBeInTheDocument();
    expect(within(emptied).getByText('Trash emptied')).toBeInTheDocument();
    expect(within(emptied).queryByRole('button', { name: 'Undo' })).not.toBeInTheDocument();

    fireEvent.click(within(panel).getByRole('button', { name: 'Empty the trash…' }));
    const ask = within(panel).getByRole('group', { name: 'Empty the trash' });
    expect(within(ask).getByText(/deleted for good/)).toBeInTheDocument();
    fireEvent.click(within(ask).getByRole('button', { name: 'Empty the trash' }));
    expect(await within(panel).findByText('The trash is empty. 1 item was deleted for good.')).toBeInTheDocument();
    expect(sent.some((entry) => entry.method === 'POST' && entry.path === '/api/romeka/bulk/trash/purge')).toBe(true);
  });

  it('shows an editor no way to empty the trash', async () => {
    installServer([], [listed(done, 1)]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Recent bulk changes' }));
    const panel = await screen.findByRole('region', { name: 'Recent bulk changes' });
    await within(panel).findByRole('row', { name: /Move to the trash/ });
    expect(within(panel).queryByRole('button', { name: 'Empty the trash…' })).not.toBeInTheDocument();
  });

  it('shows a researcher no way to delete items in bulk', async () => {
    setSession(person('researcher'));
    installServer([]);
    await openList();
    expect(screen.queryByRole('button', { name: 'Delete items…' })).not.toBeInTheDocument();
  });
});
