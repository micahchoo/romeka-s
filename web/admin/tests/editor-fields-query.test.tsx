/**
 * The `itemQuery` field control, through a live `BlockForm`: the builder writes
 * the advanced-search pairs a `browsePreview` block stores, keeps a pair it does
 * not show, and reads the match count from `Omeka-S-Total-Results`.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { DraftAction } from '../src/pages/sites/editor/draft';
import {
  actionsOf,
  BlockFormHarness,
  captureBlock,
  dispatchSpy,
  SITE_ID,
} from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestsFor } from './stub';

const TOTAL = 7;

/** `/api/romeka/classes/counts`: the classes items use, with their item counts. */
const CLASS_COUNTS = {
  classes: [
    { id: 51, term: 'bibo:Book', label: 'Book', vocabulary_label: 'Bibliographic Ontology', items: 4 },
    { id: 21, term: 'dctype:Image', label: 'Image', vocabulary_label: 'Dublin Core Type', items: 3 },
    { id: 41, term: 'schema:Image', label: 'Image', vocabulary_label: 'Schema.org', items: 1 },
  ],
};

const handler = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/items':
      return listResponse([], TOTAL);
    case '/api/romeka/classes/counts':
      return jsonResponse(CLASS_COUNTS);
    case '/api/item_sets':
      return jsonResponse(loadFixture('item-sets-page.json'));
    case '/api/resource_classes':
      return jsonResponse(loadFixture('resource-classes-page.json'));
    default:
      return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  }
};

const renderQuery = (query: string) => {
  const dispatch = dispatchSpy();
  const captured = captureBlock();
  render(
    <BlockFormHarness
      layout="browsePreview"
      data={{ resource_type: 'items', query, components: [] }}
      onAction={dispatch}
      onBlock={captured.onBlock}
    />,
  );
  return { dispatch, key: captured.key() };
};

const queryAction = (dispatch: ReturnType<typeof dispatchSpy>): DraftAction => {
  const action = actionsOf(dispatch)[0];
  if (action === undefined) {
    throw new Error('expected a dispatched action');
  }
  return action;
};

