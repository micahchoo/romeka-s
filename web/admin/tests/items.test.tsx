/**
 * Items page tests: a stubbed fetch serves the fixtures, so the pages are
 * exercised through the real typed client and its parsers. No network.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Mock } from 'vitest';
import { brandId, isRecord, isUnknownArray } from '../src/api/parse';
import { setSession } from '../src/api/session';
import type { User } from '../src/api/types';
import { ItemsList } from '../src/pages/items/list';
import { ItemsPage } from '../src/pages/items/page';
import { loadFixture } from './loadFixture';

const requestUrl = (input: RequestInfo | URL): URL => {
  if (typeof input === 'string') {
    return new URL(input, window.location.href);
  }
  if (input instanceof URL) {
    return input;
  }
  return new URL(input.url, window.location.href);
};

const jsonResponse = (
  body: unknown,
  options: { readonly status?: number; readonly headers?: Record<string, string> } = {},
): Response =>
  new Response(JSON.stringify(body), {
    status: options.status ?? 200,
    headers: { 'Content-Type': 'application/json', ...options.headers },
  });

const installFetch = (handler: (url: URL) => Response): Mock<typeof fetch> => {
  const fetchMock = vi.fn<typeof fetch>((input) => Promise.resolve(handler(requestUrl(input))));
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
};

const urlsFor = (fetchMock: Mock<typeof fetch>, pathname: string): readonly URL[] =>
  fetchMock.mock.calls
    .map(([input]) => requestUrl(input))
    .filter((url) => url.pathname === pathname);

const first = <T,>(values: readonly T[]): T => {
  const value = values[0];
  if (value === undefined) {
    throw new Error('expected at least one value');
  }
  return value;
};

const linkHeader = (rels: ReadonlyArray<readonly [string, number]>): string =>
  rels
    .map(([rel, page]) => `<http://localhost/api/items?page=${String(page)}>; rel="${rel}"`)
    .join(', ');

const OPTION_FIXTURES: Readonly<Record<string, string>> = {
  '/api/users': 'users-page.json',
  '/api/resource_classes': 'resource-classes-page.json',
  '/api/item_sets': 'item-sets-page.json',
  '/api/resource_templates': 'resource-templates-page.json',
  '/api/sites': 'sites-page.json',
  '/api/properties': 'properties-page.json',
};

/** The chip counts: Photograph (2) and Document (3), with the given untyped and classed totals. */
const typeCounts = (noItemType: number, withClass: number): unknown => ({
  item_types: [
    { id: 2, items: 297 },
    { id: 3, items: 0 },
  ],
  no_item_type: noItemType,
  with_class: withClass,
});

/** Items answer from the fixtures; a query naming `search=nothing` matches no item. */
const listHandler = (url: URL): Response => {
  if (url.pathname === '/api/romeka/item-types/counts') {
    return jsonResponse(typeCounts(0, 2));
  }
  if (url.pathname === '/api/items') {
    if (url.searchParams.get('search') === 'nothing') {
      return jsonResponse([], { headers: { 'Omeka-S-Total-Results': '0' } });
    }
    if (url.searchParams.get('page') === '2') {
      return jsonResponse(loadFixture('items-page-2.json'), {
        headers: {
          'Omeka-S-Total-Results': '60',
          Link: linkHeader([
            ['prev', 1],
            ['next', 3],
            ['last', 3],
          ]),
        },
      });
    }
    return jsonResponse(loadFixture('items-page.json'), {
      headers: {
        'Omeka-S-Total-Results': '60',
        Link: linkHeader([
          ['next', 2],
          ['last', 3],
        ]),
      },
    });
  }
  const fixture = OPTION_FIXTURES[url.pathname];
  if (fixture !== undefined) {
    return jsonResponse(loadFixture(fixture));
  }
  return jsonResponse(
    { errors: { error: `Unexpected request to ${url.pathname}` } },
    { status: 404 },
  );
};

