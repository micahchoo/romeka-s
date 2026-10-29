/**
 * Dashboard, system info, query and settings page tests. All fetch
 * stubs serve fixtures through the real typed client; none touch the network.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { DashboardPage } from '../src/pages/dashboard/page';
import { QueryPage } from '../src/pages/system/query';
import { SettingsPage } from '../src/pages/settings/page';
import { SystemInfoPage } from '../src/pages/system/about';
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

  it('dashboard_shows_resource_counts_and_links_each_site', async () => {
    installFetch(route(dashboardRoutes));
    render(<DashboardPage />);

    expect(await screen.findByRole('link', { name: 'Items (60)' })).toHaveAttribute(
      'href',
      '#/items',
    );
    expect(screen.getByRole('link', { name: 'Item sets (2)' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Standards (2)' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Item types (2)' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'My Site' })).toHaveAttribute('href', '#/sites/1');
    expect(screen.getByRole('link', { name: 'Second Site' })).toHaveAttribute('href', '#/sites/2');
  });

  it('dashboard_error_state_shows_the_upstream_message', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'Server error' } }, { status: 500 }));
    render(<DashboardPage />);

    // "Needs attention" and the resource links read apart, and each says why it has nothing.
    const alerts = await screen.findAllByRole('alert');
    expect(alerts).toHaveLength(2);
    for (const alert of alerts) {
      expect(alert).toHaveTextContent('Server error (HTTP 500)');
    }
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

describe('settings page', () => {
  const fileSettings = {
    media_type_whitelist: ['application/pdf', 'image/jpeg'],
    extension_whitelist: ['pdf', 'jpg'],
    disable_file_validation: false,
    import_allow_private_urls: false,
    romeka_annotations_public: true,
  };
  /** `SettingForm`'s general, display and editing fields as a new install answers them. */
  const installSettings = {
    installation_title: 'Romeka S',
    administrator_email: 'admin@example.org',
    time_zone: 'UTC',
    pagination_per_page: 25,
    default_to_private_item_sets: false,
    version_notifications: false,
  };
  const installSettingsWrite = { ...installSettings, index_fulltext_search: false };
  const settingsRoutes = (settings: (init: RequestInit | undefined) => object) => ({
    '/api/settings': (_url: URL, init: RequestInit | undefined) =>
      jsonResponse({ ...fileSettings, ...installSettings, ...settings(init) }),
    '/api/romeka/time-zones': () => jsonResponse(['Asia/Kolkata', 'Europe/Paris', 'UTC']),
    '/api/romeka/installation': () =>
      jsonResponse({
        installation_title: 'Romeka S',
        pagination_per_page: 25,
        time_zone: 'UTC',
        default_to_private_item_sets: false,
      }),
  });
  const patchOf = (fetchMock: ReturnType<typeof installFetch>) =>
    first(requestsFor(fetchMock, '/api/settings').filter((request) => request.init?.method === 'PATCH'));

  it('settings_saves_every_field_and_reads_the_installation_again', async () => {
    const fetchMock = installFetch(
      route(settingsRoutes((init) => ({ locale: init?.method === 'PATCH' ? 'en' : 'es' }))),
    );
    render(<SettingsPage />);

    const locale = await screen.findByLabelText('Locale');
    expect(locale).toHaveValue('es');
    fireEvent.change(locale, { target: { value: 'en' } });
    const title = screen.getByLabelText('Installation title');
    expect(title).toHaveValue('Romeka S');
    expect(title).toBeRequired();
    fireEvent.change(title, { target: { value: 'Gawan Museum' } });
    fireEvent.change(screen.getByLabelText('Administrator email'), {
      target: { value: 'registrar@gawan.example' },
    });
    const zone = screen.getByLabelText('Time zone');
    expect(zone).toHaveValue('UTC');
    expect(screen.getAllByRole('option', { name: 'Asia/Kolkata' })).toHaveLength(1);
    fireEvent.change(zone, { target: { value: 'Asia/Kolkata' } });
    const perPage = screen.getByLabelText('Results per page');
    expect(perPage).toHaveValue(25);
    fireEvent.change(perPage, { target: { value: '50' } });
    fireEvent.click(screen.getByLabelText('Make new item sets private'));
    expect(screen.getByLabelText('Tell me about new versions')).toHaveAccessibleDescription(
      'Stored only for now. Romeka does not check for new versions yet.',
    );
    fireEvent.change(screen.getByLabelText('Media alt text property'), {
      target: { value: 'dcterms:title' },
    });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByRole('status')).toHaveTextContent('Settings updated.');
    expect(requestBody(patchOf(fetchMock))).toEqual({
      ...fileSettings,
      ...installSettingsWrite,
      locale: 'en',
      media_alt_text_property: 'dcterms:title',
      installation_title: 'Gawan Museum',
      administrator_email: 'registrar@gawan.example',
      time_zone: 'Asia/Kolkata',
      pagination_per_page: 50,
      default_to_private_item_sets: true,
    });
    expect(requestsFor(fetchMock, '/api/romeka/installation')).toHaveLength(1);
  });

  it('settings_starts_the_index_job_and_links_to_it', async () => {
    const fetchMock = installFetch(
      route(
        settingsRoutes((init) => (init?.method === 'PATCH' ? { index_fulltext_search_job: 7 } : {})),
      ),
    );
    render(<SettingsPage />);

    const reindex = await screen.findByLabelText('Rebuild the full-text search index');
    expect(reindex).not.toBeChecked();
    fireEvent.click(reindex);
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('The index rebuild started as job 7.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Follow it on its job page.' })).toHaveAttribute(
      'href',
      '#/jobs/7',
    );
    expect(requestBody(patchOf(fetchMock))).toMatchObject({ index_fulltext_search: true });
    expect(screen.getByLabelText('Rebuild the full-text search index')).not.toBeChecked();
  });

  it('settings_saves_the_file_validation_lists_and_restores_a_default_with_null', async () => {
    const restoredTypes = ['application/msword', 'application/pdf'];
    const fetchMock = installFetch(
      route(
        settingsRoutes((init) =>
          init?.method === 'PATCH'
            ? {
                media_type_whitelist: restoredTypes,
                extension_whitelist: ['pdf', 'tif'],
                disable_file_validation: true,
              }
            : {},
        ),
      ),
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
      ...installSettingsWrite,
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
      route(
        settingsRoutes((init) => ({ locale: null, import_allow_private_urls: init?.method === 'PATCH' })),
      ),
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
