/**
 * Delete-control tests. Each test renders a real show page or edit form
 * through the real client and parsers against a stubbed fetch, so the request
 * it sends is the request the server would get. Every surface is covered: the
 * method and URL per resource, the confirmation gate that holds the request
 * back, the API message a refusal or a missing row renders, and the list the
 * page returns to after a success.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { Mock } from 'vitest';
import {
  isUnknownArray,
  itemSetIdFromParam,
  userIdFromParam,
} from '../src/api/parse';
import { Shell } from '../src/app/shell';
import { ItemSetForm } from '../src/pages/item-sets/form';
import { ItemsPage } from '../src/pages/items/page';
import { MediaPage } from '../src/pages/media/page';
import { SitesPage } from '../src/pages/sites/page';
import { UserForm } from '../src/pages/users/form';
import { editorReads } from './editorStubs';
import { loadFixture } from './loadFixture';
import {
  detached,
  first,
  installFetch,
  jsonResponse,
  listResponse,
  requestsFor,
  type CapturedRequest,
  type Handler,
} from './stub';

const notFound = (): Response =>
  jsonResponse({ errors: { error: 'Unexpected request' } }, { status: 404 });

const arrayEntry = (fixture: string, index: number): unknown => {
  const page = loadFixture(fixture);
  if (!isUnknownArray(page)) {
    throw new Error(`expected an array fixture: ${fixture}`);
  }
  const entry = page[index];
  if (entry === undefined) {
    throw new Error(`expected ${fixture} to have an entry at ${String(index)}`);
  }
  return entry;
};

const itemFixture = (): unknown => loadFixture('item-1.json');

const itemSetFixture = (): unknown => arrayEntry('item-sets-page.json', 0);

const userFixture = (): unknown => arrayEntry('users-page.json', 0);

const siteFixture = (): unknown => arrayEntry('sites-page.json', 0);

const pageFixture = (): unknown => loadFixture('site-page-1.json');

const mediaFixture = (): unknown => loadFixture('media-951.json');

/** The reference lists every resource form loads beside its own row. */
const formOptionsRoutes: Readonly<Record<string, Handler>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
};

const optionOrNotFound = (url: URL, init: RequestInit | undefined): Response => {
  const handler = formOptionsRoutes[url.pathname];
  return handler === undefined ? notFound() : handler(url, init);
};

/** The item show page: the row, its media list, and the form reference lists. */
const itemDetailHandler =
  (deleteResponse: () => Response): Handler =>
  (url, init) => {
    if (url.pathname === '/api/items/1') {
      if (init?.method === 'DELETE') {
        return deleteResponse();
      }
      return jsonResponse(itemFixture());
    }
    if (url.pathname === '/api/media') {
      return jsonResponse(loadFixture('media-page-live.json'), {
        headers: { 'Omeka-S-Total-Results': '3' },
      });
    }
    return optionOrNotFound(url, init);
  };

const deletedRequests = (
  fetchMock: Mock<typeof fetch>,
  pathname: string,
): readonly CapturedRequest[] =>
  requestsFor(fetchMock, pathname).filter((request) => request.init?.method === 'DELETE');

const itemSetId = itemSetIdFromParam('1');
const userId = userIdFromParam('3');
if (itemSetId === null || userId === null) {
  throw new Error('expected the fixture ids to parse');
}

describe('the confirmation gate', () => {
  it('delete_holds_the_request_until_the_confirmation_names_the_row', async () => {
    const fetchMock = installFetch(itemDetailHandler(() => jsonResponse(detached(itemFixture()))));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));

    expect(
      screen.getByText('Delete Fixture Item One? This cannot be undone.'),
    ).toBeInTheDocument();
    expect(deletedRequests(fetchMock, '/api/items/1')).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(deletedRequests(fetchMock, '/api/items/1')).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Fixture Item One' }));

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/items/1')).toHaveLength(1);
    });
    window.location.hash = '';
  });
});

