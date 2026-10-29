/**
 * Item write tests: create through the add form and update through the edit
 * panel of the show page. Both run through the real client and parsers.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { Shell } from '../src/app/shell';
import { ItemsPage } from '../src/pages/items/page';
import { classRoutes, classesWithTwin, vocabulariesWithSchema } from './class-routes';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor, route, signedIn } from './stub';

const itemFixture = (): Record<string, unknown> => {
  const fixture = loadFixture('item-1.json');
  if (!isRecord(fixture)) {
    throw new Error('expected an item fixture');
  }
  return fixture;
};

const optionsRoutes: Readonly<Record<string, () => Response>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  ...classRoutes,
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  '/api/sites': () => jsonResponse(loadFixture('sites-page.json')),
};

const detailHandler = (url: URL): Response => {
  if (url.pathname === '/api/items/1') {
    return jsonResponse(itemFixture());
  }
  if (url.pathname === '/api/media') {
    return jsonResponse(loadFixture('media-page-live.json'), {
      headers: { 'Omeka-S-Total-Results': '3' },
    });
  }
  const handler = optionsRoutes[url.pathname];
  return handler === undefined
    ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
    : handler();
};

describe('item create form', () => {
  it('item_create_posts_the_title_item_set_and_defaults', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/items' && init?.method === 'POST') {
        return jsonResponse(itemFixture());
      }
      const handler = optionsRoutes[url.pathname];
      return handler === undefined
        ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
        : handler();
    });
    render(<ItemsPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Title'), {
      target: { value: 'Created item' },
    });
    fireEvent.click(screen.getByLabelText('Featured photographs'));
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items').filter(
          (request) => request.init?.method === 'POST',
        ),
      ).toHaveLength(1);
    });
    const post = requestsFor(fetchMock, '/api/items').find(
      (request) => request.init?.method === 'POST',
    );
    if (post === undefined) {
      throw new Error('expected a POST to /api/items');
    }
    expect(post.init?.method).toBe('POST');
    const body = requestBody(post);
    expect(body['o:is_public']).toBe(true);
    expect(body['o:owner']).toBeUndefined();
    expect(body['o:resource_class']).toEqual({ 'o:id': null });
    expect(body['o:resource_template']).toEqual({ 'o:id': null });
    expect(body['o:item_set']).toEqual([{ 'o:id': 1 }]);
    expect(body['o:primary_media']).toEqual({ 'o:id': null });
    // No `o:site`, so the sites that keep in sync with their search can take the item.
    expect(body['o:site']).toBeUndefined();
    expect(body['dcterms:title']).toEqual([
      { property_id: 1, type: 'literal', '@value': 'Created item', is_public: true },
    ]);
    expect(window.location.hash).toBe('#/items/1');
    window.location.hash = '';
  });
});

describe('item form class select', () => {
  it('names two classes that share a label each with its standard', async () => {
    installFetch(
      route({
        ...optionsRoutes,
        '/api/resource_classes': () => jsonResponse(classesWithTwin()),
        '/api/vocabularies': () => jsonResponse(vocabulariesWithSchema()),
      }),
    );
    render(<ItemsPage param="new" />);

    await screen.findByLabelText('Title');
    const classSelect = screen.getByRole('combobox', { name: 'Class', hidden: true });
    expect(within(classSelect).getByRole('option', { name: 'Still Image (Dublin Core Type)', hidden: true })).toBeInTheDocument();
    expect(within(classSelect).getByRole('option', { name: 'Still Image (Schema.org)', hidden: true })).toBeInTheDocument();
    expect(within(classSelect).getByRole('option', { name: 'Book', hidden: true })).toBeInTheDocument();
  });
});

describe('item show page after a create', () => {
  it('item_create_renders_the_new_item_without_reading_it_back', async () => {
    const fetchMock = installFetch(signedIn((url, init) => {
      if (url.pathname === '/api/items' && init?.method === 'POST') {
        return jsonResponse(itemFixture());
      }
      return detailHandler(url);
    }));
    window.location.hash = '#/items/new';
    render(<Shell />);

    fireEvent.change(await screen.findByLabelText('Title'), {
      target: { value: 'Created item' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));

    // The show page the create navigated to renders the record the POST
    // returned, so it never reads that item back.
    await screen.findByRole('heading', { name: 'Fixture Item One', level: 2 });
    expect(requestsFor(fetchMock, '/api/items/1')).toHaveLength(0);
    window.location.hash = '';
  });
});

describe('item edit form', () => {
  it('item_edit_renders_values_and_puts_the_whole_resource', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/items/1' && init?.method === 'PUT') {
        return jsonResponse({
          ...itemFixture(),
          'o:title': 'Fixture Item One edited',
          'dcterms:title': [
            {
              type: 'literal',
              property_id: 1,
              property_label: 'Title',
              is_public: true,
              '@value': 'Fixture Item One edited',
              '@language': 'en',
            },
          ],
        });
      }
      return detailHandler(url);
    });
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByText('Edit this item'));

    const titleInput = await screen.findByRole('textbox', { name: 'Title' });
    expect(titleInput).toHaveValue('Fixture Item One');
    expect(screen.getByDisplayValue('http://example.org/subjects/fixtures')).toBeInTheDocument();
    expect(screen.getByDisplayValue('Fixtures')).toBeInTheDocument();
    expect(screen.getByLabelText('dcterms:relation resource ID')).toHaveValue('9');

    fireEvent.change(titleInput, { target: { value: 'Fixture Item One edited' } });
    // The "Photograph" template names dcterms:description "Caption", in English.
    fireEvent.click(screen.getByRole('button', { name: 'Add value to Caption' }));
    const captions = screen.getAllByRole('textbox', { name: 'Caption' });
    const added = captions.at(-1);
    if (added === undefined) {
      throw new Error('expected the added caption input');
    }
    fireEvent.change(added, { target: { value: 'A description' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items/1').filter(
          (request) => request.init?.method === 'PUT',
        ),
      ).toHaveLength(1);
    });
    const put = requestsFor(fetchMock, '/api/items/1').find(
      (request) => request.init?.method === 'PUT',
    );
    if (put === undefined) {
      throw new Error('expected a PUT to /api/items/1');
    }
    expect(put.init?.method).toBe('PUT');
    const body = requestBody(put);
    expect(body['o:is_public']).toBe(false);
    expect(body['o:owner']).toEqual({ 'o:id': 3 });
    expect(body['o:resource_class']).toEqual({ 'o:id': 13 });
    expect(body['o:resource_template']).toEqual({ 'o:id': 2 });
    expect(body['o:thumbnail']).toEqual({ 'o:id': null });
    expect(body['o:item_set']).toEqual([{ 'o:id': 4 }]);
    expect(body['o:primary_media']).toEqual({ 'o:id': 7 });
    expect(body['dcterms:title']).toEqual([
      {
        property_id: 1,
        type: 'literal',
        '@value': 'Fixture Item One edited',
        '@language': 'en',
        is_public: true,
      },
    ]);
    expect(body['dcterms:subject']).toEqual([
      {
        property_id: 4,
        type: 'uri',
        '@id': 'http://example.org/subjects/fixtures',
        'o:label': 'Fixtures',
        is_public: true,
      },
    ]);
    expect(body['dcterms:relation']).toEqual([
      { property_id: 12, type: 'resource:item', value_resource_id: 9, is_public: true },
    ]);
    expect(body['dcterms:description']).toEqual([
      {
        property_id: 4,
        type: 'literal',
        '@value': 'A description',
        '@language': 'en',
        is_public: true,
      },
    ]);

    expect(
      await screen.findByRole('heading', { name: 'Fixture Item One edited', level: 2 }),
    ).toBeInTheDocument();
  });

  it('item_edit_removes_a_value_the_user_deletes', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/items/1' && init?.method === 'PUT') {
        return jsonResponse(itemFixture());
      }
      return detailHandler(url);
    });
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByText('Edit this item'));
    await screen.findByRole('textbox', { name: 'Title' });
    const subject = screen.getByDisplayValue('http://example.org/subjects/fixtures').closest('li');
    if (subject === null) {
      throw new Error('expected the subject value row');
    }
    fireEvent.click(within(subject).getByRole('button', { name: /^Remove .* value$/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items/1').filter(
          (request) => request.init?.method === 'PUT',
        ),
      ).toHaveLength(1);
    });
    const put = requestsFor(fetchMock, '/api/items/1').find(
      (request) => request.init?.method === 'PUT',
    );
    if (put === undefined) {
      throw new Error('expected a PUT to /api/items/1');
    }
    const body = requestBody(put);
    expect(body['dcterms:subject']).toBeUndefined();
    expect(body['dcterms:relation']).toEqual([
      { property_id: 12, type: 'resource:item', value_resource_id: 9, is_public: true },
    ]);
  });

  it('item_edit_keeps_the_sites_the_item_belongs_to', async () => {
    // A full `PUT` replaces `o:site`, so an edit that left it out would take the
    // item off every site the editor may assign.
    const onSites = {
      ...itemFixture(),
      'o:site': [{ '@id': 'http://localhost/api/sites/5', 'o:id': 5 }],
    };
    const fetchMock = installFetch((url) =>
      url.pathname === '/api/items/1' ? jsonResponse(onSites) : detailHandler(url),
    );
    render(<ItemsPage param="1" />);

    fireEvent.click(await screen.findByText('Edit this item'));
    fireEvent.change(await screen.findByLabelText('Title'), { target: { value: 'Renamed' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items/1').filter((request) => request.init?.method === 'PUT'),
      ).toHaveLength(1);
    });
    const put = requestsFor(fetchMock, '/api/items/1').find(
      (request) => request.init?.method === 'PUT',
    );
    if (put === undefined) {
      throw new Error('expected a PUT to /api/items/1');
    }
    expect(requestBody(put)['o:site']).toEqual([{ 'o:id': 5 }]);
  });
});

/** The PUT or POST the form sent to `path`. */
const sentBody = async (
  fetchMock: ReturnType<typeof installFetch>,
  path: string,
  method: 'POST' | 'PUT',
): Promise<Record<string, unknown>> => {
  await waitFor(() => {
    expect(
      requestsFor(fetchMock, path).filter((request) => request.init?.method === method),
    ).toHaveLength(1);
  });
  const sent = requestsFor(fetchMock, path).find((request) => request.init?.method === method);
  if (sent === undefined) {
    throw new Error(`expected a ${method} to ${path}`);
  }
  return requestBody(sent);
};

