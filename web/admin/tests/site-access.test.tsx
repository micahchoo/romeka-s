/**
 * What each site role may do, and what each site tab shows a caller who may
 * not change it (`AclFactory::addRulesForSites`, as
 * `crates/omeka-api/src/read/mod.rs` checks it: a site update needs the owner,
 * an admin role or the `admin` site role, a page needs `editor` or above).
 *
 * The signed-in user comes from the session store the shell's probe fills.
 */
import { render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { isRecord, parseSite, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { Site, User } from '../src/api/types';
import { SitesPage } from '../src/pages/sites/page';
import { siteAccess } from '../src/pages/sites/siteAccess';
import type { SiteCapability } from '../src/pages/sites/siteAccess';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

/** The admin fixture's site: owned by user 3, with user 8 a Manager, 9 a Creator, 5 a Viewer. */
const siteJson = (): Record<string, unknown> => {
  const raw = loadFixture('site-admin.json');
  if (!isRecord(raw)) {
    throw new Error('expected a site object');
  }
  const permission = (id: number, role: string): Record<string, unknown> => ({
    'o:user': { '@id': `http://localhost/api/users/${String(id)}`, 'o:id': id },
    'o:role': role,
  });
  return {
    ...raw,
    'o:site_permission': [
      permission(3, 'admin'),
      permission(8, 'admin'),
      permission(9, 'editor'),
      permission(5, 'viewer'),
    ],
  };
};

const site = (): Site => parseSite(siteJson());

const user = (id: number, role: string): User =>
  parseUser({
    '@id': `http://localhost/api/users/${String(id)}`,
    '@type': 'o:User',
    'o:id': id,
    'o:name': `User ${String(id)}`,
    'o:email': `user${String(id)}@example.org`,
    'o:created': '2026-10-15T08:00:00+00:00',
    'o:role': role,
    'o:is_active': true,
  });

const OWNER = user(3, 'researcher');
const GLOBAL_ADMIN = user(11, 'global_admin');
const MANAGER = user(8, 'researcher');
const CREATOR = user(9, 'author');
const VIEWER = user(5, 'editor');
const STRANGER = user(10, 'editor');

const sorted = (capabilities: ReadonlySet<SiteCapability>): SiteCapability[] =>
  [...capabilities].sort();

afterEach(() => {
  setSession(null);
});

describe('site access', () => {
  it('the owner and the admin roles hold every capability, deleting the site among them', () => {
    expect(sorted(siteAccess(site(), OWNER))).toEqual(['changeSite', 'deleteSite', 'editPages']);
    expect(sorted(siteAccess(site(), GLOBAL_ADMIN))).toEqual([
      'changeSite',
      'deleteSite',
      'editPages',
    ]);
    expect(sorted(siteAccess(site(), user(12, 'site_admin')))).toEqual([
      'changeSite',
      'deleteSite',
      'editPages',
    ]);
  });

  it('a manager changes the site but does not delete it; a creator only edits pages', () => {
    expect(sorted(siteAccess(site(), MANAGER))).toEqual(['changeSite', 'editPages']);
    expect(sorted(siteAccess(site(), CREATOR))).toEqual(['editPages']);
  });

  it('a viewer and a user with no site role change nothing, whatever their global role', () => {
    expect(sorted(siteAccess(site(), VIEWER))).toEqual([]);
    expect(sorted(siteAccess(site(), STRANGER))).toEqual([]);
  });

  it('with no session known the admin offers everything and the server decides', () => {
    expect(sorted(siteAccess(site(), null))).toEqual(['changeSite', 'deleteSite', 'editPages']);
  });
});

const handler = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/sites/1':
      return jsonResponse(siteJson());
    case '/api/site_pages':
      return listResponse(
        [loadFixture('site-page-1.json'), loadFixture('site-page-2-private.json')],
        2,
      );
    case '/api/users':
      return listResponse(loadFixture('users-page.json'), 2);
    case '/api/properties':
      return listResponse(loadFixture('properties-page.json'), 2);
    case '/api/items':
      return listResponse(loadFixture('items-page.json'), 7);
    case '/api/item_sets':
      return listResponse(loadFixture('item-sets-page.json'), 2);
    default:
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
  }
};