const lastItemsUrl = (fetchMock: Mock<typeof fetch>): URL => {
  const urls = urlsFor(fetchMock, '/api/items');
  const url = urls[urls.length - 1];
  if (url === undefined) {
    throw new Error('expected an items request');
  }
  return url;
};

/** The route query the screen wrote into the URL. */
const routeQuery = (): URLSearchParams =>
  new URLSearchParams(window.location.hash.split('?')[1] ?? '');

const openList = async (hash = '#/items'): Promise<Mock<typeof fetch>> => {
  window.history.replaceState(null, '', `/${hash}`);
  const fetchMock = installFetch(listHandler);
  render(<ItemsList />);
  await screen.findByRole('link', { name: 'Fixture Item One' });
  return fetchMock;
};

const setFixture = (index: number): Record<string, unknown> => {
  const page = loadFixture('item-sets-page.json');
  const entry = isUnknownArray(page) ? page[index] : undefined;
  if (!isRecord(entry)) {
    throw new Error('expected an item set fixture');
  }
  return { ...entry };
};

const detailHandler = (url: URL): Response => {
  if (url.pathname === '/api/items/1') {
    return jsonResponse(loadFixture('item-1.json'));
  }
  if (url.pathname === '/api/media') {
    return jsonResponse(loadFixture('media-page-live.json'), {
      headers: { 'Omeka-S-Total-Results': '3' },
    });
  }
  if (url.pathname === '/api/users') {
    return jsonResponse(loadFixture('users-page.json'));
  }
  if (url.pathname === '/api/resource_classes') {
    return jsonResponse(loadFixture('resource-classes-page.json'));
  }
  if (url.pathname === '/api/resource_templates/2') {
    return jsonResponse(loadFixture('resource-template-2.json'));
  }
  if (url.pathname === '/api/data_types') {
    return jsonResponse(loadFixture('data-types-page.json'));
  }
  // The item names set 4; the page reads it back by id.
  if (url.pathname === '/api/item_sets' && url.searchParams.get('id') === '4') {
    return jsonResponse([{ ...setFixture(0), 'o:id': 4, 'o:title': 'Box 12 letters' }]);
  }
  return jsonResponse(
    { errors: { error: `Unexpected request to ${url.pathname}` } },
    { status: 404 },
  );
};

/** Everything the detail view needs except the item itself. */
const itemNotFoundHandler = (url: URL): Response => {
  if (url.pathname === '/api/items/1') {
    return jsonResponse({ errors: { error: 'Item not found' } }, { status: 404 });
  }
  return detailHandler(url);
};

