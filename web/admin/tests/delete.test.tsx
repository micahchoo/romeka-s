/**
 * Delete-control tests. Each test renders a real show page or edit form
 * through the real client and parsers against a stubbed fetch, so the request
 * it sends is the request the server would get. Every surface is covered: the
 * method and URL per resource, the confirmation gate that holds the request
 * back, the API message a refusal or a missing row renders, and the list the
 * page returns to after a success. The item page's, the media page's and the
 * item set's delete move the record to the trash (a bulk delete of it alone)
 * and never send DELETE.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { Mock } from 'vitest';
import {
  isRecord,
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
import { classRoutes } from './class-routes';
import { loadFixture } from './loadFixture';
import {
  detached,
  first,
  installFetch,
  jsonResponse,
  listResponse,
  requestBody,
  requestsFor,
  signedIn,
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
  ...classRoutes,
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
};

const optionOrNotFound = (url: URL, init: RequestInit | undefined): Response => {
  const handler = formOptionsRoutes[url.pathname];
  return handler === undefined ? notFound() : handler(url, init);
};

/** A one-item delete preview of item 1, as `POST /api/romeka/bulk` answers it. */
const itemPreview = (overrides: Record<string, unknown> = {}): Record<string, unknown> => ({
  id: 7,
  status: 'previewed',
  kind: 'delete',
  total: 1,
  denied: 0,
  media: 0,
  bytes: 0,
  trashDays: 30,
  samples: [{ id: 1, title: 'Fixture Item One' }],
  ...overrides,
});

const trashedRun = (): unknown => {
  const done = loadFixture('bulk/delete-done.json');
  return {
    ...(isRecord(done) ? done : {}),
    id: 7,
    query: { id: [1] },
    total: 1,
    done: 1,
    counts: { pending: 0, done: 1, unchanged: 0, failed: 0, restored: 0, kept: 0 },
  };
};

/** The bulk routes behind the item page's delete: the preview `preview` answers, the run, the done run. */
const trashRoutes = (preview: () => Response): Readonly<Record<string, Handler>> => ({
  '/api/romeka/bulk': preview,
  '/api/romeka/bulk/7/run': () => jsonResponse({ jobId: 3 }),
  '/api/romeka/bulk/7': () => jsonResponse(trashedRun()),
});

/** The item show page: the row, its media list, what links to it, the form reference lists, and the trash. */
const itemDetailHandler =
  (preview: () => Response): Handler =>
  (url, init) => {
    if (url.pathname === '/api/items/1') {
      return init?.method === 'DELETE' ? notFound() : jsonResponse(itemFixture());
    }
    if (url.pathname === '/api/romeka/links/1') {
      return jsonResponse(loadFixture('places/links-none.json'));
    }
    if (url.pathname === '/api/media') {
      return jsonResponse(loadFixture('media-page-live.json'), {
        headers: { 'Omeka-S-Total-Results': '3' },
      });
    }
    const trash = trashRoutes(preview)[url.pathname];
    return trash === undefined ? optionOrNotFound(url, init) : trash(url, init);
  };

const previewed = (): Response => jsonResponse(itemPreview(), { status: 201 });

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
    const fetchMock = installFetch(itemDetailHandler(previewed));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));

    expect(await screen.findByText('Move Fixture Item One to the trash?')).toBeInTheDocument();
    expect(requestsFor(fetchMock, '/api/romeka/bulk/7/run')).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(requestsFor(fetchMock, '/api/romeka/bulk/7/run')).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Move to the trash' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/romeka/bulk/7/run')).toHaveLength(1);
    });
    expect(deletedRequests(fetchMock, '/api/items/1')).toHaveLength(0);
    window.location.hash = '';
  });
});

