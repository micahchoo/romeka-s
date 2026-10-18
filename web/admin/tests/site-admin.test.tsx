/**
 * Site-admin page tests: navigation, resources, users and theme settings.
 *
 * Each page runs through the real client and parsers against a stubbed fetch,
 * so the method, URL and body a form sends are the bytes the server would get.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isUnknownArray } from '../src/api/parse';
import { parseHash } from '../src/app/router';
import { Shell } from '../src/app/shell';
import { SitesPage } from '../src/pages/sites/page';
import {
  addNavigationLink,
  moveNavigationLink,
  navigationDrafts,
  navigationJson,
  removeNavigationLink,
} from '../src/pages/sites/navigation';
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

    expect(await screen.findByText('Items in this site: 7')).toBeInTheDocument();
    window.location.hash = '';
  });
});

describe('site detail', () => {
  it('site_detail_lists_each_page_with_its_visibility', async () => {
    installFetch((url) => {
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(loadFixture('site-page-1.json'));
      }
      if (url.pathname === '/api/site_pages/2') {
        return jsonResponse(loadFixture('site-page-2-private.json'));
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1']} />);

    const welcome = await screen.findByRole('link', { name: 'Welcome' });
    const draft = screen.getByRole('link', { name: 'Draft' });
    expect(welcome).toHaveAttribute('href', '#/sites/1/pages/1');
    // The private page carries the marker; the public one does not.
    expect(draft.parentElement).toHaveTextContent('Draft Private');
    expect(welcome.parentElement).toHaveTextContent('Welcome');
    expect(welcome.parentElement).not.toHaveTextContent('Private');
  });

  it('site_detail_links_to_the_member_group_pages', async () => {
    installFetch((url) => {
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/site_pages/1' || url.pathname === '/api/site_pages/2') {
        return jsonResponse(loadFixture('site-page-1.json'));
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1']} />);

    const nav = await screen.findByRole('navigation', { name: 'Site sections' });
    expect(within(nav).getByRole('link', { name: 'Navigation' })).toHaveAttribute(
      'href',
      '#/sites/1/navigation',
    );
    expect(within(nav).getByRole('link', { name: 'Site settings' })).toHaveAttribute(
      'href',
      '#/sites/1/settings',
    );
  });
});

describe('site navigation', () => {
  it('navigation_editor_patches_the_tree_it_builds', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1' && init?.method === 'PATCH') {
        return jsonResponse(site());
      }
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      return sitePagesRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'navigation']} />);

    // The two stored links load, then a third is added as a custom URL.
    expect(await screen.findAllByLabelText('Link type')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'Add link' }));
    const types = screen.getAllByLabelText('Link type');
    fireEvent.change(types[2] ?? first(types), { target: { value: 'url' } });
    fireEvent.change(screen.getByLabelText('URL'), {
      target: { value: 'https://example.org' },
    });
    const labels = screen.getAllByLabelText('Label');
    fireEvent.change(labels[2] ?? first(labels), { target: { value: 'Example' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    );
    expect(patch.url.pathname).toBe('/api/sites/1');
    expect(requestBody(patch)).toEqual({
      'o:navigation': [
        { type: 'browse', data: { label: 'Browse', query: '' }, links: [] },
        { type: 'page', data: { id: 1, label: 'Welcome' }, links: [] },
        {
          type: 'url',
          data: { label: 'Example', url: 'https://example.org', target_blank: false },
          links: [],
        },
      ],
    });
  });

  it('navigation_editor_rejects_a_url_link_with_no_target', async () => {
    const fetchMock = installFetch((url) => {
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(site());
      }
      return sitePagesRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'navigation']} />);

    await screen.findAllByLabelText('Link type');
    fireEvent.click(screen.getByRole('button', { name: 'Add link' }));
    const types = screen.getAllByLabelText('Link type');
    fireEvent.change(types[2] ?? first(types), { target: { value: 'url' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'A page or URL link needs its target.',
    );
    expect(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    ).toHaveLength(0);
  });

  it('navigation_model_round_trips_and_reorders_the_tree', () => {
    const stored = [
      { type: 'browse', data: { label: 'Browse', query: '' }, links: [] },
      { type: 'url', data: { label: 'Home', url: '/', target_blank: true }, links: [] },
    ];
    const drafts = navigationDrafts(stored);
    expect(navigationJson(drafts)).toEqual(stored);
    const moved = moveNavigationLink(drafts, drafts[1]?.key ?? 0, -1);
    expect(navigationJson(moved)[0]).toEqual(stored[1]);
    const added = addNavigationLink(moved, null, 'page');
    expect(navigationJson(added)).toHaveLength(3);
    const removed = removeNavigationLink(added, drafts[0]?.key ?? 0);
    expect(navigationJson(removed)).toHaveLength(2);
  });

  it('navigation_model_keeps_an_unknown_link_type_verbatim', () => {
    const stored = [{ type: 'thirdParty', data: { keep: 'this' }, links: [] }];
    expect(navigationJson(navigationDrafts(stored))).toEqual(stored);
  });
});

describe('site resources', () => {
  it('site_resources_patches_the_item_pool_and_item_sets', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1' && init?.method === 'PATCH') {
        return jsonResponse(site());
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
      return notFound();
    });
    render(<SitesPage param="1" rest={['1', 'resources']} />);

    expect(await screen.findByText('Items in this site: 7')).toBeInTheDocument();
    const pool = screen.getByLabelText('Item pool (JSON)');
    fireEvent.change(pool, { target: { value: '{"search":"fish"}' } });
    fireEvent.click(screen.getByLabelText('Featured photographs'));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const patch = first(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    );
    expect(requestBody(patch)).toEqual({
      'o:item_pool': { search: 'fish' },
      // The stored attachment (set 2) keeps its place; the new one appends.
      'o:site_item_set': [
        { 'o:item_set': { 'o:id': 2 } },
        { 'o:item_set': { 'o:id': 1 } },
      ],
    });
  });

  it('site_resources_shows_a_typed_error_for_bad_pool_json', async () => {
    const fetchMock = installFetch((url) => {
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
    render(<SitesPage param="1" rest={['1', 'resources']} />);

    fireEvent.change(await screen.findByLabelText('Item pool (JSON)'), {
      target: { value: '{' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Item pool (JSON):');
    expect(
      requestsFor(fetchMock, '/api/sites/1').filter(
        (request) => request.init?.method === 'PATCH',
      ),
    ).toHaveLength(0);
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
      return notFound();
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