describe('items list', () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it('browse rows show title, class, item sets, owner, created and the Private cue', async () => {
    const fetchMock = await openList();

    // Each row's page keeps the list (`in`, empty for the plain list) and the row's place in it.
    expect(screen.getByRole('link', { name: 'Fixture Item One' })).toHaveAttribute('href', '#/items/12?in=&at=1');
    expect(screen.getAllByRole('row')).toHaveLength(3);
    expect(screen.getByRole('link', { name: 'Fixture Item Two' })).toHaveAttribute('href', '#/items/9?in=&at=2');
    expect(await screen.findByRole('button', { name: 'Still Image' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Book' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Ada Lovelace' })).toBeInTheDocument();
    expect(screen.getByText('[No owner]')).toBeInTheDocument();
    expect(screen.getByText('Oct 16, 2026, 9:30 AM')).toBeInTheDocument();
    // Only the private item carries the cue.
    expect(screen.getAllByRole('button', { name: 'Private' })).toHaveLength(1);

    const itemsUrl = lastItemsUrl(fetchMock);
    expect(itemsUrl.searchParams.get('page')).toBe('1');
    expect(itemsUrl.searchParams.get('per_page')).toBe('25');
    expect(itemsUrl.searchParams.get('sort_by')).toBe('id');
    expect(itemsUrl.searchParams.get('sort_order')).toBe('desc');
    for (const path of ['/api/users', '/api/resource_classes', '/api/item_sets', '/api/resource_templates', '/api/sites']) {
      expect(first(urlsFor(fetchMock, path)).searchParams.get('per_page')).toBe('200');
    }
  });

  it('a page change fetches the next page and not the option lists again', async () => {
    const fetchMock = await openList();
    expect(screen.getByText('Page 1 of 3 (60 items)')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Previous' })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(await screen.findByRole('link', { name: 'Fixture Item Three' })).toBeInTheDocument();
    expect(screen.getByText('Page 2 of 3 (60 items)')).toBeInTheDocument();
    expect(urlsFor(fetchMock, '/api/items').map((url) => url.searchParams.get('page'))).toEqual(['1', '2']);
    expect(urlsFor(fetchMock, '/api/users')).toHaveLength(1);
    expect(urlsFor(fetchMock, '/api/resource_classes')).toHaveLength(1);
    expect(routeQuery().get('page')).toBe('2');
  });

  it('search settles into one request, the URL and a chip; Escape clears it', async () => {
    const fetchMock = await openList();
    const box = screen.getByRole('searchbox', { name: 'Search' });

    fireEvent.change(box, { target: { value: 'harbour' } });

    expect(await screen.findByText('Search: harbour')).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('search')).toBe('harbour');
    expect(urlsFor(fetchMock, '/api/items').filter((url) => url.searchParams.has('search'))).toHaveLength(1);
    expect(routeQuery().get('q')).toBe('harbour');
    expect(await screen.findByText('60 items match')).toBeInTheDocument();

    fireEvent.keyDown(box, { key: 'Escape' });

    expect(box).toHaveValue('');
    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.has('search')).toBe(false);
    });
    expect(screen.queryByText('Search: harbour')).not.toBeInTheDocument();
  });

  it('a class filter sends its id, names itself in a chip, and the chip removes it', async () => {
    const fetchMock = await openList();
    fireEvent.click(screen.getByRole('button', { name: 'More filters' }));
    const classSelect = screen.getByRole('combobox', { name: 'Class' });
    await within(classSelect).findByRole('option', { name: 'Book' });

    fireEvent.change(classSelect, { target: { value: '51' } });

    expect(await screen.findByText('Class: Book')).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('resource_class_id')).toBe('51');
    expect(routeQuery().get('class')).toBe('51');

    fireEvent.click(screen.getByRole('button', { name: 'Remove filter: Class: Book' }));

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.has('resource_class_id')).toBe(false);
    });
    expect(screen.queryByText('Class: Book')).not.toBeInTheDocument();
    expect(classSelect).toHaveValue('');
  });

  it('clicking an owner filters by them and opens More filters', async () => {
    const fetchMock = await openList();
    const more = screen.getByRole('button', { name: 'More filters' });
    expect(more).toHaveAttribute('aria-expanded', 'false');

    fireEvent.click(await screen.findByRole('button', { name: 'Ada Lovelace' }));

    expect(await screen.findByText('Owner: Ada Lovelace')).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('owner_id')).toBe('3');
    expect(more).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('combobox', { name: 'Owner' })).toHaveValue('3');
  });

  it('a field filter sends its own property row, names itself in a chip, and the chip removes it', async () => {
    const fetchMock = await openList();
    fireEvent.click(screen.getByRole('button', { name: 'More filters' }));
    const fieldSelect = screen.getByRole('combobox', { name: 'Field' });
    await within(fieldSelect).findByRole('option', { name: 'Description' });
    expect(screen.queryByRole('combobox', { name: 'Field is' })).not.toBeInTheDocument();

    fireEvent.change(fieldSelect, { target: { value: '4' } });

    expect(await screen.findByText('Description: empty')).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('property[1][property]')).toBe('4');
    expect(lastItemsUrl(fetchMock).searchParams.get('property[1][type]')).toBe('nex');
    expect(routeQuery().get('field')).toBe('4:empty');

    fireEvent.change(screen.getByRole('combobox', { name: 'Field is' }), { target: { value: 'filled' } });

    expect(await screen.findByText('Description: has a value')).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('property[1][type]')).toBe('ex');

    fireEvent.click(screen.getByRole('button', { name: 'Remove filter: Description: has a value' }));

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.has('property[1][type]')).toBe(false);
    });
    expect(fieldSelect).toHaveValue('');
  });

  it("with an item type chosen, the field filter offers that type's fields by the names it gives them", async () => {
    const fetchMock = await openList('#/items?template=2&field=4:empty');
    const fieldSelect = screen.getByRole('combobox', { name: 'Field' });
    await within(fieldSelect).findByRole('option', { name: 'Caption' });
    expect(within(fieldSelect).getAllByRole('option').map((option) => option.textContent)).toEqual([
      'Any',
      '#13',
      'Caption',
      'Title',
    ]);
    expect(await screen.findByText('Caption: empty')).toBeInTheDocument();
    expect(first(urlsFor(fetchMock, '/api/items')).searchParams.get('resource_template_id')).toBe('2');
  });

  it('clicking the Private cue filters to private items', async () => {
    const fetchMock = await openList();

    fireEvent.click(screen.getByRole('button', { name: 'Private' }));

    expect(await screen.findByRole('button', { name: 'Remove filter: Private' })).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('is_public')).toBe('0');
  });

  it('the grid shows cards, is remembered, and keeps the selection', async () => {
    await openList();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select Fixture Item One' }));
    const grid = screen.getByRole('button', { name: 'Grid' });

    fireEvent.click(grid);

    expect(grid).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
    const cards = within(screen.getByRole('list', { name: 'items' })).getAllByRole('listitem');
    expect(cards).toHaveLength(2);
    expect(within(first(cards)).getByText('No media')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Select Fixture Item One' })).toBeChecked();
    expect(routeQuery().get('view')).toBe('grid');
    expect(window.localStorage.getItem('romeka.items.view')).toBe('grid');
  });

  it('a URL opens the slice it names', async () => {
    const fetchMock = await openList('#/items?class=13&media=none&view=grid&sort=title');

    const url = first(urlsFor(fetchMock, '/api/items'));
    expect(url.searchParams.get('resource_class_id')).toBe('13');
    expect(url.searchParams.get('has_media')).toBe('0');
    expect(url.searchParams.get('sort_by')).toBe('title');
    expect(await screen.findByText('Class: Still Image')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Grid' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('combobox', { name: 'Sort' })).toHaveValue('title');
  });

  it('a link to another slice while the list is open opens that slice', async () => {
    const fetchMock = await openList('#/items?q=harbour');

    window.history.replaceState(null, '', '/#/items?class=51');
    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(await screen.findByText('Class: Book')).toBeInTheDocument();
    expect(screen.queryByText('Search: harbour')).not.toBeInTheDocument();
    expect(screen.getByRole('searchbox', { name: 'Search' })).toHaveValue('');
    expect(lastItemsUrl(fetchMock).searchParams.get('resource_class_id')).toBe('51');
    expect(lastItemsUrl(fetchMock).searchParams.has('search')).toBe(false);
  });

  it('a filter with no match says so and offers Clear all', async () => {
    window.history.replaceState(null, '', '/#/items?q=nothing');
    const fetchMock = installFetch(listHandler);
    render(<ItemsList />);

    expect(await screen.findByText('No items match these filters.')).toBeInTheDocument();
    const clearButtons = screen.getAllByRole('button', { name: 'Clear all' });
    expect(clearButtons).toHaveLength(2);
    fireEvent.click(clearButtons[1] ?? document.body);

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.has('search')).toBe(false);
    });
    expect(screen.getByRole('searchbox', { name: 'Search' })).toHaveValue('');
  });
});

