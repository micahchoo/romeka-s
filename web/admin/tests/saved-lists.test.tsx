/**
 * Saved lists: the Items screen names its filters and order, the nav lists the names under
 * Items, and opening one shows its name with Rename and Delete. The lists are the user's
 * `romeka_saved_lists` setting (`crates/omeka-server/tests/saved_lists.rs`); each change reads
 * `GET /api/me` first, so a list saved in another browser is kept.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { brandId, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { SavedList, User } from '../src/api/types';
import { SavedListLinks } from '../src/app/saved-list-links';
import { ItemsList } from '../src/pages/items/list';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestBody, requestsFor, route } from './stub';

const researcher = (savedLists: readonly SavedList[]): User => ({
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
  savedLists,
  adminLocale: null,
});

/** The server's copy of the lists: `GET /api/me` answers it, a `PATCH` replaces it. */
const installServer = (initial: readonly SavedList[]) => {
  let stored: readonly SavedList[] = initial;
  const me = (): Record<string, unknown> => ({
    '@id': 'http://localhost/api/users/4',
    '@type': 'o:User',
    'o:id': 4,
    'o:name': 'Researcher',
    'o:email': 'researcher@example.com',
    'o:created': '2026-10-16T09:00:00+00:00',
    'o:role': 'researcher',
    'o:is_active': true,
    ...(stored.length === 0 ? {} : { 'o:user_settings': { romeka_saved_lists: stored } }),
  });
  const fixture = (name: string) => () => jsonResponse(loadFixture(name));
  const fetchMock = installFetch(
    route({
      '/api/items': () => listResponse(loadFixture('items-page.json'), 2),
      '/api/users': fixture('users-page.json'),
      '/api/resource_classes': fixture('resource-classes-page.json'),
      '/api/item_sets': fixture('item-sets-page.json'),
      '/api/resource_templates': fixture('resource-templates-page.json'),
      '/api/sites': fixture('sites-page.json'),
      '/api/properties': fixture('properties-page.json'),
      '/api/me': () => jsonResponse(me()),
      '/api/users/4': (url, init) => {
        stored = parseUser({ ...me(), 'o:user_settings': requestBody({ url, init })['o:user_settings'] }).savedLists;
        return jsonResponse(me());
      },
    }),
  );
  return { fetchMock, stored: () => stored };
};

const written = (fetchMock: ReturnType<typeof installServer>['fetchMock']): unknown[] =>
  requestsFor(fetchMock, '/api/users/4').map((request) => requestBody(request)['o:user_settings']);

describe('saved lists', () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    setSession(null);
  });

  it('lists each saved list under Items, marking the one open', () => {
    setSession(researcher([{ name: 'Letters', query: 'class=3' }, { name: 'Undated', query: 'field=7%3Aempty' }]));
    window.history.replaceState(null, '', '/#/items?class=3&page=2');
    render(<SavedListLinks />);
    const lists = screen.getByRole('list', { name: 'Saved lists' });
    const letters = within(lists).getByRole('link', { name: 'Letters' });
    expect(letters).toHaveAttribute('href', '#/items?class=3');
    expect(letters).toHaveAttribute('aria-current', 'true');
    expect(within(lists).getByRole('link', { name: 'Undated' })).not.toHaveAttribute('aria-current');
  });

  it('shows no list for a user with none', () => {
    setSession(researcher([]));
    render(<SavedListLinks />);
    expect(screen.queryByRole('list', { name: 'Saved lists' })).toBeNull();
  });

  it('saves, renames and deletes the list on screen, keeping a list saved elsewhere', async () => {
    const elsewhere = { name: 'Undated', query: 'field=7%3Aempty' };
    setSession(researcher([]));
    const server = installServer([elsewhere]);
    window.history.replaceState(null, '', '/#/items?class=3&sort=title&page=2');
    render(<ItemsList />);

    fireEvent.click(await screen.findByRole('button', { name: 'Save this list' }));
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), { target: { value: 'Letters by title' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved list: Letters by title')).toBeInTheDocument();
    expect(server.stored()).toEqual([elsewhere, { name: 'Letters by title', query: 'class=3&sort=title' }]);

    fireEvent.click(screen.getByRole('button', { name: 'Rename saved list' }));
    const name = screen.getByRole('textbox', { name: 'Name' });
    expect(name).toHaveValue('Letters by title');
    fireEvent.change(name, { target: { value: 'Letters' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved list: Letters')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete saved list' }));
    expect(await screen.findByText('Deleted the saved list Letters.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save this list' })).toBeInTheDocument();
    expect(written(server.fetchMock)).toEqual([
      { romeka_saved_lists: [elsewhere, { name: 'Letters by title', query: 'class=3&sort=title' }] },
      { romeka_saved_lists: [elsewhere, { name: 'Letters', query: 'class=3&sort=title' }] },
      { romeka_saved_lists: [elsewhere] },
    ]);
  });

  it('refuses an empty name and a name already used, without a write', async () => {
    setSession(researcher([{ name: 'Letters', query: 'class=3' }]));
    const server = installServer([{ name: 'Letters', query: 'class=3' }]);
    window.history.replaceState(null, '', '/#/items?class=4');
    render(<ItemsList />);

    fireEvent.click(await screen.findByRole('button', { name: 'Save this list' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Give the list a name.')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), { target: { value: ' Letters ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('You already have a list named Letters.')).toBeInTheDocument();
    await waitFor(() => {
      expect(written(server.fetchMock)).toEqual([]);
    });
  });

  it('offers no save for the plain list of every item', async () => {
    setSession(researcher([]));
    installServer([]);
    window.history.replaceState(null, '', '/#/items');
    render(<ItemsList />);
    await screen.findByRole('link', { name: 'Fixture Item One' });
    expect(screen.queryByRole('button', { name: 'Save this list' })).toBeNull();
  });
});
