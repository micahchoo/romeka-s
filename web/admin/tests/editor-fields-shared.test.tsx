/**
 * The option lists the editor shares with its block controls, and the debounce
 * on the item-query match count. A control that receives the lists makes no
 * request of its own, and a typed query asks for one count.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { parseItemSet, parseResourceClass } from '../src/api/parse';
import type { EditorOptions } from '../src/pages/sites/editor/options';
import { BlockFormHarness } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestsFor } from './stub';

const TOTAL = 7;

const rowsOf = (name: string): readonly unknown[] => {
  const rows = loadFixture(name);
  return Array.isArray(rows) ? rows : [];
};

const options = (): EditorOptions => ({
  pages: [],
  itemSets: rowsOf('item-sets-page.json').slice(0, 2).map((row) => parseItemSet(row)),
  classes: rowsOf('resource-classes-page.json').slice(0, 2).map((row) => parseResourceClass(row)),
});

const handler = (url: URL): Response => {
  if (url.pathname === '/api/items') {
    return listResponse([], TOTAL);
  }
  return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

const renderQuery = (lists: EditorOptions) =>
  render(
    <BlockFormHarness
      layout="browsePreview"
      data={{ resource_type: 'items', query: '' }}
      options={lists}
    />,
  );

describe("the editor's shared option lists", () => {
  it('reads the item sets and classes the editor supplies and requests neither', async () => {
    const fetchMock = installFetch(handler);
    renderQuery(options());

    await screen.findByText('7 items in this site match.');
    expect(requestsFor(fetchMock, '/api/item_sets')).toHaveLength(0);
    expect(requestsFor(fetchMock, '/api/resource_classes')).toHaveLength(0);
    const itemSet = screen.getByLabelText('Item set');
    expect(itemSet.querySelectorAll('option').length).toBeGreaterThan(1);
  });

  it('asks for one match count after the typed query settles', async () => {
    const fetchMock = installFetch(handler);
    renderQuery(options());
    await screen.findByText('7 items in this site match.');
    const before = requestsFor(fetchMock, '/api/items').length;

    const fulltext = screen.getByLabelText('Full-text search');
    fireEvent.change(fulltext, { target: { value: 'c' } });
    fireEvent.change(fulltext, { target: { value: 'ca' } });
    fireEvent.change(fulltext, { target: { value: 'cat' } });

    await waitFor(
      () => {
        const sent = requestsFor(fetchMock, '/api/items').slice(before);
        expect(sent).toHaveLength(1);
        expect(sent[0]?.url.searchParams.get('fulltext_search')).toBe('cat');
      },
      { timeout: 2000 },
    );
  });
});
