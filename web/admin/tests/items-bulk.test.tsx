/**
 * The Items list's bulk change panel, through the real typed client and parsers: a stubbed fetch
 * plays the bulk routes with the answers `crates/omeka-cli/tests/bulk_wire.rs` recorded.
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
};

const json = (body: unknown, status = 200, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json', ...headers } });

const asObject = (value: unknown): Record<string, unknown> => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('expected an object');
  }
  return Object.fromEntries(Object.entries(value));
};

const done = asObject(loadFixture('bulk/run-done.json'));
const running = { ...done, status: 'running', counts: { ...asObject(done['counts']), pending: 2, done: 1 } };

interface Sent {
  readonly method: string;
  readonly path: string;
  readonly body: unknown;
}

/**
 * The list and option routes from the fixtures, and the bulk routes from `bulk`, which answers
 * each run read with the next of `reads` (the last one again once they run out).
 */
const installServer = (reads: readonly unknown[]): Sent[] => {
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
      return method === 'POST' ? json(loadFixture('bulk/preview.json'), 201) : json(loadFixture('bulk/runs.json'));
    }
    if (path === '/api/romeka/bulk/1/run' || path === '/api/romeka/bulk/1/undo') {
      return json({ jobId: 1 });
    }
    if (path === '/api/romeka/bulk/1/undo/preview') {
      return json(loadFixture('bulk/undo-preview.json'));
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

const editor: User = {
  id: brandId<'UserId'>(1),
  apiUrl: 'http://localhost/api/users/1',
  types: ['o:User'],
  name: 'Editor',
  email: 'editor@example.com',
  created: '2026-10-16T09:00:00+00:00',
  role: 'editor',
  isActive: true,
  localeSetting: null,
  lessons: { hidden: false, seen: [] },
  adminLocale: null,
};

const openList = async (): Promise<void> => {
  window.history.replaceState(null, '', '/#/items');
  render(<ItemsList />);
  await screen.findByRole('link', { name: 'Fixture Item One' });
};

const itemReads = (sent: readonly Sent[]): number => sent.filter((entry) => entry.path === '/api/items').length;

describe('items bulk change', () => {
  beforeEach(() => {
    window.localStorage.clear();
    setSession(editor);
  });

  afterEach(() => {
    setSession(null);
  });

  it('previews, applies, shows progress and the end, and reads the list again', async () => {
    const sent = installServer([running, done]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Change items…' }));
    const panel = await screen.findByRole('region', { name: 'Change items' });
    expect(within(panel).getByRole('radio', { name: 'All 60 items' })).toBeChecked();

    fireEvent.change(within(panel).getByRole('combobox', { name: 'Visibility' }), { target: { value: 'private' } });
    fireEvent.click(within(panel).getByRole('button', { name: 'Preview' }));

    expect(await within(panel).findByText('3 items will change.')).toBeInTheDocument();
    const preview = sent.find((entry) => entry.method === 'POST' && entry.path === '/api/romeka/bulk');
    expect(preview?.body).toEqual({ query: {}, change: { kind: 'edit', isPublic: false } });
    const anna = within(panel).getByRole('row', { name: /Letter to Anna/ });
    const [was, will] = within(anna).getAllByRole('cell');
    expect(was).toHaveTextContent('Public');
    expect(will).toHaveTextContent('Private');

    const before = itemReads(sent);
    fireEvent.click(within(panel).getByRole('button', { name: 'Apply' }));
    const progress = await within(panel).findByRole('progressbar', { name: 'Changing items: 1 of 3' });
    expect(progress).toHaveAttribute('value', '1');
    expect(within(panel).getByText('You can leave this page; the change keeps running.')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Stop' })).toBeInTheDocument();

    expect(await within(panel).findByText('2 items changed.', {}, { timeout: 4000 })).toBeInTheDocument();
    expect(within(panel).getByText(/This item is no longer in Omeka\./)).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Undo' })).toBeInTheDocument();
    await waitFor(() => {
      expect(itemReads(sent)).toBe(before + 1);
    });
  });

  it('undoes a recent change, keeping an item edited since unless the person ticks it', async () => {
    const sent = installServer([loadFixture('bulk/run-partly-undone.json')]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Recent bulk changes' }));
    const panel = await screen.findByRole('region', { name: 'Recent bulk changes' });
    const row = await within(panel).findByRole('row', { name: /Private/ });
    expect(within(row).getByText('Done')).toBeInTheDocument();
    expect(within(row).getByText('2 of 3')).toBeInTheDocument();

    fireEvent.click(within(row).getByRole('button', { name: 'Undo' }));
    expect(await within(panel).findByText('Undo puts 1 item back as it was before this change.')).toBeInTheDocument();
    expect(within(panel).getByText('1 item was edited after this change. Undo leaves it as it is:')).toBeInTheDocument();
    expect(within(panel).getByRole('link', { name: 'Item 3' })).toHaveAttribute('href', '#/items/3');
    const force = within(panel).getByRole('checkbox', { name: 'Restore these too' });
    expect(force).not.toBeChecked();

    fireEvent.click(within(panel).getByRole('button', { name: 'Undo the change' }));
    expect(await within(panel).findByText(/Back as they were: 1\./)).toBeInTheDocument();
    const undo = sent.find((entry) => entry.path === '/api/romeka/bulk/1/undo');
    expect(undo?.body).toEqual({ force: false });
  });

  it('names the items an undo left because the person who asked may not edit them', async () => {
    const partly = asObject(loadFixture('bulk/run-partly-undone.json'));
    const message = 'The person who asked for the undo may not edit this item, so the undo left it as it is.';
    installServer([{ ...partly, failures: [{ item: 5, reason: 'undoDenied', message }] }]);
    await openList();
    fireEvent.click(screen.getByRole('button', { name: 'Recent bulk changes' }));
    const panel = await screen.findByRole('region', { name: 'Recent bulk changes' });
    const row = await within(panel).findByRole('row', { name: /Private/ });
    fireEvent.click(within(row).getByRole('button', { name: 'Undo' }));
    fireEvent.click(await within(panel).findByRole('button', { name: 'Undo the change' }));

    expect(await within(panel).findByText(/Back as they were: 1\./)).toBeInTheDocument();
    expect(within(panel).getByText('Not put back:')).toBeInTheDocument();
    expect(within(panel).getByRole('link', { name: 'Item 5' })).toHaveAttribute('href', '#/items/5');
    expect(within(panel).getByText(/may not edit this item, so the undo left it as it is\./)).toBeInTheDocument();
  });

  it('a researcher sees no way to change items in bulk', async () => {
    setSession({ ...editor, role: 'researcher' });
    installServer([]);
    await openList();
    expect(screen.queryByRole('button', { name: 'Change items…' })).not.toBeInTheDocument();
  });
});
