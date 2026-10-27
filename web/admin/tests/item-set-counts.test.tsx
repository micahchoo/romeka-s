/**
 * The set list's Items column: each set's count from one counts request per page, as a link to
 * the set's place, and the item count order offered only to a role that sees every item.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { User } from '../src/api/types';
import { ItemSetList } from '../src/pages/item-sets/list';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, route, urlsFor } from './stub';
import type { Handler } from './stub';

const user = (role: string): User => ({
  id: brandId<'UserId'>(1),
  apiUrl: 'http://localhost/api/users/1',
  types: ['o:User'],
  name: 'Reader',
  email: 'reader@example.com',
  created: '2026-10-16T09:00:00+00:00',
  role,
  isActive: true,
  localeSetting: null,
  lessons: { hidden: false, seen: [] },
  adminLocale: null,
});

/** Sets 1 and 2; the counts route answers `counts`. */
const server = (counts: Handler): Handler =>
  route({
    '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
    '/api/users': () => jsonResponse(loadFixture('users-page.json')),
    '/api/romeka/item-sets/counts': counts,
  });

const rowOf = async (title: string): Promise<HTMLElement> =>
  (await screen.findByRole('link', { name: title })).closest('tr') ?? document.body;

afterEach(() => {
  setSession(null);
});

describe('the set list Items column', () => {
  it('shows each count as a link to the set, 0 as 0, from one request for the page', async () => {
    const fetchMock = installFetch(server(() => jsonResponse({ item_sets: [{ id: 1, items: 3410 }, { id: 2, items: 0 }] })));
    render(<ItemSetList />);

    const first = await rowOf('Featured photographs');
    expect(within(first).getByRole('link', { name: '3,410 items' })).toHaveAttribute('href', '#/items?set=1');
    expect(within(first).getByRole('link', { name: '3,410 items' })).toHaveTextContent('3,410');
    const second = await rowOf('Working drafts');
    expect(within(second).getByRole('link', { name: '0 items' })).toHaveAttribute('href', '#/items?set=2');
    const requests = urlsFor(fetchMock, '/api/romeka/item-sets/counts');
    expect(requests).toHaveLength(1);
    expect(requests[0]?.searchParams.getAll('id[]')).toEqual(['1', '2']);
    expect(screen.getByRole('columnheader', { name: 'Items' })).toBeInTheDocument();
  });

  it('shows a dash for a set the answer leaves out', async () => {
    installFetch(server(() => jsonResponse({ item_sets: [{ id: 1, items: 1 }] })));
    render(<ItemSetList />);

    const second = await rowOf('Working drafts');
    expect(within(second).getByRole('img', { name: 'No count for this set' })).toHaveTextContent('—');
    expect(within(await rowOf('Featured photographs')).getByRole('link', { name: '1 item' })).toBeInTheDocument();
  });

  it('keeps the rows when the counts fail, each count a dash that says why', async () => {
    installFetch(server(() => jsonResponse({ errors: { error: 'Counts are down' } }, { status: 500 })));
    render(<ItemSetList />);

    const first = await rowOf('Featured photographs');
    expect(within(first).getByRole('img', { name: 'Counts are down' })).toHaveAttribute('title', 'Counts are down');
  });
});

describe('the item count order', () => {
  it('is offered to an editor and sorts by item count', async () => {
    setSession(user('editor'));
    const fetchMock = installFetch(server(() => jsonResponse({ item_sets: [] })));
    render(<ItemSetList />);

    fireEvent.change(await screen.findByRole('combobox', { name: 'Sort' }), { target: { value: 'most' } });
    await screen.findByRole('link', { name: 'Featured photographs' });
    const last = urlsFor(fetchMock, '/api/item_sets').at(-1);
    expect(last?.searchParams.get('sort_by')).toBe('item_count');
    expect(last?.searchParams.get('sort_order')).toBe('desc');
  });

  it('is not offered to an author, whose counts leave out what the order counts', async () => {
    setSession(user('author'));
    installFetch(server(() => jsonResponse({ item_sets: [] })));
    render(<ItemSetList />);

    await screen.findByRole('link', { name: 'Featured photographs' });
    expect(screen.queryByRole('combobox', { name: 'Sort' })).not.toBeInTheDocument();
  });
});