/** `listHandler`, with some paths answered otherwise. */
const overriding =
  (answers: Readonly<Record<string, () => Response>>) =>
  (url: URL): Response =>
    answers[url.pathname]?.() ?? listHandler(url);

const openWith = async (handler: (url: URL) => Response, hash = '#/items'): Promise<Mock<typeof fetch>> => {
  window.history.replaceState(null, '', `/${hash}`);
  const fetchMock = installFetch(handler);
  render(<ItemsList />);
  await screen.findByRole('link', { name: 'Fixture Item One' });
  return fetchMock;
};

/** The fixture page with its first item made a Photograph. */
const typedItems = (): Response => {
  const page = loadFixture('items-page.json');
  const items = isUnknownArray(page) ? page.filter(isRecord) : [];
  const [head, ...rest] = items;
  return jsonResponse(
    head === undefined
      ? []
      : [{ ...head, 'o:resource_template': { '@id': 'http://localhost/api/resource_templates/2', 'o:id': 2 } }, ...rest],
    { headers: { 'Omeka-S-Total-Results': '2' } },
  );
};

const chipGroup = async (): Promise<HTMLElement> => screen.findByRole('radiogroup', { name: 'Item type' });

describe('item type chips', () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it('lead the bar: each type by name with its total, unused ones at 0, and no Class select in the first row', async () => {
    await openList();
    const group = await chipGroup();
    await within(group).findByRole('radio', { name: 'Document 0' });
    expect(within(group).getAllByRole('radio').map((chip) => chip.textContent)).toEqual(['Document 0', 'Photograph 297']);
    expect(within(group).getByRole('radio', { name: 'Photograph 297' })).toHaveAttribute(
      'title',
      '297 items of this type in all',
    );
    // No item lacks a type here, so there is no "No item type" chip.
    expect(within(group).queryByRole('radio', { name: /No item type/ })).not.toBeInTheDocument();
    // Class waits behind More filters.
    expect(screen.queryByRole('combobox', { name: 'Class' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'More filters' }));
    expect(screen.getByRole('combobox', { name: 'Class' })).toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: 'Item type' })).not.toBeInTheDocument();
  });

  it('a press filters to the type, keeps it in the URL and a chip; a second press lets it go', async () => {
    const fetchMock = await openList();
    const photograph = await within(await chipGroup()).findByRole('radio', { name: 'Photograph 297' });
    expect(photograph).toHaveAttribute('aria-checked', 'false');

    fireEvent.click(photograph);

    expect(await screen.findByText('Item type: Photograph')).toBeInTheDocument();
    expect(lastItemsUrl(fetchMock).searchParams.get('resource_template_id')).toBe('2');
    expect(routeQuery().get('template')).toBe('2');
    expect(photograph).toHaveAttribute('aria-checked', 'true');
    // The count is the type's total, so choosing it does not change it.
    expect(photograph).toHaveAccessibleName('Photograph 297');

    fireEvent.click(photograph);

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.has('resource_template_id')).toBe(false);
    });
    expect(screen.queryByText('Item type: Photograph')).not.toBeInTheDocument();
    expect(photograph).toHaveAttribute('aria-checked', 'false');
  });

  it('arrow keys move focus between the chips without choosing', async () => {
    const fetchMock = await openList('#/items?template=2');
    const group = await chipGroup();
    const document_ = await within(group).findByRole('radio', { name: 'Document 0' });
    const photograph = within(group).getByRole('radio', { name: 'Photograph 297' });
    // The chosen chip is the one Tab reaches.
    expect(photograph).toHaveAttribute('tabindex', '0');
    expect(document_).toHaveAttribute('tabindex', '-1');
    const requests = urlsFor(fetchMock, '/api/items').length;

    photograph.focus();
    fireEvent.keyDown(photograph, { key: 'ArrowRight' });

    expect(document_).toHaveFocus();
    expect(urlsFor(fetchMock, '/api/items')).toHaveLength(requests);
  });

  it('"No item type" shows while some item has none, and filters to them', async () => {
    const fetchMock = await openWith(
      overriding({ '/api/romeka/item-types/counts': () => jsonResponse(typeCounts(12, 2)) }),
    );
    const none = await within(await chipGroup()).findByRole('radio', { name: 'No item type 12' });
    expect(none).toHaveAttribute('title', '12 items with no item type in all');

    fireEvent.click(none);

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.get('romeka_no_resource_template')).toBe('1');
    });
    expect(routeQuery().get('template')).toBe('none');
  });

  it('chips work without counts when the counts fail', async () => {
    const fetchMock = await openWith(
      overriding({ '/api/romeka/item-types/counts': () => jsonResponse({ errors: { error: 'no' } }, { status: 500 }) }),
    );
    const photograph = await within(await chipGroup()).findByRole('radio', { name: 'Photograph' });

    fireEvent.click(photograph);

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.get('resource_template_id')).toBe('2');
    });
    // Unknown is not "none": the class column stays.
    expect(screen.getByRole('columnheader', { name: 'Class' })).toBeInTheDocument();
  });

  it('with no item types, say so and point to Item types', async () => {
    await openWith(
      overriding({
        '/api/resource_templates': () => jsonResponse([]),
        '/api/romeka/item-types/counts': () =>
          jsonResponse({ item_types: [], no_item_type: 2, with_class: 2 }),
      }),
    );
    expect(await screen.findByText('No item types yet.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Make one in Item types.' })).toHaveAttribute('href', '#/resource-templates');
    expect(screen.queryByRole('radiogroup', { name: 'Item type' })).not.toBeInTheDocument();
  });
});

