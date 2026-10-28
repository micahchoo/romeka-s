/**
 * The Items list's CSV download links: every match with the screen's filters and order, and the
 * ticked items alone. The file itself is `GET /api/romeka/items.csv` (`crates/omeka-server/tests/items_csv.rs`).
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { brandId } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { User } from '../src/api/types';
import { ItemsList } from '../src/pages/items/list';
import { loadFixture } from './loadFixture';

const FIXTURES: Readonly<Record<string, string>> = {
  '/api/users': 'users-page.json',
  '/api/resource_classes': 'resource-classes-page.json',
  '/api/item_sets': 'item-sets-page.json',
  '/api/resource_templates': 'resource-templates-page.json',
  '/api/sites': 'sites-page.json',
  '/api/properties': 'properties-page.json',
};

const json = (body: unknown, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status: 200, headers: { 'Content-Type': 'application/json', ...headers } });

const installServer = (total: number): void => {
  vi.stubGlobal(
    'fetch',
    vi.fn<typeof fetch>((input) => {
      const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url, window.location.href);
      if (url.pathname === '/api/items') {
        return Promise.resolve(
          json(total === 0 ? [] : loadFixture('items-page.json'), { 'Omeka-S-Total-Results': String(total) }),
        );
      }
      const fixture = FIXTURES[url.pathname];
      return Promise.resolve(fixture === undefined ? json([]) : json(loadFixture(fixture)));
    }),
  );
};

const researcher: User = {
  id: brandId<'UserId'>(4),
  apiUrl: 'http://localhost/api/users/4',
  types: ['o:User'],
  name: 'Researcher',
  email: 'researcher@example.com',
  created: '2026-10-16T09:00:00+00:00',
  role: 'researcher',
  isActive: true,
  localeSetting: null,
  lessons: { hidden: false, seen: [] },
  adminLocale: null,
};

const hrefOf = (name: string): URL => {
  const link = screen.getByRole('link', { name });
  return new URL(link.getAttribute('href') ?? '', window.location.href);
};

describe('items CSV download', () => {
  beforeEach(() => {
    window.localStorage.clear();
    setSession(researcher);
  });

  afterEach(() => {
    setSession(null);
  });

  it('downloads every match with the filters and order on screen, never one page', async () => {
    installServer(60);
    window.history.replaceState(null, '', '/#/items?set=4&visibility=private&sort=title&page=2');
    render(<ItemsList />);
    await screen.findByRole('link', { name: 'Fixture Item One' });

    const link = screen.getByRole('link', { name: 'Download 60 items as CSV' });
    expect(link).toHaveAttribute('download');
    const href = hrefOf('Download 60 items as CSV');
    expect(href.pathname).toBe('/api/romeka/items.csv');
    expect(Object.fromEntries(href.searchParams)).toEqual({
      sort_by: 'title',
      sort_order: 'asc',
      item_set_id: '4',
      is_public: '0',
    });
  });

  it('offers the ticked items alone once a row is ticked', async () => {
    installServer(60);
    window.history.replaceState(null, '', '/#/items?q=map');
    render(<ItemsList />);
    await screen.findByRole('link', { name: 'Fixture Item One' });
    expect(screen.queryByRole('link', { name: /ticked/ })).toBeNull();

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Fixture Item One' }));
    expect(Object.fromEntries(hrefOf('Download the ticked item as CSV').searchParams)).toEqual({
      sort_by: 'id',
      sort_order: 'desc',
      id: '12',
    });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Fixture Item Two' }));
    expect(hrefOf('Download the 2 ticked items as CSV').searchParams.get('id')).toBe('12,9');
    expect(screen.getByRole('link', { name: 'Download 60 items as CSV' })).toBeInTheDocument();
  });

  it('says one item in the singular and offers nothing for an empty list', async () => {
    installServer(1);
    window.history.replaceState(null, '', '/#/items');
    const { unmount } = render(<ItemsList />);
    expect(await screen.findByRole('link', { name: 'Download 1 item as CSV' })).toBeInTheDocument();
    unmount();

    installServer(0);
    render(<ItemsList />);
    await screen.findByText(/No items/);
    expect(screen.queryByRole('link', { name: /CSV/ })).toBeNull();
  });
});
