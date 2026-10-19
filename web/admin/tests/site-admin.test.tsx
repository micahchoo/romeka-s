/**
 * Site-admin page tests: navigation, resources, users and theme settings.
 *
 * Each page runs through the real client and parsers against a stubbed fetch,
 * so the method, URL and body a form sends are the bytes the server would get.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { parseHash } from '../src/app/router';
import { Shell } from '../src/app/shell';
import { SitesPage } from '../src/pages/sites/page';
import { siteSettingDrafts, siteSettingsWire } from '../src/pages/sites/siteSettingsSchema';
import { SITE_SETTINGS_REGISTRY } from '../src/pages/sites/siteSettingsRegistry';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, requestBody, requestsFor } from './stub';

const site = (): unknown => loadFixture('site-admin.json');

const usersRoute = (url: URL): Response | null =>
  url.pathname === '/api/users' ? listResponse(loadFixture('users-page.json'), 2) : null;

const sitePagesRoute = (url: URL): Response | null =>
  url.pathname === '/api/site_pages'
    ? listResponse([loadFixture('site-page-1.json'), loadFixture('site-page-2-private.json')], 2)
    : null;

const notFound = (): Response =>
  jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });

describe('site section routing', () => {
  it('router_keeps_a_site_section_segment', () => {
    expect(parseHash('#/sites/1/theme')).toEqual({
      page: 'sites',
      param: '1',
      rest: ['1', 'theme'],
    });
  });

  it('shell_routes_a_site_section_url_to_its_editor', async () => {
    window.location.hash = '#/sites/1/resources';
    installFetch((url) => {
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/item_sets') {
        return listResponse(loadFixture('item-sets-page.json'), 2);
      }
      if (url.pathname === '/api/items') {
        return listResponse(loadFixture('items-page.json'), 7);
      }
      return notFound();
    });
    render(<Shell />);

    expect(await screen.findByText('Items on this site: 7')).toBeInTheDocument();
    window.location.hash = '';
  });
});

const pagePatch = (fetchMock: ReturnType<typeof installFetch>): Record<string, unknown>[] =>
  requestsFor(fetchMock, '/api/sites/1')
    .filter((request) => request.init?.method === 'PATCH')
    .map(requestBody);

const siteRoutes = (url: URL): Response | null =>
  url.pathname === '/api/sites/1' ? jsonResponse(site()) : sitePagesRoute(url);

describe('site hub', () => {
  it('hub_lists_each_page_with_its_visibility_homepage_and_menu_state', async () => {
    installFetch((url) => siteRoutes(url) ?? notFound());
    render(<SitesPage param="1" rest={['1']} />);

    const table = await screen.findByRole('table');
    const welcome = within(table).getByRole('row', { name: /Welcome/ });
    const draft = within(table).getByRole('row', { name: /Draft/ });
    expect(within(welcome).getByRole('link', { name: 'Welcome' })).toHaveAttribute(
      'href',
      '#/sites/1/pages/1',
    );
    expect(welcome).toHaveTextContent('Homepage');
    expect(welcome).toHaveTextContent('Public');
    expect(welcome).toHaveTextContent('In navigation');
    expect(within(welcome).queryByRole('button', { name: 'Set as homepage' })).toBeNull();
    expect(within(welcome).getByRole('link', { name: 'View' })).toHaveAttribute(
      'href',
      '/s/my-site/page/welcome',
    );
    expect(draft).toHaveTextContent('Private');
    expect(draft).not.toHaveTextContent('Homepage');
    expect(within(draft).getByRole('button', { name: 'Add to navigation' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Add page' })).toHaveAttribute(
      'href',
      '#/sites/1/pages/new',
    );
  });

  it('hub_sets_the_homepage_and_adds_a_page_to_the_menu_with_one_member_each', async () => {
    const fetchMock = installFetch((url) => siteRoutes(url) ?? notFound());
    render(<SitesPage param="1" rest={['1']} />);

    const draft = await screen.findByRole('row', { name: /Draft/ });
    fireEvent.click(within(draft).getByRole('button', { name: 'Set as homepage' }));
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    fireEvent.click(within(draft).getByRole('button', { name: 'Add to navigation' }));
    await screen.findByText('Saving...');
    expect(await screen.findByText('Saved.')).toBeInTheDocument();

    expect(pagePatch(fetchMock)).toEqual([
      { 'o:homepage': { 'o:id': 2 } },
      {
        'o:navigation': [
          { type: 'browse', data: { label: 'Browse', query: '' }, links: [] },
          { type: 'page', data: { id: 1, label: 'Welcome' }, links: [] },
          { type: 'page', data: { id: 2, label: null }, links: [] },
        ],
      },
    ]);
  });

  it('hub_clears_an_explicit_homepage_with_one_null_member_and_follows_the_menu', async () => {
    const stored = site();
    if (!isRecord(stored)) {
      throw new Error('the site fixture is not an object');
    }
    const named = { ...stored, 'o:homepage': { '@id': 'http://localhost/api/site_pages/2', 'o:id': 2 } };
    const followsMenu = { ...stored, 'o:homepage': null };
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(init?.method === 'PATCH' ? followsMenu : named);
      }
      return sitePagesRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1']} />);

    const draft = await screen.findByRole('row', { name: /Draft/ });
    const welcome = screen.getByRole('row', { name: /Welcome/ });
    expect(draft).toHaveTextContent('Homepage');
    expect(within(welcome).queryByRole('button', { name: 'Use the first menu page' })).toBeNull();
    fireEvent.click(within(draft).getByRole('button', { name: 'Use the first menu page' }));
    expect(await screen.findByText('Saved.')).toBeInTheDocument();

    expect(pagePatch(fetchMock)).toEqual([{ 'o:homepage': null }]);
    expect(draft).not.toHaveTextContent('Homepage');
    expect(welcome).toHaveTextContent('Homepage');
    expect(screen.queryByRole('button', { name: 'Use the first menu page' })).toBeNull();
  });

  it('every_site_screen_shares_one_breadcrumb_heading_and_tab_bar', async () => {
    installFetch((url) => siteRoutes(url) ?? notFound());
    const { rerender } = render(<SitesPage param="1" rest={['1']} />);

    const crumbs = await screen.findByRole('navigation', { name: 'Breadcrumb' });
    expect(within(crumbs).getByRole('link', { name: 'Sites' })).toHaveAttribute('href', '#/sites');
    expect(await within(crumbs).findByText('My Site')).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('heading', { level: 2, name: 'My Site' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'View site' })).toHaveAttribute('href', '/s/my-site');
    const tabs = screen.getByRole('navigation', { name: 'Site sections' });
    expect(within(tabs).getByRole('link', { name: 'Pages' })).toHaveAttribute(
      'aria-current',
      'page',
    );

    rerender(<SitesPage param="1" rest={['1', 'navigation']} />);
    expect(await within(crumbs).findByRole('link', { name: 'My Site' })).toHaveAttribute(
      'href',
      '#/sites/1',
    );
    expect(within(crumbs).getByText('Navigation')).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('heading', { level: 2, name: 'Navigation' })).toBeInTheDocument();
    expect(within(tabs).getByRole('link', { name: 'Navigation' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(within(tabs).getByRole('link', { name: 'Pages' })).not.toHaveAttribute('aria-current');
    expect(within(tabs).getByRole('link', { name: 'Site settings' })).toHaveAttribute(
      'href',
      '#/sites/1/settings',
    );
  });
});

describe('site navigation', () => {
  const tree = (): HTMLElement => screen.getByRole('list', { name: 'Menu links' });

  it('navigation_editor_patches_the_tree_it_builds', async () => {
    const fetchMock = installFetch((url) => siteRoutes(url) ?? notFound());
    render(<SitesPage param="1" rest={['1', 'navigation']} />);

    expect(await screen.findByRole('button', { name: 'Move Browse' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Move Welcome' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Add link' }));
    // A new link opens with its controls, as a page link until its type changes.
    fireEvent.change(screen.getByLabelText('Link type'), { target: { value: 'url' } });
    fireEvent.change(screen.getByLabelText('URL'), { target: { value: 'https://example.org' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'Example' } });
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    expect(pagePatch(fetchMock)).toEqual([
      {
        'o:navigation': [
          { type: 'browse', data: { label: 'Browse', query: '' }, links: [] },
          { type: 'page', data: { id: 1, label: 'Welcome' }, links: [] },
          {
            type: 'url',
            data: { label: 'Example', url: 'https://example.org', target_blank: false },
            links: [],
          },
        ],
      },
    ]);
    expect(screen.queryByText('Unsaved changes')).toBeNull();
  });

  it('navigation_editor_nests_a_link_by_keyboard_and_escape_puts_it_back', async () => {
    const fetchMock = installFetch((url) => siteRoutes(url) ?? notFound());
    render(<SitesPage param="1" rest={['1', 'navigation']} />);

    const handle = await screen.findByRole('button', { name: 'Move Welcome' });
    fireEvent.click(handle);
    expect(handle).toHaveAttribute('aria-pressed', 'true');
    fireEvent.keyDown(handle, { key: 'ArrowUp' });
    expect(screen.getByText('Welcome: position 1 of 2, level 1.')).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole('button', { name: 'Move Welcome' }), { key: 'Escape' });
    expect(within(tree()).getAllByRole('button', { name: /^Move / }).map((b) => b.getAttribute('aria-label'))).toEqual([
      'Move Browse',
      'Move Welcome',
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Move Welcome' }));
    fireEvent.keyDown(screen.getByRole('button', { name: 'Move Welcome' }), { key: 'ArrowRight' });
    expect(screen.getByText('Welcome: position 1 of 1, level 2.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Move Welcome' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    expect(pagePatch(fetchMock)).toEqual([
      {
        'o:navigation': [
          {
            type: 'browse',
            data: { label: 'Browse', query: '' },
            links: [{ type: 'page', data: { id: 1, label: 'Welcome' }, links: [] }],
          },
        ],
      },
    ]);
  });

  it('the_menu_preview_shows_the_labels_the_public_menu_falls_back_to', async () => {
    installFetch((url) => siteRoutes(url) ?? notFound());
    render(<SitesPage param="1" rest={['1', 'navigation']} />);

    await screen.findByRole('button', { name: 'Move Welcome' });
    fireEvent.click(screen.getByRole('button', { name: 'Edit Welcome' }));
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: '' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add link' }));
    fireEvent.change(screen.getByLabelText('Page'), { target: { value: '2' } });

    const preview = screen.getByRole('complementary', { name: 'Menu preview' });
    expect(within(preview).getAllByRole('listitem').map((item) => item.textContent)).toEqual([
      'Browse',
      'Welcome',
      'Draft',
    ]);
  });

  it('navigation_editor_rejects_a_nested_url_link_with_no_target', async () => {
    const fetchMock = installFetch((url) => siteRoutes(url) ?? notFound());
    render(<SitesPage param="1" rest={['1', 'navigation']} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Add child link to Browse' }));
    fireEvent.change(screen.getByLabelText('Link type'), { target: { value: 'url' } });
    expect(within(tree()).getByText('Needs a target')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'A page or URL link needs its target.',
    );
    expect(pagePatch(fetchMock)).toHaveLength(0);
  });
});

describe('site items', () => {
  const siteItemsRoutes =
    (dryRun: Record<string, unknown>) =>
    (url: URL, init?: RequestInit): Response | null => {
      if (url.pathname === '/api/sites/1/items') {
        const raw = init?.body;
        const body: unknown = typeof raw === 'string' ? JSON.parse(raw) : null;
        return jsonResponse(
          isRecord(body) && body['dry_run'] === true
            ? dryRun
            : { ...dryRun, total: 9 },
        );
      }
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/item_sets') {
        return listResponse(loadFixture('item-sets-page.json'), 2);
      }
      if (url.pathname === '/api/items') {
        return listResponse(loadFixture('items-page.json'), 7);
      }
      if (url.pathname === '/api/resource_classes') {
        return listResponse([], 0);
      }
      return null;
    };
  const itemChanges = (fetchMock: ReturnType<typeof installFetch>): Record<string, unknown>[] =>
    requestsFor(fetchMock, '/api/sites/1/items').map(requestBody);

  it('the search tab counts what it would add, then saves the search and adds the items', async () => {
    const fetchMock = installFetch(
      (url, init) =>
        siteItemsRoutes({ matched: 3, added: 2, removed: 0, total: 9 })(url, init) ?? notFound(),
    );
    render(<SitesPage param="1" rest={['1', 'resources']} />);

    expect(await screen.findByText('Items on this site: 7')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'By search' }));
    const search = screen.getByRole('group', { name: 'Search for items' });
    // The search selects a set of items, so it offers no sort.
    expect(within(search).queryByLabelText('Sort by')).not.toBeInTheDocument();
    fireEvent.change(within(search).getByLabelText('Full-text search'), {
      target: { value: 'fish' },
    });
    expect(await screen.findByText('3 items match, 2 not on this site yet.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Add items (2)' }));

    expect(await screen.findByText('Items added: 2.')).toBeInTheDocument();
    expect(pagePatch(fetchMock)).toEqual([{ 'o:item_pool': { fulltext_search: 'fish' } }]);
    expect(itemChanges(fetchMock).filter((body) => body['dry_run'] === false)).toEqual([
      { action: 'add', query: { fulltext_search: 'fish' }, dry_run: false },
    ]);
  });

  it('the item set tab attaches a set and adds its items in one action', async () => {
    const fetchMock = installFetch(
      (url, init) =>
        siteItemsRoutes({ matched: 4, added: 4, removed: 0, total: 9 })(url, init) ?? notFound(),
    );
    render(<SitesPage param="1" rest={['1', 'resources']} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Featured photographs' }));
    expect(
      await screen.findByText('Featured photographs: 4 items, 4 not on this site yet.'),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Attach the set and add items (4)' }));

    expect(
      await screen.findByText('Items added: 4.'),
    ).toBeInTheDocument();
    expect(pagePatch(fetchMock)).toEqual([
      // The stored attachment (set 2) keeps its place; the new one appends.
      { 'o:site_item_set': [{ 'o:item_set': { 'o:id': 2 } }, { 'o:item_set': { 'o:id': 1 } }] },
    ]);
    expect(itemChanges(fetchMock).filter((body) => body['dry_run'] === false)).toEqual([
      { action: 'add', item_sets: [1], dry_run: false },
    ]);
  });

  it('the sync switch stores assign_new_items', async () => {
    const fetchMock = installFetch(
      (url, init) =>
        siteItemsRoutes({ matched: 0, added: 0, removed: 0, total: 7 })(url, init) ?? notFound(),
    );
    render(<SitesPage param="1" rest={['1', 'resources']} />);

    fireEvent.click(
      await screen.findByRole('switch', { name: 'Keep this site in sync with the search' }),
    );
    expect(
      await screen.findByText('Saved.'),
    ).toBeInTheDocument();
    expect(pagePatch(fetchMock)).toEqual([{ 'o:assign_new_items': true }]);
  });
});

describe('site users', () => {
  it('site_users_patches_the_permission_list_it_keeps', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1' && init?.method === 'PATCH') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'users']} />);

    const roles = await screen.findAllByLabelText('Role');
    expect(roles).toHaveLength(2);
    // User 3's row is removed and user 5 becomes an editor.
    fireEvent.click(first(screen.getAllByRole('button', { name: 'Remove' })));
    fireEvent.change(screen.getByLabelText('Role'), { target: { value: 'editor' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    );
    expect(requestBody(patch)).toEqual({
      'o:site_permission': [{ 'o:user': { 'o:id': 5 }, 'o:role': 'editor' }],
    });
  });

  it('site_users_adds_a_row_from_the_user_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1' && init?.method === 'PATCH') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'users']} />);

    await screen.findByLabelText('Add user');
    fireEvent.change(screen.getByLabelText('Add user'), { target: { value: '5' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add user' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    );
    const body = requestBody(patch);
    const permissions = body['o:site_permission'];
    expect(isUnknownArray(permissions)).toBe(true);
    if (isUnknownArray(permissions)) {
      expect(permissions).toHaveLength(3);
      expect(permissions[2]).toEqual({ 'o:user': { 'o:id': 5 }, 'o:role': 'viewer' });
    }
  });
});

describe('site settings', () => {
  it('site_settings_patch_the_map_with_the_schema_defaults_filled_in', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1' && init?.method === 'PATCH') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'settings']} />);

    const title = await screen.findByLabelText('Installation title');
    expect(title).toHaveValue('Library');
    expect(screen.getByLabelText('Show page pagination')).not.toBeChecked();
    fireEvent.change(title, { target: { value: 'Archive' } });
    fireEvent.click(screen.getByLabelText('Show page pagination'));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    );
    const body = requestBody(patch);
    expect(body).not.toHaveProperty('o:theme_settings');
    expect(body['o:site_settings']).toMatchObject({
      installation_title: 'Archive',
      locale: 'en',
      attachment_link_type: 'item',
      show_page_pagination: true,
    });
  });

  it('site_schema_defaults_are_the_accessor_defaults', () => {
    const drafts = siteSettingDrafts(SITE_SETTINGS_REGISTRY, {});
    expect(drafts['installation_title']).toBe('Romeka S');
    expect(drafts['locale']).toBe('en');
    expect(drafts['show_page_pagination']).toBe(true);
    // An unset default renders an empty control and stores null.
    expect(drafts['subnav_display']).toBe('');
    expect(siteSettingsWire(SITE_SETTINGS_REGISTRY, drafts)['subnav_display']).toBeNull();
    const stored = siteSettingDrafts(SITE_SETTINGS_REGISTRY, { installation_title: 'Library' });
    expect(stored['installation_title']).toBe('Library');
  });
});
