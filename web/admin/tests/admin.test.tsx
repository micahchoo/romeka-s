/**
 * Dashboard, system info, query, columns and settings page tests. All fetch
 * stubs serve fixtures through the real typed client; none touch the network.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ColumnsPage } from '../src/pages/columns/page';
import { DashboardPage } from '../src/pages/dashboard/page';
import { QueryPage } from '../src/pages/query/page';
import { SettingsPage } from '../src/pages/settings/page';
import { SystemInfoPage } from '../src/pages/system-info/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, route, urlsFor } from './stub';

describe('dashboard page', () => {
  const dashboardRoutes = {
    '/api/items': () => listResponse(loadFixture('items-page.json'), 60),
    '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
    '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2),
    '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
    '/api/sites': () => listResponse(loadFixture('sites-page.json'), 2),
  };

  it('dashboard_shows_resource_counts_and_the_site_list', async () => {
    installFetch(route(dashboardRoutes));
    render(<DashboardPage />);

    expect(await screen.findByRole('link', { name: 'Items (60)' })).toHaveAttribute(
      'href',
      '#/items',
    );
    expect(screen.getByRole('link', { name: 'Item sets (2)' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Vocabularies (2)' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Resource templates (2)' })).toBeInTheDocument();
    expect(screen.getByText('My Site')).toBeInTheDocument();
    expect(screen.getByText('Second Site')).toBeInTheDocument();
  });

  it('dashboard_error_state_shows_the_upstream_message', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Server error' } }, { status: 500 }));
    render(<DashboardPage />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Server error (HTTP 500)');
  });
});

describe('system info page', () => {
  it('system_info_reports_the_version_and_registered_resources', async () => {
    installFetch(
      route({
        '/api': () =>
          jsonResponse(loadFixture('api-root.json'), {
            headers: { 'Omeka-S-Version': '4.3.0-alpha' },
          }),
      }),
    );
    render(<SystemInfoPage />);

    expect(await screen.findByText('4.3.0-alpha')).toBeInTheDocument();
    expect(screen.getByText('items, media, users, vocabularies')).toBeInTheDocument();
  });

  it('system_info_error_state_shows_the_upstream_message', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Not found' } }, { status: 404 }));
    render(<SystemInfoPage />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Not found (HTTP 404)');
  });
});

describe('query page', () => {
  const queryRoutes = {
    '/api/items': () => listResponse(loadFixture('items-page.json'), 2),
    '/api/media': () => listResponse(loadFixture('media-page.json'), 2),
    '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
  };

  it('query_preview_runs_the_search_and_lists_the_matches', async () => {
    const fetchMock = installFetch(route(queryRoutes));
    render(<QueryPage />);

    fireEvent.change(screen.getByLabelText('Search'), { target: { value: 'cat' } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview' }));

    expect(await screen.findByText('Fixture Item One')).toBeInTheDocument();
    expect(first(urlsFor(fetchMock, '/api/items')).searchParams.get('search')).toBe('cat');
  });

  it('query_preview_can_target_media', async () => {
    const fetchMock = installFetch(route(queryRoutes));
    render(<QueryPage />);

    fireEvent.change(screen.getByLabelText('Resource type'), { target: { value: 'media' } });
    fireEvent.change(screen.getByLabelText('Search'), { target: { value: 'photo' } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview' }));

    expect(await screen.findByText('Fixture photo')).toBeInTheDocument();
    const mediaUrl = first(urlsFor(fetchMock, '/api/media'));
    expect(mediaUrl.searchParams.get('search')).toBe('photo');
  });
});

describe('columns page', () => {
  it('columns_toggle_hides_a_browse_column_from_the_preview', () => {
    render(<ColumnsPage />);

    expect(screen.getByRole('columnheader', { name: 'Created' })).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText('Created'));
    expect(screen.queryByRole('columnheader', { name: 'Created' })).not.toBeInTheDocument();
  });

  it('columns_follow_the_selected_resource_type', () => {
    render(<ColumnsPage />);

    fireEvent.change(screen.getByLabelText('Resource type'), { target: { value: 'media' } });
    expect(screen.getByRole('columnheader', { name: 'Ingester' })).toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: 'Created' })).not.toBeInTheDocument();
  });
});

describe('settings page', () => {
  it('settings_edit_updates_the_field_and_saves', () => {
    render(<SettingsPage />);

    fireEvent.change(screen.getByLabelText('Installation title'), {
      target: { value: 'My Omeka' },
    });
    expect(screen.getByLabelText('Installation title')).toHaveValue('My Omeka');

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(screen.getByRole('status')).toHaveTextContent('Settings updated.');
  });
});