const onSites = (...ids: number[]): Record<string, unknown> => ({
  ...itemFixture(),
  'o:site': ids.map((id) => ({ '@id': `http://localhost/api/sites/${String(id)}`, 'o:id': id })),
});

describe('item sites control', () => {
  it('item_edit_checks_its_sites_and_puts_the_ones_chosen', async () => {
    const fetchMock = installFetch((url) =>
      url.pathname === '/api/items/1' ? jsonResponse(onSites(1)) : detailHandler(url),
    );
    render(<ItemsPage param="1" />);
    fireEvent.click(await screen.findByText('Edit this item'));

    const sites = await screen.findByRole('group', { name: 'Sites' });
    expect(within(sites).getByLabelText('My Site')).toBeChecked();
    expect(within(sites).getByLabelText('Second Site')).not.toBeChecked();
    fireEvent.click(within(sites).getByLabelText('My Site'));
    fireEvent.click(within(sites).getByLabelText('Second Site'));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect((await sentBody(fetchMock, '/api/items/1', 'PUT'))['o:site']).toEqual([{ 'o:id': 2 }]);
  });

  it('item_create_starts_with_no_site_and_posts_the_ones_chosen', async () => {
    const fetchMock = installFetch(signedIn((url, init) => {
      if (url.pathname === '/api/items' && init?.method === 'POST') {
        return jsonResponse(itemFixture());
      }
      return detailHandler(url);
    }));
    // Through the shell, so the show page the create opens takes the record it returned.
    window.location.hash = '#/items/new';
    render(<Shell />);

    const sites = await screen.findByRole('group', { name: 'Sites' });
    expect(within(sites).getByText(/every site that keeps in sync with its search/)).toBeVisible();
    expect(within(sites).getByLabelText('My Site')).not.toBeChecked();
    fireEvent.click(within(sites).getByLabelText('Second Site'));
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));

    expect((await sentBody(fetchMock, '/api/items', 'POST'))['o:site']).toEqual([{ 'o:id': 2 }]);
    await screen.findByRole('heading', { name: 'Fixture Item One', level: 2 });
    window.location.hash = '';
  });

  /**
   * `Acl::userIsAllowed($site, 'can-assign-items')`: an editor may assign to a
   * site they own or hold an admin or editor role on. The server keeps a
   * membership the user may not change, so the form sends it back unchanged.
   */
  it('item_form_offers_only_the_sites_the_user_may_assign_and_keeps_the_rest', async () => {
    const users = loadFixture('users-page.json');
    if (!isUnknownArray(users)) {
      throw new Error('expected a users page');
    }
    setSession(parseUser(users[1]));
    const sitesPage = loadFixture('sites-page.json');
    if (!isUnknownArray(sitesPage) || !isRecord(sitesPage[1])) {
      throw new Error('expected a sites page');
    }
    const editable = {
      ...sitesPage[1],
      'o:site_permission': [
        { 'o:user': { '@id': 'http://localhost/api/users/5', 'o:id': 5 }, 'o:role': 'editor' },
      ],
    };
    const fetchMock = installFetch((url) => {
      if (url.pathname === '/api/items/1') {
        return jsonResponse(onSites(1));
      }
      if (url.pathname === '/api/sites') {
        return jsonResponse([sitesPage[0], editable]);
      }
      return detailHandler(url);
    });
    try {
      render(<ItemsPage param="1" />);
      fireEvent.click(await screen.findByText('Edit this item'));

      const sites = await screen.findByRole('group', { name: 'Sites' });
      expect(within(sites).queryByLabelText('My Site')).toBeNull();
      fireEvent.click(within(sites).getByLabelText('Second Site'));
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));

      expect((await sentBody(fetchMock, '/api/items/1', 'PUT'))['o:site']).toEqual([
        { 'o:id': 1 },
        { 'o:id': 2 },
      ]);
    } finally {
      setSession(null);
    }
  });
});