describe('items table columns', () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it('the Item type column names each row\'s type, and a press filters by it', async () => {
    const fetchMock = await openWith(overriding({ '/api/items': typedItems }));
    expect(screen.getByRole('columnheader', { name: 'Item type' })).toBeInTheDocument();
    expect(screen.queryByRole('columnheader', { name: 'Resource class' })).not.toBeInTheDocument();
    const row = first(screen.getAllByRole('row').filter((entry) => within(entry).queryByText('Fixture Item One') !== null));

    fireEvent.click(await within(row).findByRole('button', { name: 'Photograph' }));

    await waitFor(() => {
      expect(lastItemsUrl(fetchMock).searchParams.get('resource_template_id')).toBe('2');
    });
  });

  it('the Class column shows only while some item has a class', async () => {
    await openWith(overriding({ '/api/romeka/item-types/counts': () => jsonResponse(typeCounts(0, 0)) }));
    await within(await chipGroup()).findByRole('radio', { name: 'Photograph 297' });
    expect(screen.queryByRole('columnheader', { name: 'Class' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Still Image' })).not.toBeInTheDocument();
  });

  it('two classes with one name carry their vocabulary prefix, in the select and the chip', async () => {
    const classes = loadFixture('resource-classes-page.json');
    const twin = {
      ...(isUnknownArray(classes) && isRecord(classes[0]) ? classes[0] : {}),
      'o:id': 60,
      'o:term': 'schema:StillImage',
      'o:local_name': 'StillImage',
    };
    await openWith(
      overriding({
        '/api/resource_classes': () => jsonResponse([...(isUnknownArray(classes) ? classes : []), twin]),
      }),
      '#/items?class=60',
    );
    const classSelect = screen.getByRole('combobox', { name: 'Class' });
    await within(classSelect).findByRole('option', { name: 'Still Image (schema)' });
    expect(within(classSelect).getByRole('option', { name: 'Still Image (dctype)' })).toBeInTheDocument();
    expect(within(classSelect).getByRole('option', { name: 'Book' })).toBeInTheDocument();
    expect(await screen.findByText('Class: Still Image (schema)')).toBeInTheDocument();
  });
});

const user = (role: string, id: number): User => ({
  id: brandId<'UserId'>(id),
  apiUrl: `http://localhost/api/users/${String(id)}`,
  types: ['o:User'],
  name: `User ${String(id)}`,
  email: `user${String(id)}@example.com`,
  created: '2026-10-16T09:00:00+00:00',
  role,
  isActive: true,
  localeSetting: null,
  lessons: { hidden: false, seen: [] },
  savedLists: [],
  adminLocale: null,
});

afterEach(() => {
  setSession(null);
});

describe('item detail', () => {
  it('show renders fields, media rows and values', async () => {
    const fetchMock = installFetch(detailHandler);
    render(<ItemsPage param="1" />);

    expect(
      await screen.findByRole('heading', { name: 'Fixture Item One', level: 2 }),
    ).toBeInTheDocument();
    expect(screen.getByText('Still Image')).toBeInTheDocument();
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getByText('Private')).toBeInTheDocument();
    expect(await screen.findByText('Photograph')).toBeInTheDocument();
    expect(await screen.findByRole('link', { name: 'Box 12 letters' })).toHaveAttribute('href', '#/items?set=4');
    expect(screen.getByText('In sites').nextElementSibling).toHaveTextContent('[None]');

    expect(screen.getByRole('heading', { name: 'Media (3)', level: 2 })).toBeInTheDocument();

    // Three media rows: two carry a file, one has no picture and names its kind,
    // once in the empty slot and once as the tile's badge.
    const images = screen.getAllByRole('img');
    expect(images).toHaveLength(2);
    expect(screen.getAllByRole('figure')).toHaveLength(3);
    expect(screen.getAllByText('Embed')).toHaveLength(2);

    expect(screen.getByRole('link', { name: 'BHC006_GMT_AF_010a.jpg' })).toHaveAttribute(
      'href',
      'http://localhost:8124/files/original/e82cef00da75e9c5661a9ee66155ef419cf89988.jpg',
    );
    expect(first(images)).toHaveAttribute(
      'src',
      'http://localhost:8124/files/original/e82cef00da75e9c5661a9ee66155ef419cf89988.jpg',
    );
    expect(images[1]).toHaveAttribute(
      'src',
      'http://localhost:8124/files/thumbnails/square/abcdef0123.jpg',
    );
    expect(screen.getByText('Thumbnailed photo')).toBeInTheDocument();
    expect(screen.getByText('image/jpeg')).toBeInTheDocument();

    expect(screen.getByRole('link', { name: 'Fixtures' })).toHaveAttribute(
      'href',
      'http://example.org/subjects/fixtures',
    );
    expect(screen.getByText('Fixture Item Two (items)')).toBeInTheDocument();
    const langText = screen.getByText('Fixture Item One', { selector: '.value-text' });
    expect(langText.textContent).toBe('Fixture Item One (en)');

    expect(screen.getByText('Oct 16, 2026, 9:30 AM')).toBeInTheDocument();
    expect(screen.getByText('Oct 16, 2026, 10:15 AM')).toBeInTheDocument();

    const mediaUrl = first(urlsFor(fetchMock, '/api/media'));
    expect(mediaUrl.searchParams.get('item_id')).toBe('1');
    expect(mediaUrl.searchParams.get('per_page')).toBe('100');
  });

  it('a researcher sees the media but no way to add one', async () => {
    setSession(user('researcher', 3));
    installFetch(detailHandler);
    render(<ItemsPage param="1" />);

    expect(await screen.findByRole('heading', { name: 'Media (3)', level: 2 })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add media' })).not.toBeInTheDocument();
    expect(screen.getByText('You can view these media. Only people who may edit this item can add media.')).toBeInTheDocument();
  });

  it('an author sees the button on her own item and the note on another author’s', async () => {
    setSession(user('author', 3));
    installFetch(detailHandler);
    const own = render(<ItemsPage param="1" />);
    expect(await screen.findByRole('button', { name: 'Add media' })).toBeInTheDocument();
    own.unmount();

    setSession(user('author', 4));
    installFetch(detailHandler);
    render(<ItemsPage param="1" />);
    expect(await screen.findByText('You can view these media. Only people who may edit this item can add media.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add media' })).not.toBeInTheDocument();
  });

  it('missing item shows the upstream error message with status', async () => {
    installFetch(itemNotFoundHandler);
    render(<ItemsPage param="1" />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Item not found (HTTP 404)');
  });
});

describe('items route', () => {
  it('rejects a non-numeric item id without fetching', () => {
    const fetchMock = installFetch(() => {
      throw new Error('unexpected fetch');
    });
    render(<ItemsPage param="abc" />);

    expect(screen.getByRole('alert')).toHaveTextContent('"abc" is not a valid item id.');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
