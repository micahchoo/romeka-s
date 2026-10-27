/**
 * The set's place: `#/item-sets/<id>` opens the Items screen filtered to the
 * set, under the set's card. A stubbed fetch serves the fixtures, so each
 * screen runs through the real client and parsers.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { ItemSetsPage } from '../src/pages/item-sets/page';
import { ItemSetForm } from '../src/pages/item-sets/form';
import { ItemsList } from '../src/pages/items/list';
import { MediaPage } from '../src/pages/media/page';
import { loadFixture } from './loadFixture';
import { detached, installFetch, jsonResponse, listResponse, requestsFor, requestUrl, route } from './stub';
import type { Handler } from './stub';

const recordOf = (value: unknown): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new Error('expected a record fixture');
  }
  return { ...value };
};

const entryOf = (fixture: string, index: number): Record<string, unknown> => {
  const page = loadFixture(fixture);
  return recordOf(isUnknownArray(page) ? page[index] : undefined);
};

const DESCRIPTION = 'Letters from the 1950s. '.repeat(30);

/** Set 1, "Featured photographs": public, open, with a description. */
const setOne = (): Record<string, unknown> => ({
  ...entryOf('item-sets-page.json', 0),
  'dcterms:description': [
    { type: 'literal', property_id: 4, property_label: 'Description', is_public: true, '@value': DESCRIPTION },
  ],
});

/** "My Site" shows set 1; "Second Site" does not. */
const sites = (): unknown[] => [
  {
    ...entryOf('sites-page.json', 0),
    'o:site_item_set': [{ 'o:item_set': { '@id': 'http://localhost/api/item_sets/1', 'o:id': 1 } }],
  },
  entryOf('sites-page.json', 1),
];

interface PlaceServer {
  /** How the set read answers. */
  readonly set: 'ok' | 'missing' | 'hidden';
  /** The set's item count, `item_set_id=1&per_page=1` total. */
  readonly count: number;
  /** The list total when another filter narrows the set. */
  readonly narrowed?: number;
}

