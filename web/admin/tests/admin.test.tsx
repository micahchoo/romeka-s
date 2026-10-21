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
import {
  first,
  installFetch,
  jsonResponse,
  listResponse,
  requestBody,
  requestsFor,
  route,
  urlsFor,
} from './stub';

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
  const fileSettings = {
    media_type_whitelist: ['application/pdf', 'image/jpeg'],
    extension_whitelist: ['pdf', 'jpg'],
    disable_file_validation: false,
    import_allow_private_urls: false,
    romeka_annotations_public: true,
  };

  it('settings_saves_the_global_locale_and_keeps_the_other_fields_in_the_page', async () => {
    const fetchMock = installFetch(
      route({
        '/api/settings': (_url, init) =>
          jsonResponse({ ...fileSettings, locale: init?.method === 'PATCH' ? 'en' : 'es' }),
      }),
    );
    render(<SettingsPage />);

    const locale = await screen.findByLabelText('Locale');
    expect(locale).toHaveValue('es');
    fireEvent.change(locale, { target: { value: 'en' } });
    fireEvent.change(screen.getByLabelText('Installation title'), {
      target: { value: 'My Omeka' },
    });
    expect(screen.getByLabelText('Installation title')).toHaveValue('My Omeka');
    const altTextProperty = screen.getByLabelText('Media alt text property');
    expect(altTextProperty).toHaveValue('');
    fireEvent.change(altTextProperty, { target: { value: 'dcterms:title' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByRole('status')).toHaveTextContent('Settings updated.');
    const patch = first(
      requestsFor(fetchMock, '/api/settings').filter((request) => request.init?.method === 'PATCH'),
    );
    expect(requestBody(patch)).toEqual({
      locale: 'en',
      media_alt_text_property: 'dcterms:title',
      ...fileSettings,
    });
  });

  it('settings_saves_the_file_validation_lists_and_restores_a_default_with_null', async () => {
    const restoredTypes = ['application/msword', 'application/pdf'];
    const fetchMock = installFetch(
      route({
        '/api/settings': (_url, init) =>
          jsonResponse(
            init?.method === 'PATCH'
              ? {
                  ...fileSettings,
                  media_type_whitelist: restoredTypes,
                  extension_whitelist: ['pdf', 'tif'],
                  disable_file_validation: true,
                }
              : fileSettings,
          ),
      }),
    );
    render(<SettingsPage />);

    const types = await screen.findByLabelText('Allowed media types');
    expect(types).toHaveValue('application/pdf,image/jpeg');
    const extensions = screen.getByLabelText('Allowed file extensions');
    fireEvent.change(extensions, { target: { value: ' pdf , tif,, ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Restore default media types' }));
    expect(types).toHaveValue('');
    expect(types).toHaveAccessibleDescription(
      'A comma-separated list of allowed media types for file uploads. The default list is restored when you save.',
    );
    fireEvent.change(types, { target: { value: ' , ' } });
    expect(types).toHaveAccessibleDescription(
      'A comma-separated list of allowed media types for file uploads. An empty list refuses every file.',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Restore default media types' }));
    fireEvent.click(screen.getByLabelText('Disable file validation'));

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByRole('status')).toHaveTextContent('Settings updated.');
    const patch = first(
      requestsFor(fetchMock, '/api/settings').filter((request) => request.init?.method === 'PATCH'),
    );
    expect(requestBody(patch)).toEqual({
      locale: '',
      media_alt_text_property: '',
      media_type_whitelist: null,
      extension_whitelist: ['pdf', 'tif'],
      disable_file_validation: true,
      import_allow_private_urls: false,
      romeka_annotations_public: true,
    });
    expect(types).toHaveValue(restoredTypes.join(','));
    expect(screen.getByLabelText('Disable file validation')).toBeChecked();
  });

  it('settings_allows_private_urls_and_warns_while_the_switch_is_on', async () => {
    const fetchMock = installFetch(
      route({
        '/api/settings': (_url, init) =>
          jsonResponse({
            ...fileSettings,
            locale: null,
            import_allow_private_urls: init?.method === 'PATCH',
          }),
      }),
    );
    render(<SettingsPage />);

    const allow = await screen.findByLabelText('Allow URLs on private networks');
    expect(allow).not.toBeChecked();
    expect(allow).toHaveAccessibleDescription(
      'Media and imports refuse URLs that point at this server or its private network: localhost, the local network and cloud metadata addresses. Check this to allow them.',
    );
    fireEvent.click(allow);
    expect(allow).toHaveAccessibleDescription(
      expect.stringContaining(
        'While this is on, anyone who can add media can make this server fetch from its own network.',
      ),
    );

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByRole('status')).toHaveTextContent('Settings updated.');
    const patch = first(
      requestsFor(fetchMock, '/api/settings').filter((request) => request.init?.method === 'PATCH'),
    );
    expect(requestBody(patch)).toMatchObject({ import_allow_private_urls: true });
    expect(screen.getByLabelText('Allow URLs on private networks')).toBeChecked();
  });

  it('settings_shows_the_refusal_a_caller_other_than_a_global_admin_gets', async () => {
    installFetch(() =>
      jsonResponse(
        { errors: { error: 'Permission denied for the current user to access the global settings.' } },
        { status: 403 },
      ),
    );
    render(<SettingsPage />);

    expect(await screen.findByRole('alert')).toHaveTextContent('global settings');
    expect(screen.queryByLabelText('Locale')).toBeNull();
  });
});
