/**
 * The item page's History: each entry says when, who and what made it, then each field before
 * and after. "Put this value back" is an ordinary `PUT` of the item with that one field as it
 * was and every other field as it is now.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { Suspense } from 'react';
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import { ItemHistory } from '../src/pages/items/history';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';

const ITEM = brandId<'ItemId'>(1);

const dateChange = {
  field: 'values',
  property_id: 7,
  term: 'dcterms:date',
  label: 'Date',
  before: [{ type: 'literal', property_id: 7, is_public: true, '@value': '1950', display: '1950' }],
  after: [{ type: 'literal', property_id: 7, is_public: true, '@value': 'c. 1950', display: 'c. 1950' }],
};

const entry = (id: number, changes: readonly unknown[], source: unknown = { kind: 'edit', id: null }) => ({
  id,
  created: '2026-10-22T09:30:00+00:00',
  user: { id: 1, name: 'Admin' },
  source,
  changes,
});

const renderHistory = (onSaved: () => void = () => undefined) =>
  render(
    <Suspense fallback={null}>
      <ItemHistory item={ITEM} onSaved={onSaved} />
    </Suspense>,
  );

describe('item history', () => {
  it('states when, who and what made each change, and each field before and after', async () => {
    installFetch(
      route({
        '/api/romeka/items/1/history': () =>
          jsonResponse({
            entries: [
              entry(2, [dateChange, { field: 'visibility', before: true, after: false }], { kind: 'bulk', id: 12 }),
              entry(1, [{ field: 'item_sets', before: [], after: [{ id: 4, title: 'Letters' }] }]),
            ],
          }),
      }),
    );
    renderHistory();

    expect(await screen.findByText(/· Admin · Bulk change 12$/)).toBeInTheDocument();
    expect(screen.getByText('Date: 1950 → c. 1950')).toBeInTheDocument();
    expect(screen.getByText('Visibility: Public → Private')).toBeInTheDocument();
    expect(screen.getByText('Item sets: (none) → Letters')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Put this value back' })).toHaveLength(3);
  });

  it('puts one field back through the item write, keeping every other field as it is now', async () => {
    let entries = [entry(2, [dateChange])];
    const fetchMock = installFetch(
      route({
        '/api/romeka/items/1/history': () => jsonResponse({ entries }),
        '/api/items/1': (_url, init) => {
          if (init?.method === 'PUT') {
            entries = [entry(3, [{ ...dateChange, before: dateChange.after, after: dateChange.before }]), ...entries];
          }
          return jsonResponse(loadFixture('item-1.json'));
        },
      }),
    );
    renderHistory();

    fireEvent.click(await screen.findByRole('button', { name: 'Put this value back' }));

    expect(await screen.findByText('The value is back. The newest change at the top says so.')).toBeInTheDocument();
    const put = first(requestsFor(fetchMock, '/api/items/1').filter((request) => request.init?.method === 'PUT'));
    const body = requestBody(put);
    expect(body['dcterms:date']).toEqual([{ property_id: 7, type: 'literal', '@value': '1950', is_public: true }]);
    expect(body['dcterms:title']).toEqual([
      { property_id: 1, type: 'literal', '@value': 'Fixture Item One', '@language': 'en', is_public: true },
    ]);
    expect(body['o:item_set']).toEqual([{ 'o:id': 4 }]);
    expect(body['o:is_public']).toBe(false);
  });

  it('says so when the item already has the value, and offers no put-back for a field that had none', async () => {
    const added = { ...dateChange, before: [] };
    installFetch(
      route({
        '/api/romeka/items/1/history': () =>
          jsonResponse({ entries: [entry(2, [dateChange]), entry(1, [added])] }),
        '/api/items/1': () => jsonResponse(loadFixture('item-1.json')),
      }),
    );
    renderHistory();

    const buttons = await screen.findAllByRole('button', { name: 'Put this value back' });
    expect(buttons).toHaveLength(1);
    fireEvent.click(buttons[0] ?? document.body);
    expect(await screen.findByText('Nothing changed: the item has this value now.')).toBeInTheDocument();
  });

  it('says when nothing is recorded yet', async () => {
    installFetch(route({ '/api/romeka/items/1/history': () => jsonResponse({ entries: [] }) }));
    renderHistory();
    expect(await screen.findByText('No change to this item is recorded yet.')).toBeInTheDocument();
  });
});
