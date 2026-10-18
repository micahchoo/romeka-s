/**
 * Items page tests: a stubbed fetch serves the fixtures, so the pages are
 * exercised through the real typed client and its parsers. No network.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { Mock } from 'vitest';
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

const listHandler = (url: URL): Response => {
  if (url.pathname === '/api/items') {
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
  if (url.pathname === '/api/users') {
    return jsonResponse(loadFixture('users-page.json'));
  }
  if (url.pathname === '/api/resource_classes') {
    return jsonResponse(loadFixture('resource-classes-page.json'));
  }
  return jsonResponse(
    { errors: { error: `Unexpected request to ${url.pathname}` } },
    { status: 404 },
  );
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
  it('browse rows show title, class, owner and created labels', async () => {
    const fetchMock = installFetch(listHandler);
    render(<ItemsList />);

    expect(await screen.findByRole('link', { name: 'Fixture Item One' })).toHaveAttribute(
      'href',
      '#/items/12',
    );
    expect(screen.getAllByRole('row')).toHaveLength(3);
    expect(screen.getByRole('link', { name: 'Fixture Item Two' })).toHaveAttribute(
      'href',
      '#/items/9',
    );
    expect(screen.getByText('Still Image')).toBeInTheDocument();
    expect(screen.getByText('Book')).toBeInTheDocument();
    expect(screen.getByText('Ada Lovelace')).toBeInTheDocument();
    expect(screen.getByText('[No owner]')).toBeInTheDocument();
    expect(screen.getByText('2026-10-16 09:30')).toBeInTheDocument();
    expect(screen.getByText('2026-10-16 11:00')).toBeInTheDocument();

    const itemsUrl = first(urlsFor(fetchMock, '/api/items'));
    expect(itemsUrl.searchParams.get('page')).toBe('1');
    expect(itemsUrl.searchParams.get('per_page')).toBe('25');
    expect(itemsUrl.searchParams.get('sort_by')).toBe('id');
    expect(itemsUrl.searchParams.get('sort_order')).toBe('desc');
    expect(first(urlsFor(fetchMock, '/api/users')).searchParams.get('per_page')).toBe('100');
    expect(first(urlsFor(fetchMock, '/api/resource_classes')).searchParams.get('per_page')).toBe(
      '100',
    );
  });

  it('link header pagination fetches the next page', async () => {
    const fetchMock = installFetch(listHandler);
    render(<ItemsList />);

    expect(await screen.findByText('Page 1 of 3 (60 items)')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Previous' })).toBeDisabled();

    fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(await screen.findByRole('link', { name: 'Fixture Item Three' })).toBeInTheDocument();
    expect(screen.getByText('Page 2 of 3 (60 items)')).toBeInTheDocument();
    const pages = urlsFor(fetchMock, '/api/items').map((url) => url.searchParams.get('page'));
    expect(pages).toEqual(['1', '2']);
  });
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
    expect(screen.getByText('#2')).toBeInTheDocument();

    expect(screen.getByRole('heading', { name: 'Media (3)', level: 2 })).toBeInTheDocument();

    // Three media rows: two carry a file, one has nothing to show.
    const images = screen.getAllByRole('img');
    expect(images).toHaveLength(2);
    expect(screen.getAllByRole('figure')).toHaveLength(3);
    expect(screen.getAllByText('No preview')).toHaveLength(1);

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
    const langCell = screen.getByText('Fixture Item One', { selector: 'td' });
    expect(langCell.textContent).toBe('Fixture Item One (en)');

    expect(screen.getByText('2026-10-16 09:30')).toBeInTheDocument();
    expect(screen.getByText('2026-10-16 10:15')).toBeInTheDocument();

    const mediaUrl = first(urlsFor(fetchMock, '/api/media'));
    expect(mediaUrl.searchParams.get('item_id')).toBe('1');
    expect(mediaUrl.searchParams.get('per_page')).toBe('100');
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
