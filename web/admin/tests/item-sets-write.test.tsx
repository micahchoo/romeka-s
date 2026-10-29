/**
 * Item set write tests. Each form runs through the real client and parsers
 * against a stubbed fetch, so the method, URL and body a form sends are the
 * same bytes the server would get.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isUnknownArray, itemSetIdFromParam } from '../src/api/parse';
import { ItemSetForm } from '../src/pages/item-sets/form';
import { classRoutes } from './class-routes';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const itemSetFixture = (index: number): unknown => {
  const page = loadFixture('item-sets-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected an item set list fixture');
  }
  const entry = page[index];
  if (entry === undefined) {
    throw new Error(`expected an item set fixture at index ${String(index)}`);
  }
  return entry;
};

const optionsRoutes: Readonly<Record<string, () => Response>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  ...classRoutes,
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  '/api/item_sets/1': () => jsonResponse(itemSetFixture(0)),
};

const itemSetId = (): NonNullable<ReturnType<typeof itemSetIdFromParam>> => {
  const id = itemSetIdFromParam('1');
  if (id === null) {
    throw new Error('expected item set id 1');
  }
  return id;
};

const optionsHandler = (url: URL): Response => {
  const handler = optionsRoutes[url.pathname];
  return handler === undefined
    ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
    : handler();
};

describe('item set create form', () => {
  it('item_set_create_posts_the_title_visibility_and_open_flag', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/item_sets' && init?.method === 'POST') {
        return jsonResponse(itemSetFixture(1));
      }
      return optionsHandler(url);
    });
    render(<ItemSetForm itemSetId={null} />);

    fireEvent.change(await screen.findByLabelText('Title'), {
      target: { value: 'New item set' },
    });
    fireEvent.click(screen.getByLabelText('Open item set'));
    fireEvent.click(screen.getByRole('button', { name: 'Add item set' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();

    const post = first(
      requestsFor(fetchMock, '/api/item_sets').filter(
        (request) => request.init?.method === 'POST',
      ),
    );
    expect(post.init?.method).toBe('POST');
    expect(post.url.pathname).toBe('/api/item_sets');
    const body = requestBody(post);
    expect(body['o:is_public']).toBe(true);
    expect(body['o:is_open']).toBe(true);
    expect(body['o:owner']).toBeUndefined();
    expect(body['o:resource_class']).toEqual({ 'o:id': null });
    expect(body['o:resource_template']).toEqual({ 'o:id': null });
    expect(body['o:thumbnail']).toEqual({ 'o:id': null });
    expect(body['dcterms:title']).toEqual([
      { property_id: 1, type: 'literal', '@value': 'New item set', is_public: true },
    ]);
  });
});

describe('item set edit form', () => {
  it('item_set_edit_renders_the_stored_title_visibility_and_open_state', async () => {
    installFetch(optionsHandler);
    render(<ItemSetForm itemSetId={itemSetId()} />);

    expect(await screen.findByLabelText('Title')).toHaveValue('Featured photographs');
    expect(screen.getByRole('checkbox', { name: 'Public' })).toBeChecked();
    expect(screen.getByLabelText('Open item set')).toBeChecked();
  });

  it('item_set_edit_puts_the_full_resource_and_reads_the_saved_row', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/item_sets/1' && init?.method === 'PUT') {
        return jsonResponse(itemSetFixture(0));
      }
      return optionsHandler(url);
    });
    render(<ItemSetForm itemSetId={itemSetId()} />);

    fireEvent.change(await screen.findByLabelText('Title'), {
      target: { value: 'Renamed photographs' },
    });
    fireEvent.click(screen.getByLabelText('Open item set'));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();

    const put = first(
      requestsFor(fetchMock, '/api/item_sets/1').filter(
        (request) => request.init?.method === 'PUT',
      ),
    );
    expect(put.init?.method).toBe('PUT');
    const body = requestBody(put);
    expect(body['o:is_public']).toBe(true);
    expect(body['o:is_open']).toBe(false);
    expect(body['o:owner']).toEqual({ 'o:id': 3 });
    expect(body['o:resource_class']).toEqual({ 'o:id': null });
    expect(body['o:resource_template']).toEqual({ 'o:id': null });
    expect(body['dcterms:title']).toEqual([
      { property_id: 1, type: 'literal', '@value': 'Renamed photographs', is_public: true },
    ]);
  });

  it('item_set_edit_surfaces_a_403_permission_message', async () => {
    installFetch((url, init) => {
      if (url.pathname === '/api/item_sets/1' && init?.method === 'PUT') {
        return jsonResponse(
          {
            errors: {
              error:
                'Permission denied for the current user to update the Omeka\\Api\\Adapter\\ItemSetAdapter resource.',
            },
          },
          { status: 403 },
        );
      }
      return optionsHandler(url);
    });
    render(<ItemSetForm itemSetId={itemSetId()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Permission denied for the current user to update the Omeka\\Api\\Adapter\\ItemSetAdapter resource. (HTTP 403)',
    );
  });
});
