/**
 * Walking a result on screen: a row on Items opens its item page with the list
 * and its place in the address, and the item page says "2 of 3 in <place>"
 * with the items before and after it. A stubbed fetch serves the fixtures, so
 * each screen runs through the real client and parsers.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { ItemsPage } from '../src/pages/items/page';
import { ItemsList } from '../src/pages/items/list';
import { classRoutes } from './class-routes';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestsFor, route, urlsFor } from './stub';
import type { Handler } from './stub';

const recordOf = (value: unknown): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new Error('expected a record fixture');
  }
  return { ...value };
};

const item = (id: number): Record<string, unknown> => ({
  ...recordOf(loadFixture('item-1.json')),
  'o:id': id,
  'o:title': `Item ${String(id)}`,
});

const set = (): Record<string, unknown> => {
  const page = loadFixture('item-sets-page.json');
  return { ...recordOf(isUnknownArray(page) ? page[0] : undefined), 'o:id': 4, 'o:title': 'Box 12 letters' };
};

const OPTIONS: Readonly<Record<string, Handler>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  ...classRoutes,
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/resource_templates/2': () => jsonResponse(loadFixture('resource-template-2.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  '/api/sites': () => jsonResponse(loadFixture('sites-page.json')),
  '/api/media': () => listResponse([], 0),
  '/api/romeka/links/1': () => jsonResponse({ links: [] }),
  '/api/resources/4': () => jsonResponse(set()),
};

/** The set's list, in order, as ids; item 1 is the one on screen. */
const itemPage = (list: readonly number[]): Handler =>
  route({
    ...OPTIONS,
    '/api/items/1': (_url, init) => jsonResponse(init?.method === 'PUT' ? item(1) : { ...item(1), 'o:title': 'Item 1' }),
    '/api/items': (url) => {
      const page = Number(url.searchParams.get('page'));
      const perPage = Number(url.searchParams.get('per_page'));
      const rows = list.slice((page - 1) * perPage, page * perPage).map(item);
      return listResponse(rows, list.length);
    },
  });

const openItem = (query: string, list: readonly number[]) => {
  window.history.replaceState(null, '', `/#/items/1${query === '' ? '' : `?${query}`}`);
  const fetchMock = installFetch(itemPage(list));
  render(<ItemsPage param="1" />);
  return fetchMock;
};

const IN_SET = `in=${encodeURIComponent('set=4')}`;

beforeEach(() => {
  window.localStorage.clear();
});

describe('a row on Items', () => {
  it('opens its item page with the list and its place in the whole list', async () => {
    window.history.replaceState(null, '', '/#/items?set=4&page=2');
    installFetch(
      route({
        ...OPTIONS,
        '/api/item_sets/4': () => jsonResponse(set()),
        '/api/romeka/item-sets/counts': () => jsonResponse({ item_sets: [] }),
        '/api/items': () => listResponse([item(30), item(31)], 26),
      }),
    );
    render(<ItemsList />);

    // Page 2 at the default page size of 25 starts at the 26th item.
    expect(await screen.findByRole('link', { name: 'Item 30' })).toHaveAttribute('href', `#/items/30?${IN_SET}&at=26`);
    expect(screen.getByRole('link', { name: 'Item 31' })).toHaveAttribute('href', `#/items/31?${IN_SET}&at=27`);
  });

  it('a card in the grid opens the same address', async () => {
    window.history.replaceState(null, '', '/#/items?view=grid');
    installFetch(route({ ...OPTIONS, '/api/items': () => listResponse([item(30), item(31)], 2) }));
    render(<ItemsList />);

    expect(await screen.findByRole('link', { name: 'Item 31' })).toHaveAttribute(
      'href',
      `#/items/31?in=${encodeURIComponent('view=grid')}&at=2`,
    );
  });
});

