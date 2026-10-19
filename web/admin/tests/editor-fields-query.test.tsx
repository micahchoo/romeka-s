/**
 * The `itemQuery` field control, through a live `BlockForm`: the builder writes
 * the advanced-search pairs a `browsePreview` block stores, keeps a pair it does
 * not show, and reads the match count from `Omeka-S-Total-Results`.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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

const handler = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/items':
      return listResponse([], TOTAL);
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

  it('says why the block shows nothing when no item of the site matches', async () => {
    installFetch((url) => (url.pathname === '/api/items' ? listResponse([], 0) : handler(url)));
    renderQuery('');

    expect(
      await screen.findByText(/No items in this site match, so the block shows nothing\./),
    ).toBeInTheDocument();
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

    fireEvent.change(screen.getByLabelText('Resource class'), { target: { value: '13' } });

    expect(queryAction(dispatch)).toEqual({
      type: 'setField',
      blockKey: key,
      field: 'query',
      value: 'resource_class_id=13',
    });
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
