/**
 * The site Users tab: who may work on the site, and with which role.
 *
 * The tab runs through the real client and parsers against a stubbed fetch,
 * so the `o:site_permission` list a save sends is the body the server gets.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, requestBody, requestsFor } from './stub';

const permission = (id: number, role: string): Record<string, unknown> => ({
  'o:user': { '@id': `http://localhost/api/users/${String(id)}`, 'o:id': id },
  'o:role': role,
});

/** The admin fixture's site with `permissions` as its `o:site_permission`. */
const siteWith = (permissions: readonly Record<string, unknown>[]): Record<string, unknown> => {
  const raw = loadFixture('site-admin.json');
  if (!isRecord(raw)) {
    throw new Error('expected a site object');
  }
  return { ...raw, 'o:site_permission': permissions };
};

/** The users fixture plus a third user no permission names yet. */
const users = (): unknown[] => {
  const page = loadFixture('users-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected a user list');
  }
  return [
    ...page,
    {
      '@id': 'http://localhost/api/users/7',
      '@type': 'o:User',
      'o:id': 7,
      'o:name': 'Katherine Johnson',
      'o:email': 'katherine@example.org',
      'o:created': '2026-10-15T10:00:00+00:00',
      'o:role': 'researcher',
      'o:is_active': true,
    },
  ];
};

const renderUsers = (site: Record<string, unknown>): ReturnType<typeof installFetch> => {
  const fetchMock = installFetch((url) => {
    if (url.pathname === '/api/sites/1') {
      return jsonResponse(site);
    }
    if (url.pathname === '/api/users') {
      return listResponse(users(), 3);
    }
    return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
  });
  render(<SitesPage param="1" rest={['1', 'users']} />);
  return fetchMock;
};

const savedPermissions = (fetchMock: ReturnType<typeof installFetch>): unknown[] =>
  requestsFor(fetchMock, '/api/sites/1')
    .filter((request) => request.init?.method === 'PATCH')
    .map((request) => requestBody(request)['o:site_permission']);

const optionNames = (select: HTMLElement): string[] =>
  within(select)
    .getAllByRole('option')
    .map((option) => option.textContent);

describe('site users', () => {
  it('site_users_patches_the_permission_list_it_keeps', async () => {
    const fetchMock = renderUsers(siteWith([permission(3, 'admin'), permission(5, 'viewer')]));

    expect(await screen.findAllByLabelText('Role')).toHaveLength(2);
    fireEvent.click(first(screen.getAllByRole('button', { name: 'Remove' })));
    fireEvent.change(screen.getByLabelText('Role'), { target: { value: 'editor' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    expect(savedPermissions(fetchMock)).toEqual([[{ 'o:user': { 'o:id': 5 }, 'o:role': 'editor' }]]);
  });

  it('a_row_names_its_user_and_never_offers_a_bare_number', async () => {
    renderUsers(siteWith([permission(3, 'admin')]));

    const user = await screen.findByLabelText('User');
    expect(user).toHaveDisplayValue('Ada Lovelace');
    expect(optionNames(user)).not.toContain('3');
  });

  it('a_row_whose_user_the_list_lacks_names_it_by_number_with_a_word', async () => {
    renderUsers(siteWith([permission(42, 'viewer')]));

    expect(await screen.findByLabelText('User')).toHaveDisplayValue('User 42');
  });

  it('add_user_offers_only_users_the_site_does_not_list_yet', async () => {
    const fetchMock = renderUsers(siteWith([permission(3, 'admin')]));

    const add = await screen.findByLabelText('Add user');
    expect(optionNames(add)).toEqual(['Select a user', 'Grace Hopper', 'Katherine Johnson']);
    fireEvent.change(add, { target: { value: '5' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add user' }));
    expect(optionNames(screen.getByLabelText('Add user'))).toEqual([
      'Select a user',
      'Katherine Johnson',
    ]);
    // A row's own list leaves out the users other rows hold.
    const rows = screen.getAllByLabelText('User');
    expect(optionNames(first(rows))).not.toContain('Grace Hopper');

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    expect(savedPermissions(fetchMock)).toEqual([
      [
        { 'o:user': { 'o:id': 3 }, 'o:role': 'admin' },
        { 'o:user': { 'o:id': 5 }, 'o:role': 'viewer' },
      ],
    ]);
  });

  it('a_row_with_no_user_is_flagged_and_blocks_the_save_instead_of_vanishing', async () => {
    const fetchMock = renderUsers(siteWith([permission(3, 'admin'), permission(5, 'viewer')]));

    const rows = await screen.findAllByLabelText('User');
    fireEvent.change(rows[1] ?? first(rows), { target: { value: '' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Choose a user in each row, or remove the row.',
    );
    expect(rows[1]).toHaveAttribute('aria-invalid', 'true');
    expect(savedPermissions(fetchMock)).toEqual([]);
  });
});