const placeHandler = ({ set, count, narrowed = 0 }: PlaceServer): Handler =>
  route({
    '/api/item_sets/1': (_url, init) => {
      if (set === 'missing') {
        return jsonResponse({ errors: { error: 'Not found' } }, { status: 404 });
      }
      if (set === 'hidden') {
        return jsonResponse({ errors: { error: 'Forbidden' } }, { status: 403 });
      }
      return jsonResponse(init?.method === 'DELETE' ? detached(setOne()) : setOne());
    },
    '/api/items': (url) => {
      if (url.searchParams.get('per_page') === '1') {
        return listResponse([], count);
      }
      if (url.searchParams.has('search')) {
        return listResponse(loadFixture('items-page.json'), narrowed);
      }
      return count === 0 ? listResponse([], 0) : listResponse(loadFixture('items-page.json'), count);
    },
    '/api/sites': () => listResponse(sites(), 2),
    '/api/users': () => jsonResponse(loadFixture('users-page.json')),
    '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
    '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
    '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
    '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
    '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  });

const openPlace = (server: PlaceServer, query = 'set=1') => {
  window.history.replaceState(null, '', `/#/items?${query}`);
  const fetchMock = installFetch(placeHandler(server));
  render(<ItemsList />);
  return fetchMock;
};

const card = async (): Promise<HTMLElement> =>
  (await screen.findByRole('heading', { name: 'Featured photographs', level: 2 })).closest('section') ??
  document.body;

beforeEach(() => {
  window.localStorage.clear();
});

describe('the set card', () => {
  it('shows a loading line in the card until the set answers', () => {
    window.history.replaceState(null, '', '/#/items?set=1');
    const handler = placeHandler({ set: 'ok', count: 3 });
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>((input, init) => {
        const url = requestUrl(input);
        return url.pathname === '/api/item_sets/1'
          ? new Promise<Response>(() => undefined)
          : Promise.resolve(handler(url, init));
      }),
    );
    render(<ItemsList />);

    expect(screen.getByText('Loading item set...')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Item sets' })).toHaveAttribute('href', '#/item-sets');
    expect(screen.queryByText('0 items')).not.toBeInTheDocument();
  });

  it('heads the results with the title, the count, the facts, the sites and the actions', async () => {
    const fetchMock = openPlace({ set: 'ok', count: 3 });
    const place = await card();

    expect(within(place).getByText('3 items')).toBeInTheDocument();
    expect(within(place).getByText('Public')).toBeInTheDocument();
    expect(within(place).getByText('Open to additions')).toBeInTheDocument();
    expect(within(place).getByRole('link', { name: 'My Site' })).toHaveAttribute('href', '#/sites/1');
    expect(within(place).queryByRole('link', { name: 'Second Site' })).not.toBeInTheDocument();
    expect(within(place).getByText(DESCRIPTION.trim())).toBeInTheDocument();
    expect(within(place).getByRole('link', { name: 'Edit set' })).toHaveAttribute('href', '#/item-sets/1/edit');
    expect(within(place).getByRole('link', { name: 'Item sets' })).toHaveAttribute('href', '#/item-sets');
    expect(await screen.findByRole('link', { name: 'Fixture Item One' })).toBeInTheDocument();
    // The card states the count, so the list does not say it again.
    expect(screen.queryByText('3 items match')).not.toBeInTheDocument();

    const countRead = requestsFor(fetchMock, '/api/items').find(
      (request) => request.url.searchParams.get('per_page') === '1',
    );
    expect(countRead?.url.searchParams.get('item_set_id')).toBe('1');
  });

  it('states how many of the set match when another filter narrows it', async () => {
    openPlace({ set: 'ok', count: 3410, narrowed: 2 }, 'set=1&q=letters');
    const place = await card();

    expect(within(place).getByText('3,410 items')).toBeInTheDocument();
    expect(await within(place).findByText('2 match the other filters')).toBeInTheDocument();
  });

  it.each([
    ['missing', 'missing' as const],
    ['hidden', 'hidden' as const],
  ])('says a %s set is not available and shows no rows', async (_name, set) => {
    openPlace({ set, count: 3 });

    expect(await screen.findByText('This set is not available.')).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'Fixture Item One' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Change items…' })).not.toBeInTheDocument();
  });

  it('teaches how items get into a set that has none', async () => {
    openPlace({ set: 'ok', count: 0 });
    const place = await card();

    expect(within(place).getByText('0 items')).toBeInTheDocument();
    expect(await screen.findByText('No items in this set yet.')).toBeInTheDocument();
    expect(
      screen.getByText('To add some, tick items on the Items screen, choose “Change items…”, then “Add to item set”.', {
        exact: false,
      }),
    ).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Go to Items' })).toHaveAttribute('href', '#/items');
  });

  it('leaves the place when the set chip is removed', async () => {
    openPlace({ set: 'ok', count: 3 });
    await card();

    fireEvent.click(await screen.findByRole('button', { name: 'Remove filter: Item set: Featured photographs' }));

    await waitFor(() => {
      expect(screen.queryByRole('heading', { name: 'Featured photographs' })).not.toBeInTheDocument();
    });
    expect(window.location.hash).toBe('#/items');
  });

  it('confirms a delete with what it changes, then deletes the set', async () => {
    const fetchMock = openPlace({ set: 'ok', count: 3 });
    const place = await card();

    fireEvent.click(within(place).getByRole('button', { name: 'Delete this item set' }));

    expect(within(place).getByText('Delete Featured photographs? This cannot be undone.')).toBeInTheDocument();
    expect(within(place).getByText('The 3 items stay. They leave this set.')).toBeInTheDocument();
    expect(within(place).getByText('1 site shows this set. It stops showing it.')).toBeInTheDocument();

    fireEvent.click(within(place).getByRole('button', { name: 'Delete Featured photographs' }));
    await waitFor(() => {
      expect(window.location.hash).toBe('#/item-sets');
    });
    expect(requestsFor(fetchMock, '/api/item_sets/1').some((request) => request.init?.method === 'DELETE')).toBe(true);
  });
});

describe('the set routes', () => {
  afterEach(() => {
    window.history.replaceState(null, '', '/');
  });

  it("opens the set's place from its address and replaces the history entry", async () => {
    window.history.replaceState(null, '', '/#/item-sets/1');
    const before = window.history.length;
    render(<ItemSetsPage param="1" rest={['1']} />);

    await waitFor(() => {
      expect(window.location.hash).toBe('#/items?set=1');
    });
    expect(window.history.length).toBe(before);
  });

  it('edits the set at its edit address, and its delete states the counts', async () => {
    installFetch(placeHandler({ set: 'ok', count: 1 }));
    render(<ItemSetsPage param="1" rest={['1', 'edit']} />);

    expect(await screen.findByLabelText('Title')).toHaveValue('Featured photographs');
    fireEvent.click(screen.getByRole('button', { name: 'Delete this item set' }));
    expect(await screen.findByText('The 1 item stays. It leaves this set.')).toBeInTheDocument();
  });

  it('rejects an address with an unknown action', () => {
    render(<ItemSetsPage param="1" rest={['1', 'items']} />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('lands a new set on its place', async () => {
    installFetch((url, init) =>
      url.pathname === '/api/item_sets' && init?.method === 'POST'
        ? jsonResponse({ ...setOne(), 'o:id': 7 })
        : placeHandler({ set: 'ok', count: 0 })(url, init),
    );
    render(<ItemSetForm itemSetId={null} />);

    fireEvent.change(await screen.findByLabelText('Title'), { target: { value: 'New set' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add item set' }));

    await waitFor(() => {
      expect(window.location.hash).toBe('#/items?set=7');
    });
  });

  it('the set list links each row to its place', async () => {
    installFetch(placeHandler({ set: 'ok', count: 0 }));
    render(<ItemSetsPage />);

    expect(await screen.findByRole('link', { name: 'Featured photographs' })).toHaveAttribute('href', '#/item-sets/1');
  });
});

describe('the media page', () => {
  it("names the media's item by its title", async () => {
    installFetch(
      route({
        '/api/media/951': () => jsonResponse(loadFixture('media-951.json')),
        '/api/items/31': () => jsonResponse({ ...recordOf(loadFixture('item-1.json')), 'o:id': 31, 'o:title': 'Letter to Ada' }),
      }),
    );
    render(<MediaPage param="951" />);

    expect(await screen.findByRole('link', { name: 'Letter to Ada' })).toHaveAttribute('href', '#/items/31');
  });
});