const open = async (signedIn: User, tab: string | null): Promise<void> => {
  setSession(signedIn);
  installFetch(handler);
  render(<SitesPage param="1" rest={tab === null ? ['1'] : ['1', tab]} />);
  await screen.findByRole('navigation', { name: 'Site sections' });
};

const tabNames = (): string[] =>
  within(screen.getByRole('navigation', { name: 'Site sections' }))
    .getAllByRole('link')
    .map((link) => link.textContent);

describe('the site tabs by role', () => {
  it('a manager sees every tab and every control', async () => {
    await open(MANAGER, null);
    const draft = await screen.findByRole('row', { name: /Draft/ });
    expect(within(draft).getByRole('button', { name: 'Set as homepage' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Add page' })).toBeInTheDocument();
    expect(tabNames()).toEqual(['Pages', 'Navigation', 'Items', 'User permissions', 'Theme', 'Site settings']);
  });

  it('a creator adds and edits pages but sees no homepage or menu control, with the reason', async () => {
    await open(CREATOR, null);
    const draft = await screen.findByRole('row', { name: /Draft/ });
    expect(within(draft).queryByRole('button', { name: 'Set as homepage' })).toBeNull();
    expect(within(draft).queryByRole('button', { name: 'Add to navigation' })).toBeNull();
    expect(within(draft).getByRole('link', { name: 'Edit' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Add page' })).toBeInTheDocument();
    expect(
      screen.getByText('Only site managers can change the homepage and the menu.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'Edit site settings' })).toBeNull();
    expect(tabNames()).toEqual(['Pages', 'Navigation', 'Items']);
  });

  it('a viewer reads the pages and can neither add nor edit one', async () => {
    await open(VIEWER, null);
    const draft = await screen.findByRole('row', { name: /Draft/ });
    expect(within(draft).queryByRole('link', { name: 'Edit' })).toBeNull();
    expect(within(draft).queryByRole('link', { name: 'Draft' })).toBeNull();
    expect(screen.queryByRole('link', { name: 'Add page' })).toBeNull();
    expect(
      screen.getByText('You can view this site. Only its creators and managers can change it.'),
    ).toBeInTheDocument();
  });

  it('a creator sees the menu but not the controls that change it', async () => {
    await open(CREATOR, 'navigation');
    expect(await screen.findByText('Only site managers can change the menu.')).toBeInTheDocument();
    expect(screen.getByRole('complementary', { name: 'Menu preview' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add link' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Save' })).toBeNull();
  });

  it('a creator sees the site items but cannot add or remove one', async () => {
    await open(CREATOR, 'resources');
    expect(await screen.findByText('Items on this site: 7')).toBeInTheDocument();
    expect(
      screen.getByText('Only site managers can add or remove items.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Add items' })).toBeNull();
    expect(screen.queryByRole('button', { name: /^Remove/ })).toBeNull();
    expect(screen.getByRole('switch')).toBeDisabled();
  });

  it('a creator who opens users, theme or settings by address gets the reason, not a form', async () => {
    for (const [tab, reason] of [
      ['users', 'Only site managers can change who works on this site.'],
      ['theme', 'Only site managers can change the theme.'],
      ['settings', 'Only site managers can change the site settings.'],
    ] as const) {
      await open(CREATOR, tab);
      expect(await screen.findByText(reason)).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Save' })).toBeNull();
      document.body.innerHTML = '';
    }
  });

  it('a manager who does not own the site is told who can delete it', async () => {
    await open(MANAGER, 'settings');
    expect(
      await screen.findByText('Only the site owner or an administrator can delete this site.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Delete/ })).toBeNull();
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument();
  });

  it('the owner may delete the site', async () => {
    await open(OWNER, 'settings');
    expect(await screen.findByRole('button', { name: 'Save' })).toBeInTheDocument();
    expect(
      screen.queryByText('Only the site owner or an administrator can delete this site.'),
    ).toBeNull();
  });
});