describe('the item page in a walk', () => {
  it('says where the item is in the set and links the items each side', async () => {
    const fetchMock = openItem(`${IN_SET}&at=2`, [50, 1, 60]);

    const walk = await screen.findByRole('navigation', { name: 'Place in the list' });
    expect(within(walk).getByText('2 of 3 in Box 12 letters')).toBeInTheDocument();
    expect(within(walk).getByRole('link', { name: 'Previous: Item 50' })).toHaveAttribute(
      'href',
      `#/items/50?${IN_SET}&at=1`,
    );
    expect(within(walk).getByRole('link', { name: 'Next: Item 60' })).toHaveAttribute('href', `#/items/60?${IN_SET}&at=3`);

    const reads = urlsFor(fetchMock, '/api/items');
    expect(reads.map((url) => [url.searchParams.get('item_set_id'), url.searchParams.get('per_page'), url.searchParams.get('page')])).toEqual([
      ['4', '5', '1'],
    ]);
  });

  it('the whole Items list is the place when no set or record narrows it', async () => {
    openItem(`in=&at=1`, [1, 60]);

    const walk = await screen.findByRole('navigation', { name: 'Place in the list' });
    expect(within(walk).getByText('1 of 2 in Items')).toBeInTheDocument();
    expect(within(walk).queryByRole('link', { name: /^Previous/ })).not.toBeInTheDocument();
  });

  it('finds the item one place earlier when an item before it left the list', async () => {
    openItem(`${IN_SET}&at=3`, [50, 1, 60]);

    const walk = await screen.findByRole('navigation', { name: 'Place in the list' });
    expect(within(walk).getByText('2 of 3 in Box 12 letters')).toBeInTheDocument();
  });

  it('shows no number when the item is no longer near its place', async () => {
    const fetchMock = openItem(`${IN_SET}&at=2`, [50, 60, 70, 1]);

    await screen.findByRole('heading', { name: 'Item 1', level: 2 });
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/items')).toHaveLength(1);
    });
    expect(screen.queryByRole('navigation', { name: 'Place in the list' })).not.toBeInTheDocument();
    expect(screen.queryByText(/ of 4 /)).not.toBeInTheDocument();
  });

  it('an item opened with no list shows no line and reads no list', async () => {
    const fetchMock = openItem('', [50, 1, 60]);

    await screen.findByRole('heading', { name: 'Item 1', level: 2 });
    expect(screen.queryByRole('navigation', { name: 'Place in the list' })).not.toBeInTheDocument();
    expect(urlsFor(fetchMock, '/api/items')).toHaveLength(0);
  });
});

describe('Save and next', () => {
  it('saves the item, then opens the next item at its edit form', async () => {
    const fetchMock = openItem(`${IN_SET}&at=2`, [50, 1, 60]);
    await screen.findByRole('navigation', { name: 'Place in the list' });

    fireEvent.click(screen.getByText('Edit this item'));
    fireEvent.click(await screen.findByRole('button', { name: 'Save and next' }));

    await waitFor(() => {
      expect(window.location.hash).toBe(`#/items/60?${IN_SET}&at=3&edit=1`);
    });
    expect(requestsFor(fetchMock, '/api/items/1').filter((request) => request.init?.method === 'PUT')).toHaveLength(1);
  });

  it('Save stays on the item', async () => {
    const fetchMock = openItem(`${IN_SET}&at=2`, [50, 1, 60]);
    await screen.findByRole('navigation', { name: 'Place in the list' });

    fireEvent.click(screen.getByText('Edit this item'));
    fireEvent.click(await screen.findByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/items/1').filter((request) => request.init?.method === 'PUT')).toHaveLength(1);
    });
    expect(window.location.hash).toBe(`#/items/1?${IN_SET}&at=2`);
  });

  it('the last item has no Save and next', async () => {
    openItem(`${IN_SET}&at=3`, [50, 60, 1]);
    const walk = await screen.findByRole('navigation', { name: 'Place in the list' });
    expect(within(walk).queryByRole('link', { name: /^Next/ })).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Edit this item'));
    expect(await screen.findByRole('button', { name: 'Save' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Save and next' })).not.toBeInTheDocument();
  });

  it('an address that asks for the edit form opens it', async () => {
    openItem(`${IN_SET}&at=3&edit=1`, [50, 60, 1]);

    expect(await screen.findByRole('button', { name: 'Save' })).toBeInTheDocument();
    expect(screen.getByText('Edit this item')).toHaveAttribute('aria-expanded', 'true');
  });
});