describe('the item query field control', () => {
  it('counts the items of the block\'s own site, as the public render does', async () => {
    const fetchMock = installFetch(handler);
    renderQuery('');

    expect(await screen.findByText('7 items in this site match.')).toBeInTheDocument();
    const sent = requestsFor(fetchMock, '/api/items');
    expect(sent.map((request) => request.url.searchParams.get('site_id'))).toContain(
      String(SITE_ID),
    );
  });

  it('offers a way to add items when the site has none', async () => {
    installFetch((url) => (url.pathname === '/api/items' ? listResponse([], 0) : handler(url)));
    renderQuery('fulltext_search=cat');

    expect(await screen.findByText('This site has no items yet.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Add items to this site' })).toHaveAttribute(
      'href',
      `#/sites/${String(SITE_ID)}/resources`,
    );
  });

  it('blames the filter, with no add link, when the site has items but none match', async () => {
    // The filtered count is 0; the site's own count, asked without the filter, is 7.
    installFetch((url) =>
      url.pathname === '/api/items' && url.searchParams.has('fulltext_search')
        ? listResponse([], 0)
        : handler(url),
    );
    renderQuery('fulltext_search=cat');

    expect(await screen.findByText('No items on this site match this filter.')).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'Add items to this site' })).toBeNull();
  });

  it('full-text search writes fulltext_search', () => {
    installFetch(handler);
    const { dispatch, key } = renderQuery('');

    fireEvent.change(screen.getByLabelText('Full-text search'), { target: { value: 'cat' } });

    expect(queryAction(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'query',
      value: 'fulltext_search=cat',
    });
  });

  it('the count request carries the typed full-text search', async () => {
    const fetchMock = installFetch(handler);
    renderQuery('');

    fireEvent.change(screen.getByLabelText('Full-text search'), { target: { value: 'cat' } });

    await waitFor(() => {
      const sent = requestsFor(fetchMock, '/api/items');
      expect(sent.some((request) => request.url.searchParams.get('fulltext_search') === 'cat')).toBe(
        true,
      );
    });
  });

  it('the item set select writes item_set_id', async () => {
    installFetch(handler);
    const { dispatch, key } = renderQuery('');
    await screen.findByText('7 items in this site match.');

    fireEvent.change(screen.getByLabelText('Item set'), { target: { value: '2' } });

    expect(queryAction(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'query',
      value: 'item_set_id=2',
    });
  });

  it('the class select writes resource_class_id', async () => {
    installFetch(handler);
    const { dispatch, key } = renderQuery('');
    await screen.findByText('7 items in this site match.');

    const classSelect = screen.getByLabelText('Class');
    await within(classSelect).findByRole('option', { name: 'Book · 4 items' });
    fireEvent.change(classSelect, { target: { value: '41' } });

    expect(queryAction(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'query',
      value: 'resource_class_id=41',
    });
  });

  it('the class select offers only the classes items use, each with its count, a shared name with its standard', async () => {
    const fetchMock = installFetch(handler);
    renderQuery('');
    const classSelect = screen.getByLabelText('Class');

    await within(classSelect).findByRole('option', { name: 'Book · 4 items' });
    expect(within(classSelect).getAllByRole('option').map((option) => option.textContent)).toEqual([
      '[Any]',
      'Book · 4 items',
      'Image (Dublin Core Type) · 3 items',
      'Image (Schema.org) · 1 item',
    ]);
    expect(requestsFor(fetchMock, '/api/resource_classes'), 'no list of every class').toEqual([]);
  });

  it('a stored class no item uses stays selected, at 0 items', async () => {
    installFetch((url) => {
      switch (url.pathname) {
        case '/api/resource_classes/13':
          return jsonResponse(loadFixture('resource-class-13.json'));
        case '/api/vocabularies/2':
          return jsonResponse(loadFixture('vocabulary-2.json'));
        default:
          return handler(url);
      }
    });
    renderQuery('resource_class_id=13');
    const classSelect = screen.getByLabelText('Class');

    const held = await within(classSelect).findByRole('option', { name: 'Still Image · 0 items' });
    expect(held).toHaveProperty('selected', true);
    expect(classSelect).toHaveValue('13');
  });

  it('counts with the whole stored query, so a property filter the builder does not show narrows the count', async () => {
    const stored =
      'property%5B0%5D%5Bproperty%5D=1&property%5B0%5D%5Btype%5D=eq&property%5B0%5D%5Btext%5D=harbour&owner_id=2&fulltext_search=cat';
    const fetchMock = installFetch((url) =>
      url.pathname === '/api/items' && url.searchParams.get('property[0][type]') === 'eq'
        ? listResponse([], 3)
        : handler(url),
    );
    renderQuery(stored);

    expect(await screen.findByText('3 items in this site match.')).toBeInTheDocument();
    const counted = requestsFor(fetchMock, '/api/items')[0]?.url.searchParams;
    expect(counted === undefined ? [] : [...counted]).toEqual([
      ['property[0][property]', '1'],
      ['property[0][type]', 'eq'],
      ['property[0][text]', 'harbour'],
      ['owner_id', '2'],
      ['fulltext_search', 'cat'],
      ['site_id', String(SITE_ID)],
      ['per_page', '1'],
    ]);
  });

  it('for item sets, the class select lists every class, since the counts count items', async () => {
    installFetch((url) => (url.pathname === '/api/item_sets' ? listResponse([], 2) : handler(url)));
    render(
      <BlockFormHarness
        layout="browsePreview"
        data={{ resource_type: 'item_sets', query: '', components: [] }}
      />,
    );
    const classSelect = screen.getByLabelText('Class');

    await within(classSelect).findByRole('option', { name: 'Still Image' });
    expect(within(classSelect).getByRole('option', { name: 'Book' })).toBeInTheDocument();
  });

  it('the sort selects write sort_by and sort_order', () => {
    installFetch(handler);
    const { dispatch, key } = renderQuery('');

    fireEvent.change(screen.getByLabelText('Sort by'), { target: { value: 'title' } });
    fireEvent.change(screen.getByLabelText('Sort order'), { target: { value: 'desc' } });

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: key, field: 'query', value: 'sort_by=title' },
      {
        type: 'setField',
        blockKey: key,
        field: 'query',
        value: 'sort_by=title&sort_order=desc',
      },
    ]);
  });

  it('keeps an advanced-search pair the builder does not show', () => {
    installFetch(handler);
    const { dispatch, key } = renderQuery('year=2000&item_set_id=2');

    fireEvent.change(screen.getByLabelText('Full-text search'), { target: { value: 'cat' } });

    expect(queryAction(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'query',
      value: 'year=2000&item_set_id=2&fulltext_search=cat',
    });
  });

  it('reports a count request that fails', async () => {
    installFetch(() => jsonResponse({ errors: { error: 'boom' } }, { status: 500 }));
    renderQuery('');

    expect(await screen.findByText('The match count could not be read.')).toBeInTheDocument();
  });
});
