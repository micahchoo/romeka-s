/**
 * The map block's inspector: the items come from the picker or a search, the
 * starting view is set by panning a small map or by typing, and the popup's
 * extra value is a property picked from the list.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { RomkaForm } from '../src/pages/sites/editor/fields/RomkaForm';
import { romkaFor, settingsOf } from '../src/pages/sites/editor/romeka';
import { apply, blockAt, load } from './editor-draft-helpers';
import { SITE_ID } from './editor-fields-helpers';
import { installLeaflet } from './leafletStub';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const handler = (url: URL): Response => {
  if (url.pathname.startsWith('/api/sites/')) {
    return jsonResponse(loadFixture('site-1.json'));
  }
  switch (url.pathname) {
    case '/api/properties':
      return jsonResponse(loadFixture('properties-page.json'));
    case '/api/items':
      return listResponse([], 0);
    case '/api/item_sets':
      return jsonResponse(loadFixture('item-sets-page.json'));
    case '/api/resource_classes':
      return jsonResponse(loadFixture('resource-classes-page.json'));
    default:
      return jsonResponse(
        { errors: { error: `Unexpected request to ${url.pathname}` } },
        { status: 404 },
      );
  }
};

const MapHarness = (): ReactElement => {
  const [state, setState] = useState<PageDraft>(() =>
    apply(load([]), { type: 'addBlock', layout: 'map', index: 0 }),
  );
  const block = blockAt(state.blocks, 0);
  const romka = romkaFor(BLOCK_REGISTRY, block);
  if (romka === undefined) {
    throw new Error('the map carrier was not recognised');
  }
  return (
    <>
      <RomkaForm
        block={block}
        romka={romka}
        issues={[]}
        siteId={SITE_ID}
        dispatch={(action) => {
          setState((current) => draftReducer(current, action));
        }}
      />
      <output data-testid="settings">{JSON.stringify(settingsOf(block))}</output>
    </>
  );
};

const stored = (): Readonly<Record<string, unknown>> => {
  const parsed: unknown = JSON.parse(screen.getByTestId('settings').textContent);
  return typeof parsed === 'object' && parsed !== null
    ? Object.fromEntries(Object.entries(parsed))
    : {};
};

describe('the map block form', () => {
  it('picks items by default, and offers the search for a search source', async () => {
    installFetch(handler);
    render(<MapHarness />);

    expect(await screen.findByRole('group', { name: 'Items' })).toBeInTheDocument();
    expect(screen.queryByText('Search')).toBeNull();
    fireEvent.change(screen.getByLabelText('Places from'), { target: { value: 'query' } });
    expect(screen.queryByRole('group', { name: 'Items' })).toBeNull();
    expect(stored()).toMatchObject({ source: 'query' });
  });

  it('fits every place until a set view is chosen, then stores the typed centre and zoom', () => {
    installFetch(handler);
    render(<MapHarness />);

    expect(screen.getByRole('radio', { name: 'Fit every place' })).toBeChecked();
    fireEvent.click(screen.getByRole('radio', { name: 'Start at a set view' }));
    expect(stored()).toMatchObject({ center: '20, 0', zoom: 2 });

    fireEvent.change(screen.getByLabelText('Centre (lat, lng)'), {
      target: { value: '48.85, 2.35' },
    });
    fireEvent.change(screen.getByLabelText('Zoom'), { target: { value: '25' } });
    expect(stored()).toMatchObject({ center: '48.85, 2.35', zoom: 19 });

    fireEvent.change(screen.getByLabelText('Centre (lat, lng)'), { target: { value: 'Paris' } });
    expect(screen.getByRole('alert')).toHaveTextContent('Type the centre as a latitude');

    fireEvent.click(screen.getByRole('radio', { name: 'Fit every place' }));
    expect(stored()).toMatchObject({ center: '', zoom: null });
  });

  it('stores the view the curator pans and zooms the small map to', async () => {
    const leaflet = installLeaflet();
    installFetch(handler);
    render(<MapHarness />);

    fireEvent.click(screen.getByRole('radio', { name: 'Start at a set view' }));
    await waitFor(() => {
      expect(leaflet.map().zoom).toBe(2);
    });
    leaflet.pan(51.507201, -0.127601, 11);
    await waitFor(() => {
      expect(stored()).toMatchObject({ center: '51.507201, -0.127601', zoom: 11 });
    });
  });

  it('stores the popup value as a property term', async () => {
    installFetch(handler);
    render(<MapHarness />);

    const select = screen.getByLabelText('Popup value');
    await screen.findByRole('option', { name: 'Description (dcterms:description)' });
    fireEvent.change(select, { target: { value: 'dcterms:description' } });
    expect(stored()).toMatchObject({ popup_property: 'dcterms:description' });
  });
});
