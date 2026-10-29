/**
 * What the user page offers for whom: your own account hides the controls the
 * server refuses you, the role select says what each role may do, a site admin
 * is not offered the admin roles, and the page lists the user's site roles.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { UsersPage } from '../src/pages/users/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, meJson } from './stub';

const userFixture = (index: number): unknown => {
  const page = loadFixture('users-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected a user list fixture');
  }
  return page[index];
};

const sitesFixture = (): unknown[] => {
  const page = loadFixture('sites-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected a site list fixture');
  }
  return page.filter(isRecord).map((site, index) => ({
    ...site,
    'o:site_permission':
      index === 0
        ? [
            { 'o:user': { '@id': 'http://localhost/api/users/5', 'o:id': 5 }, 'o:role': 'admin' },
            { 'o:user': { '@id': 'http://localhost/api/users/3', 'o:id': 3 }, 'o:role': 'viewer' },
          ]
        : [{ 'o:user': { '@id': 'http://localhost/api/users/3', 'o:id': 3 }, 'o:role': 'editor' }],
  }));
};

/** User 3 is Ada, a global admin; user 5 is Grace, inactive. */
const serve = (): void => {
  installFetch((url) => {
    if (url.pathname === '/api/users/3') {
      return jsonResponse(userFixture(0));
    }
    if (url.pathname === '/api/users/5') {
      return jsonResponse(userFixture(1));
    }
    if (url.pathname === '/api/sites') {
      return listResponse(sitesFixture(), 2);
    }
    return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
  });
};

const signInAs = (id: number, role: string): void => {
  setSession(parseUser({ ...meJson(role), '@id': `http://localhost/api/users/${String(id)}`, 'o:id': id }));
};

afterEach(() => {
  setSession(null);
});

describe('your own account', () => {
  it('hides Role, Active and Delete, and says who can change them', async () => {
    serve();
    signInAs(3, 'global_admin');
    render(<UsersPage param="3" />);

    expect(await screen.findByText('Your role is Global Administrator.')).toBeInTheDocument();
    expect(
      screen.getByText("You can't change your own role or deactivate yourself. Another administrator can."),
    ).toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: 'Role' })).not.toBeInTheDocument();
    expect(screen.queryByRole('checkbox', { name: 'Active' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Delete this user' })).not.toBeInTheDocument();
  });

  it('keeps all three on another account', async () => {
    serve();
    signInAs(3, 'global_admin');
    render(<UsersPage param="5" />);

    expect(await screen.findByRole('combobox', { name: 'Role' })).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Active' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete this user' })).toBeInTheDocument();
    expect(screen.queryByText(/You can't change your own role/)).not.toBeInTheDocument();
  });
});

describe('the role select', () => {
  it('says under it what the chosen role may do', async () => {
    serve();
    signInAs(3, 'global_admin');
    render(<UsersPage param="5" />);

    const select = await screen.findByRole('combobox', { name: 'Role' });
    fireEvent.change(select, { target: { value: 'researcher' } });
    expect(select).toHaveAccessibleDescription(
      'May read public records. May change only their own account.',
    );
    fireEvent.change(select, { target: { value: 'reviewer' } });
    expect(select).toHaveAccessibleDescription(
      'May add and change every item, item set and media, and see private records. May delete only what they own.',
    );
  });

  it('offers a site admin no administrator role to give', async () => {
    serve();
    signInAs(9, 'site_admin');
    render(<UsersPage param="5" />);

    const select = await screen.findByRole('combobox', { name: 'Role' });
    const options = within(select)
      .getAllByRole('option')
      .map((option) => option.textContent);
    expect(options).toEqual(['Select role...', 'Editor', 'Reviewer', 'Author', 'Researcher']);
  });
});

describe('the sites a user has a role on', () => {
  it('lists each site with the role, linked to its Users tab, and leaves out the rest', async () => {
    serve();
    signInAs(3, 'global_admin');
    render(<UsersPage param="5" />);

    const table = await screen.findByRole('table');
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows.map((row) => row.textContent)).toEqual(['My SiteManager']);
    expect(within(table).getByRole('link', { name: 'My Site' })).toHaveAttribute('href', '#/sites/1/users');
  });

  it('says so when the user has no role on a site the reader can see', async () => {
    installFetch((url) => {
      if (url.pathname === '/api/users/5') {
        return jsonResponse(userFixture(1));
      }
      if (url.pathname === '/api/sites') {
        return listResponse([], 0);
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    signInAs(3, 'global_admin');
    render(<UsersPage param="5" />);

    expect(await screen.findByText('This user has no role on any site you can see.')).toBeInTheDocument();
  });
});