describe('the request per resource', () => {
  it('item_delete_moves_the_item_alone_to_the_trash_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(itemDetailHandler(previewed));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Move to the trash' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/items');
    });
    expect(requestBody(first(requestsFor(fetchMock, '/api/romeka/bulk')))).toEqual({
      query: { id: [1] },
      change: { kind: 'delete' },
    });
    expect(requestsFor(fetchMock, '/api/romeka/bulk/7/run')).toHaveLength(1);
    expect(deletedRequests(fetchMock, '/api/items/1')).toHaveLength(0);
    window.location.hash = '';
  });

  it('item_set_delete_moves_the_set_alone_to_the_trash_and_returns_to_the_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/item_sets/1') {
        return init?.method === 'DELETE' ? notFound() : jsonResponse(itemSetFixture());
      }
      if (url.pathname === '/api/romeka/links/1') {
        return jsonResponse(loadFixture('places/links-none.json'));
      }
      const trash = trashRoutes(() =>
        jsonResponse(itemPreview({ samples: [{ id: 1, title: 'Featured photographs' }] }), { status: 201 }),
      )[url.pathname];
      return trash === undefined ? optionOrNotFound(url, init) : trash(url, init);
    });
    render(<ItemSetForm itemSetId={itemSetId} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item set' }));
    expect(await screen.findByText('Move Featured photographs to the trash?')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Move to the trash' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/item-sets');
    });
    expect(requestBody(first(requestsFor(fetchMock, '/api/romeka/bulk')))).toEqual({
      query: { id: [1] },
      change: { kind: 'delete', of: 'item_sets' },
    });
    expect(deletedRequests(fetchMock, '/api/item_sets/1')).toHaveLength(0);
    window.location.hash = '';
  });

  it('media_delete_moves_the_media_alone_to_the_trash_and_returns_to_the_list', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/media/951') {
        return init?.method === 'DELETE' ? notFound() : jsonResponse(mediaFixture());
      }
      if (url.pathname === '/api/romeka/links/951') {
        return jsonResponse(loadFixture('places/links-none.json'));
      }
      const trash = trashRoutes(() =>
        jsonResponse(itemPreview({ media: 1, bytes: 2048, samples: [{ id: 951, title: null }] }), { status: 201 }),
      )[url.pathname];
      return trash === undefined ? notFound() : trash(url, init);
    });
    render(<MediaPage param="951" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this media' }));
    expect(await screen.findByText('Move BHC006_GMT_AF_010a.jpg to the trash?')).toBeInTheDocument();
    expect(screen.getByText('Its file goes with it: 2 kB.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Move to the trash' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/media');
    });
    expect(requestBody(first(requestsFor(fetchMock, '/api/romeka/bulk')))).toEqual({
      query: { id: [951] },
      change: { kind: 'delete', of: 'media' },
    });
    expect(deletedRequests(fetchMock, '/api/media/951')).toHaveLength(0);
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
  it('delete_states_a_refused_preview_and_stays_on_the_page', async () => {
    installFetch(
      itemDetailHandler(() =>
        jsonResponse(
          {
            errors: {
              error:
                'Permission denied for the current user to batch_delete the Omeka\\Api\\Adapter\\ItemAdapter resource.',
            },
          },
          { status: 403 },
        ),
      ),
    );
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('You may not delete this item.');
    expect(window.location.hash).toBe('');
    // The page keeps its record rather than a blank screen.
    expect(screen.getByRole('heading', { level: 2, name: 'Fixture Item One' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Move to the trash' })).not.toBeInTheDocument();
  });

  it('delete_states_a_missing_row_as_not_available', async () => {
    installFetch(itemDetailHandler(() => jsonResponse(itemPreview({ total: 0, samples: [] }), { status: 201 })));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('This record is not available.');
    expect(screen.queryByRole('button', { name: 'Move to the trash' })).not.toBeInTheDocument();
    expect(window.location.hash).toBe('');
  });
});

describe('an answer that is not a preview', () => {
  it('delete_reports_the_unexpected_shape_and_stays_on_the_page', async () => {
    installFetch(itemDetailHandler(() => jsonResponse(itemFixture(), { status: 201 })));
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));

    expect(
      await screen.findByText('Cannot check what goes with this item, so it cannot move to the trash now.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Move to the trash' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check again' })).toBeInTheDocument();
    expect(window.location.hash).toBe('');
  });
});

describe('the list after a successful delete', () => {
  it('item_delete_navigates_to_the_list_reloads_it_and_offers_the_undo', async () => {
    window.location.hash = '#/items/1';
    const trash = trashRoutes(previewed);
    const fetchMock = installFetch(signedIn((url, init) => {
      if (url.pathname === '/api/items/1') {
        return jsonResponse(itemFixture());
      }
      if (url.pathname === '/api/items') {
        return listResponse(loadFixture('items-page.json'), 2);
      }
      if (url.pathname === '/api/romeka/links/1') {
        return jsonResponse(loadFixture('places/links-none.json'));
      }
      if (url.pathname === '/api/media') {
        return jsonResponse(loadFixture('media-page-live.json'), {
          headers: { 'Omeka-S-Total-Results': '3' },
        });
      }
      if (url.pathname === '/api') {
        return jsonResponse([]);
      }
      const handler = trash[url.pathname];
      return handler === undefined ? optionOrNotFound(url, init) : handler(url, init);
    }));
    render(<Shell />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this item' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Move to the trash' }));

    // The shell routed back to `#/items`, mounted the list, reloaded it, and offers the undo.
    expect(await screen.findByRole('link', { name: 'Fixture Item One' }, { timeout: 3000 })).toHaveAttribute(
      'href',
      '#/items/12?in=&at=1',
    );
    expect(window.location.hash).toBe('#/items');
    expect(await screen.findByText('Deleted Fixture Item One.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
    expect(requestsFor(fetchMock, '/api/items')).toHaveLength(1);
    window.location.hash = '';
  });
});
