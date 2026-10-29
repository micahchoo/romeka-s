/**
 * The item form's place control: a site that names a location field gets a
 * small map and a "lat, lng" input for that property, and the place reaches
 * the write as an ordinary literal value.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { ItemsPage } from '../src/pages/items/page';
import { installLeaflet } from './leafletStub';
import { classRoutes } from './class-routes';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

/** The sites fixture with the first site naming `dcterms:description` its location field. */
const locatedSites = (): unknown => {
  const sites = loadFixture('sites-page.json');
  if (!isUnknownArray(sites)) {
    throw new Error('expected a sites page');
  }
  return sites.map((site, index) =>
    isRecord(site) && index === 0
      ? { ...site, 'o:site_settings': { romeka_location_property: 'dcterms:description' } }
      : site,
  );
};

const routes = (sites: () => unknown): Readonly<Record<string, () => Response>> => ({
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  ...classRoutes,
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  '/api/sites': () => jsonResponse(sites()),
});

const install = (sites: () => unknown) =>
  installFetch((url, init) => {
    if (url.pathname === '/api/items' && init?.method === 'POST') {
      return jsonResponse(loadFixture('item-1.json'));
    }
    const handler = routes(sites)[url.pathname];
    return handler === undefined
      ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
      : handler();
  });

const placeField = async (): Promise<HTMLElement> =>
  screen.findByRole('group', { name: 'Place on a map: Description' });

describe('the item form place control', () => {
  it('shows no map when no site names a location field', async () => {
    install(() => loadFixture('sites-page.json'));
    render(<ItemsPage param="new" />);

    await screen.findByLabelText('Title');
    expect(screen.queryByRole('group', { name: /Place on a map/ })).toBeNull();
  });

  it('stores a typed place as "lat, lng" with at most six decimals', async () => {
    const fetchMock = install(locatedSites);
    render(<ItemsPage param="new" />);

    const field = await placeField();
    fireEvent.click(within(field).getByRole('button', { name: 'Add a place' }));
    const input = within(field).getByRole('textbox', { name: 'Place 1' });
    fireEvent.change(input, { target: { value: ' 51.50720009 , -0.1276 ' } });
    fireEvent.blur(input);
    expect(input).toHaveValue('51.5072, -0.1276');

    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));
    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/items')).toHaveLength(1);
    });
    const post = requestsFor(fetchMock, '/api/items')[0];
    if (post === undefined) {
      throw new Error('expected a POST');
    }
    expect(requestBody(post)['dcterms:description']).toEqual([
      { property_id: 4, type: 'literal', '@value': '51.5072, -0.1276', is_public: true },
    ]);
  });

  it('says when a typed place is out of range', async () => {
    install(locatedSites);
    render(<ItemsPage param="new" />);

    const field = await placeField();
    fireEvent.click(within(field).getByRole('button', { name: 'Add a place' }));
    fireEvent.change(within(field).getByRole('textbox', { name: 'Place 1' }), {
      target: { value: '91, 0' },
    });
    expect(within(field).getByRole('alert')).toHaveTextContent(
      'Type a latitude from -90 to 90, a comma, and a longitude from -180 to 180.',
    );
  });

  it('sets the place where the reader clicks the map, and draws it', async () => {
    const leaflet = installLeaflet();
    install(locatedSites);
    render(<ItemsPage param="new" />);

    const field = await placeField();
    await waitFor(() => {
      expect(leaflet.map()).toBeDefined();
    });
    leaflet.click(48.85837, 2.294481);
    expect(await within(field).findByRole('textbox', { name: 'Place 1' })).toHaveValue(
      '48.85837, 2.294481',
    );
    await waitFor(() => {
      expect(leaflet.map().dots).toEqual([{ lat: 48.85837, lng: 2.294481 }]);
    });

    leaflet.click(51.5, -0.12);
    await waitFor(() => {
      expect(within(field).getByRole('textbox', { name: 'Place 1' })).toHaveValue('51.5, -0.12');
    });
    expect(within(field).queryByRole('textbox', { name: 'Place 2' })).toBeNull();
  });
});
