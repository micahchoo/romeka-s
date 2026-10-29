/**
 * The System page: what was four Admin pages (System information, Modules,
 * Search query, Browse columns) is one page under Admin > Advanced, one tab
 * each, and every old route still opens it.
 */
import { render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { parseHash } from '../src/app/router';
import { Shell } from '../src/app/shell';
import { SystemPage } from '../src/pages/system/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route, signedIn } from './stub';

afterEach(() => {
  window.location.hash = '';
});

describe('system routes', () => {
  it('each_old_route_opens_its_system_tab', () => {
    expect(parseHash('#/system')).toEqual({ page: 'system', param: null, rest: [] });
    expect(parseHash('#/system-info')).toEqual({ page: 'system', param: 'about', rest: ['about'] });
    expect(parseHash('#/modules')).toEqual({ page: 'system', param: 'modules', rest: ['modules'] });
    expect(parseHash('#/query')).toEqual({ page: 'system', param: 'query', rest: ['query'] });
    expect(parseHash('#/columns')).toEqual({ page: 'system', param: 'about', rest: ['about'] });
  });

  it('the_nav_offers_system_under_admin_advanced_and_none_of_the_old_pages', async () => {
    window.location.hash = '#/modules';
    installFetch(signedIn(() => jsonResponse({ errors: { error: 'offline' } }, { status: 503 })));
    render(<Shell />);

    const nav = await screen.findByRole('navigation', { name: 'Admin pages' });
    const admin = within(nav).getByRole('heading', { name: 'Admin' }).closest('section');
    if (admin === null) {
      throw new Error('no Admin group');
    }
    const advanced = within(admin).getByRole('heading', { name: 'Advanced' }).closest('li');
    if (advanced === null) {
      throw new Error('no Advanced heading under Admin');
    }
    const system = within(advanced).getByRole('link', { name: 'System' });
    expect(system).toHaveAttribute('href', '#/system');
    expect(system).toHaveAttribute('aria-current', 'page');
    for (const gone of ['System information', 'Modules', 'Search query', 'Browse columns']) {
      expect(within(nav).queryByRole('link', { name: gone })).not.toBeInTheDocument();
    }
    expect(await screen.findByRole('link', { name: 'Modules' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });
});

describe('system tabs', () => {
  const systemRoutes = {
    '/api': () =>
      jsonResponse(loadFixture('api-root.json'), { headers: { 'Omeka-S-Version': '4.3.0-alpha' } }),
    '/api/modules': () => listResponse(loadFixture('modules-page.json'), 1),
  };

  it('the_page_opens_on_the_installation_facts_with_a_tab_for_each_tool', async () => {
    installFetch(route(systemRoutes));
    render(<SystemPage />);

    expect(await screen.findByText('4.3.0-alpha')).toBeInTheDocument();
    const tabs = screen.getByRole('navigation', { name: 'System' });
    expect(within(tabs).getByRole('link', { name: 'System information' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(within(tabs).getByRole('link', { name: 'Modules' })).toHaveAttribute(
      'href',
      '#/system/modules',
    );
    expect(within(tabs).getByRole('link', { name: 'Search query' })).toHaveAttribute(
      'href',
      '#/system/query',
    );
  });

  it('the_modules_tab_lists_the_modules', async () => {
    installFetch(route(systemRoutes));
    render(<SystemPage rest={['modules']} />);

    expect(await screen.findByRole('table')).toBeInTheDocument();
    expect(
      within(screen.getByRole('navigation', { name: 'System' })).getByRole('link', {
        name: 'Modules',
      }),
    ).toHaveAttribute('aria-current', 'page');
  });

  it('the_search_query_tab_shows_the_search_form', () => {
    installFetch(route(systemRoutes));
    render(<SystemPage rest={['query']} />);

    expect(screen.getByLabelText('Search')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Preview' })).toBeInTheDocument();
  });

  it('an_unknown_tab_says_there_is_no_such_page', () => {
    render(<SystemPage rest={['nope']} />);

    expect(screen.getByRole('alert')).toHaveTextContent('No such page.');
  });
});
