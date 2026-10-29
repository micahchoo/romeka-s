/**
 * Resource list page tests. A stubbed fetch serves the fixtures, so each page
 * runs through the real typed client, its parsers, and the shared ListPage.
 * No test touches the network.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { AssetsPage } from '../src/pages/assets/page';
import { ItemSetsPage } from '../src/pages/item-sets/page';
import { JobsPage } from '../src/pages/jobs/page';
import { MediaPage } from '../src/pages/media/page';
import { ModulesPage } from '../src/pages/system/modules';
import { PropertiesPage } from '../src/pages/properties/page';
import { ResourceClassesPage } from '../src/pages/resource-classes/page';
import { ItemTypesPage } from '../src/pages/item-types/page';
import { UsersPage } from '../src/pages/users/page';
import { VocabulariesPage } from '../src/pages/vocabularies/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, linkHeader, listResponse, route, urlsFor } from './stub';
const users = (): Response => jsonResponse(loadFixture('users-page.json'));
const classes = (): Response => jsonResponse(loadFixture('resource-classes-page.json'));

describe('assets page', () => {
  it('asset_browse_shows_name_filename_media_type_and_owner', async () => {
    installFetch(
      route({
        '/api/assets': () => listResponse(loadFixture('assets-page.json'), 2),
        '/api/users': users,
      }),
    );
    render(<AssetsPage />);

    expect(await screen.findByText('omeka-logo.png')).toBeInTheDocument();
    expect(
      screen.getByText('a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.png'),
    ).toBeInTheDocument();
    expect(screen.getByText('image/png')).toBeInTheDocument();
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getByText('A decorative heading')).toBeInTheDocument();
    expect(screen.getByText('[No owner]')).toBeInTheDocument();
  });

  it('asset_page_shows_the_upstream_error_with_its_status', async () => {
    installFetch(() =>
      jsonResponse({ errors: { error: 'Permission denied' } }, { status: 403 }),
    );
    render(<AssetsPage />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Permission denied (HTTP 403)');
  });
});

describe('item sets page', () => {
  it('item_set_browse_shows_title_owner_dates_and_flags', async () => {
    installFetch(
      route({
        '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
        '/api/users': users,
      }),
    );
    render(<ItemSetsPage />);

    expect(await screen.findByText('Featured photographs')).toBeInTheDocument();
    expect(screen.getByText('Working drafts')).toBeInTheDocument();
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getByText('Feb 10, 2024, 8:15 AM')).toBeInTheDocument();
    expect(screen.getAllByText('Yes')).toHaveLength(2);
    expect(screen.getAllByText('No')).toHaveLength(2);
  });

  it('pagination fetches the next page', async () => {
    const fetchMock = installFetch((url) => {
      if (url.pathname === '/api/item_sets') {
        if (url.searchParams.get('page') === '2') {
          return listResponse(loadFixture('item-sets-page.json'), 3, {
            Link: linkHeader([
              ['prev', 1],
              ['last', 2],
            ]),
          });
        }
        return listResponse(loadFixture('item-sets-page.json'), 3, {
          Link: linkHeader([
            ['next', 2],
            ['last', 2],
          ]),
        });
      }
      if (url.pathname === '/api/users') {
        return users();
      }
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    });
    render(<ItemSetsPage />);

    expect(await screen.findByText('Page 1 of 2 (3 item sets)')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(await screen.findByText('Page 2 of 2 (3 item sets)')).toBeInTheDocument();
    const pages = urlsFor(fetchMock, '/api/item_sets').map((url) => url.searchParams.get('page'));
    expect(pages).toEqual(['1', '2']);
  });
});

describe('jobs page', () => {
  it('job_browse_shows_class_status_label_owner_and_times', async () => {
    installFetch(
      route({
        '/api/jobs': () => listResponse(loadFixture('jobs-page.json'), 2),
        '/api/users': users,
      }),
    );
    render(<JobsPage />);

    expect(await screen.findByText('Site items update')).toBeInTheDocument();
    expect(screen.getByText('Completed')).toBeInTheDocument();
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getAllByText('Jan 3, 2024, 11:00 AM')).toHaveLength(2);
    expect(screen.getByText('In Progress')).toBeInTheDocument();
    expect(screen.getByText('[No owner]')).toBeInTheDocument();
  });
});

describe('media page', () => {
  it('media_browse_shows_item_link_type_source_and_ingester', async () => {
    installFetch(route({ '/api/media': () => listResponse(loadFixture('media-page.json'), 2) }));
    render(<MediaPage param={null} />);

    expect(await screen.findByText('Fixture photo')).toBeInTheDocument();
    expect(first(screen.getAllByRole('link', { name: '#1' }))).toHaveAttribute(
      'href',
      '#/items/1',
    );
    expect(screen.getByText('image/jpeg')).toBeInTheDocument();
    expect(screen.getByText('photo.jpg')).toBeInTheDocument();
    expect(screen.getByText('upload')).toBeInTheDocument();
    expect(screen.getByText('[Untitled]')).toBeInTheDocument();
    expect(screen.getAllByText('Embed')).not.toHaveLength(0);
  });

  it('media_kind_filter_reads_the_server_kind_and_keeps_it_in_the_url', async () => {
    const rows: unknown = loadFixture('media-page-kinds.json');
    const fetchMock = installFetch((url) => {
      const kinds = url.searchParams.getAll('romeka_kind');
      const picked = kinds.includes('audio') && Array.isArray(rows) ? [rows[1]] : rows;
      return listResponse(picked, Array.isArray(picked) ? picked.length : 0);
    });
    window.history.replaceState(null, '', '#/media');
    render(<MediaPage param={null} />);

    const audio = await screen.findByRole('row', { name: /tone\.mp3/ });
    expect(within(audio).getAllByRole('cell')[1]).toHaveTextContent('Audio');
    const photo = screen.getByRole('row', { name: /photo\.jpg/ });
    expect(within(photo).getAllByRole('cell')[1]).toHaveTextContent('Image');
    expect(screen.getByRole('option', { name: 'IIIF manifest' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Document' })).toBeInTheDocument();
    expect(screen.queryByText('local-image')).toBeNull();

    const select = screen.getByLabelText('Kind');
    fireEvent.change(select, { target: { value: 'audio' } });
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/media').at(-1)?.searchParams.getAll('romeka_kind')).toEqual([
        'audio',
      ]);
    });
    await waitFor(() => {
      expect(screen.queryByRole('row', { name: /photo\.jpg/ })).toBeNull();
    });
    expect(window.location.hash).toBe('#/media?kind=audio');

    fireEvent.change(select, { target: { value: 'image' } });
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/media').at(-1)?.searchParams.getAll('romeka_kind')).toEqual([
        'local-image',
        'iiif-image',
      ]);
    });

    fireEvent.change(select, { target: { value: 'document' } });
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/media').at(-1)?.searchParams.getAll('romeka_kind')).toEqual([
        'document',
      ]);
    });
  });
});

describe('modules page', () => {
  it('module_browse_reads_name_version_and_state_from_ini', async () => {
    installFetch(route({ '/api/modules': () => listResponse(loadFixture('modules-page.json'), 2) }));
    render(<ModulesPage />);

    expect(await screen.findByText('3.4.0')).toBeInTheDocument();
    expect(screen.getByText('active')).toBeInTheDocument();
    expect(screen.getByText('Omeka Team')).toBeInTheDocument();
    expect(screen.getAllByText('Common')).toHaveLength(2);
    expect(screen.getAllByText('Archive')).toHaveLength(2);
    expect(screen.getByText('not_active')).toBeInTheDocument();
  });
});

describe('item types page', () => {
  it('item_type_browse_shows_name_field_count_item_count_and_class', async () => {
    installFetch(
      route({
        '/api/resource_templates': () =>
          listResponse(loadFixture('resource-templates-page.json'), 2),
        '/api/items': (url) =>
          listResponse([], url.searchParams.get('resource_template_id') === '2' ? 41 : 0),
        '/api/resource_classes': classes,
      }),
    );
    render(<ItemTypesPage />);

    const photograph = (await screen.findByRole('link', { name: 'Photograph' })).closest('tr');
    const document = screen.getByRole('link', { name: 'Document' }).closest('tr');
    if (photograph === null || document === null) {
      throw new Error('expected two rows');
    }
    expect(Array.from(photograph.cells, (cell) => cell.textContent)).toEqual([
      'Photograph',
      '3',
      '41',
      'Photograph or picture dctype:StillImage',
    ]);
    expect(Array.from(document.cells, (cell) => cell.textContent)).toEqual(['Document', '0', '0', 'None']);
  });
});

describe('users page', () => {
  it('user_browse_shows_name_email_role_active_and_created', async () => {
    installFetch(route({ '/api/users': users }));
    render(<UsersPage />);

    expect(await screen.findByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getByText('ada@example.org')).toBeInTheDocument();
    expect(screen.getByText('global_admin')).toBeInTheDocument();
    expect(screen.getByText('Oct 15, 2026, 8:00 AM')).toBeInTheDocument();
    expect(screen.getByText('Grace Hopper')).toBeInTheDocument();
    expect(screen.getByText('editor')).toBeInTheDocument();
    expect(screen.getByText('No')).toBeInTheDocument();
  });
});

describe('list page error state', () => {
  const pages: readonly (readonly [string, () => ReactElement])[] = [
    ['assets', AssetsPage],
    ['item sets', ItemSetsPage],
    ['jobs', JobsPage],
    ['media', () => <MediaPage param={null} />],
    ['modules', ModulesPage],
    ['properties', PropertiesPage],
    ['resource classes', ResourceClassesPage],
    ['item types', ItemTypesPage],
    ['users', UsersPage],
    ['vocabularies', VocabulariesPage],
  ];

  it.each(pages)('%s page shows the upstream error with its status', async (_name, Page) => {
    installFetch(() => jsonResponse({ errors: { error: 'Server error' } }, { status: 500 }));
    render(<Page />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Server error (HTTP 500)');
  });
});