describe('the request per resource', () => {
  it('item_delete_sends_delete_to_the_items_row_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(itemDetailHandler(() => jsonResponse(detached(itemFixture()))));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Fixture Item One' }));

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/items/1')).toHaveLength(1);
    });
    const request = first(deletedRequests(fetchMock, '/api/items/1'));
    expect(request.init?.method).toBe('DELETE');
    expect(request.url.pathname).toBe('/api/items/1');
    expect(window.location.hash).toBe('#/items');
    window.location.hash = '';
  });

  it('item_set_delete_sends_delete_to_the_item_sets_row_and_returns_to_the_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/item_sets/1') {
        return jsonResponse(init?.method === 'DELETE' ? detached(itemSetFixture()) : itemSetFixture());
      }
      return optionOrNotFound(url, init);
    });
    render(<ItemSetForm itemSetId={itemSetId} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item set' }));
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete Featured photographs' }),
    );

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/item_sets/1')).toHaveLength(1);
    });
    const request = first(deletedRequests(fetchMock, '/api/item_sets/1'));
    expect(request.init?.method).toBe('DELETE');
    expect(request.url.pathname).toBe('/api/item_sets/1');
    expect(window.location.hash).toBe('#/item-sets');
    window.location.hash = '';
  });

  it('media_delete_sends_delete_to_the_media_row_and_returns_to_the_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/media/951') {
        return jsonResponse(init?.method === 'DELETE' ? detached(mediaFixture()) : mediaFixture());
      }
      return notFound();
    });
    render(<MediaPage param="951" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this media' }));
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete BHC006_GMT_AF_010a.jpg' }),
    );

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/media/951')).toHaveLength(1);
    });
    const request = first(deletedRequests(fetchMock, '/api/media/951'));
    expect(request.init?.method).toBe('DELETE');
    expect(request.url.pathname).toBe('/api/media/951');
    expect(window.location.hash).toBe('#/media');
    window.location.hash = '';
  });

  it('user_delete_sends_delete_to_the_users_row_and_returns_to_the_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/users/3') {
        return jsonResponse(init?.method === 'DELETE' ? detached(userFixture()) : userFixture());
      }
      return notFound();
    });
    render(<UserForm userId={userId} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this user' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Ada Lovelace' }));

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/users/3')).toHaveLength(1);
    });
    const request = first(deletedRequests(fetchMock, '/api/users/3'));
    expect(request.init?.method).toBe('DELETE');
    expect(request.url.pathname).toBe('/api/users/3');
    expect(window.location.hash).toBe('#/users');
    window.location.hash = '';
  });

  it('site_delete_sends_delete_to_the_sites_row_and_returns_to_the_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(init?.method === 'DELETE' ? detached(siteFixture()) : siteFixture());
      }
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(pageFixture());
      }
      return optionOrNotFound(url, init);
    });
    render(<SitesPage param="1" rest={['1', 'settings']} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this site' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete My Site' }));

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/sites/1')).toHaveLength(1);
    });
    const request = first(deletedRequests(fetchMock, '/api/sites/1'));
    expect(request.init?.method).toBe('DELETE');
    expect(request.url.pathname).toBe('/api/sites/1');
    expect(window.location.hash).toBe('#/sites');
    window.location.hash = '';
  });

  it('site_page_delete_sends_delete_to_the_pages_row_and_returns_to_its_site', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(init?.method === 'DELETE' ? detached(pageFixture()) : pageFixture());
      }
      return editorReads(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this page' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Welcome' }));

    await waitFor(() => {
      expect(deletedRequests(fetchMock, '/api/site_pages/1')).toHaveLength(1);
    });
    const request = first(deletedRequests(fetchMock, '/api/site_pages/1'));
    expect(request.init?.method).toBe('DELETE');
    expect(request.url.pathname).toBe('/api/site_pages/1');
    expect(window.location.hash).toBe('#/sites/1');
    window.location.hash = '';
  });
});

describe('a refused or missing row', () => {
  it('delete_renders_the_api_403_message_and_stays_on_the_page', async () => {
    installFetch(
      itemDetailHandler(() =>
        jsonResponse(
          {
            errors: {
              error:
                'Permission denied for the current user to delete the Omeka\\Api\\Adapter\\ItemAdapter resource.',
            },
          },
          { status: 403 },
        ),
      ),
    );
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Fixture Item One' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Permission denied for the current user to delete the Omeka\\Api\\Adapter\\ItemAdapter resource. (HTTP 403)',
    );
    expect(window.location.hash).toBe('');
    // The page keeps its record and offers the action again rather than a blank
    // screen.
    expect(screen.getByRole('heading', { level: 2, name: 'Fixture Item One' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete this item' })).toBeInTheDocument();
  });

  it('delete_renders_the_not_found_message_for_a_missing_row', async () => {
    installFetch(
      itemDetailHandler(() =>
        jsonResponse(
          {
            errors: {
              error: 'Omeka\\Entity\\Item entity with criteria {"id":"1"} not found',
            },
          },
          { status: 404 },
        ),
      ),
    );
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Fixture Item One' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Omeka\\Entity\\Item entity with criteria {"id":"1"} not found (HTTP 404)',
    );
    expect(window.location.hash).toBe('');
  });
});

describe('an answer that is not detached', () => {
  it('delete_reports_the_unexpected_shape_and_stays_on_the_page', async () => {
    installFetch(itemDetailHandler(() => jsonResponse(itemFixture())));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Fixture Item One' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('$.o:id: expected null');
    expect(window.location.hash).toBe('');
    expect(screen.getByRole('button', { name: 'Delete this item' })).toBeInTheDocument();
    window.location.hash = '';
  });
});

describe('the list after a successful delete', () => {
  it('item_delete_navigates_to_the_list_and_reloads_it', async () => {
    window.location.hash = '#/items/1';
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/items/1') {
        return jsonResponse(init?.method === 'DELETE' ? detached(itemFixture()) : itemFixture());
      }
      if (url.pathname === '/api/items') {
        return listResponse(loadFixture('items-page.json'), 2);
      }
      if (url.pathname === '/api/media') {
        return jsonResponse(loadFixture('media-page-live.json'), {
          headers: { 'Omeka-S-Total-Results': '3' },
        });
      }
      if (url.pathname === '/api') {
        return jsonResponse([]);
      }
      return optionOrNotFound(url, init);
    });
    render(<Shell />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Fixture Item One' }));

    // The shell routed back to `#/items`, mounted the list and reloaded it.
    expect(await screen.findByRole('link', { name: 'Fixture Item One' })).toHaveAttribute(
      'href',
      '#/items/12',
    );
    expect(window.location.hash).toBe('#/items');
    expect(
      requestsFor(fetchMock, '/api/items').filter(
        (request) => request.init?.method !== 'DELETE',
      ),
    ).toHaveLength(1);
    window.location.hash = '';
  });
});