/**
 * `common/resource-fields.phtml`: the owner select shows only when a stored
 * resource is edited and the user holds `change-owner`, which only the two
 * admin roles do. A full update still sends the stored owner, which the server
 * reads as no change.
 */
describe('item owner select', () => {
  const signIn = (index: number): void => {
    const users = loadFixture('users-page.json');
    if (!isUnknownArray(users)) {
      throw new Error('expected a users page');
    }
    setSession(parseUser(users[index]));
  };

  it('resource_fields_offers_the_owner_to_an_admin_editing_an_item', async () => {
    signIn(0);
    try {
      installFetch(detailHandler);
      render(<ItemsPage param="1" />);
      fireEvent.click(await screen.findByText('Edit this item'));
      expect(await screen.findByRole('combobox', { name: 'Owner' })).toHaveValue('3');
    } finally {
      setSession(null);
    }
  });

  it('resource_fields_hides_the_owner_from_an_editor_and_keeps_the_stored_one', async () => {
    signIn(1);
    try {
      const fetchMock = installFetch((url, init) => {
        if (url.pathname === '/api/items/1' && init?.method === 'PUT') {
          return jsonResponse(itemFixture());
        }
        return detailHandler(url);
      });
      render(<ItemsPage param="1" />);
      fireEvent.click(await screen.findByText('Edit this item'));
      await screen.findByRole('textbox', { name: 'Title' });
      expect(screen.queryByRole('combobox', { name: 'Owner' })).toBeNull();
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      expect((await sentBody(fetchMock, '/api/items/1', 'PUT'))['o:owner']).toEqual({ 'o:id': 3 });
    } finally {
      setSession(null);
    }
  });

  it('resource_fields_offers_no_owner_on_a_create', async () => {
    signIn(0);
    try {
      installFetch((url) => {
        const handler = optionsRoutes[url.pathname];
        return handler === undefined
          ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
          : handler();
      });
      render(<ItemsPage param="new" />);
      await screen.findByLabelText('Title');
      expect(screen.queryByRole('combobox', { name: 'Owner' })).toBeNull();
    } finally {
      setSession(null);
    }
  });
});
