/**
 * "Linked from": one row per kind and field with its count, an items row that opens the links
 * place, item set rows listed inline a page at a time, and the loading, empty and error lines.
 * In the set card it is collapsed until opened.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { Suspense } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { brandId, isRecord, isUnknownArray } from '../src/api/parse';
import { LinkedFrom } from '../src/pages/items/linked-from';
import { ItemsList } from '../src/pages/items/list';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestUrl, route, urlsFor } from './stub';

const PERSON = brandId<'ItemId'>(1);

const setNamed = (id: number, title: string): Record<string, unknown> => {
  const page = loadFixture('item-sets-page.json');
  const first: unknown = isUnknownArray(page) ? page[0] : null;
  return { ...(isRecord(first) ? first : {}), 'o:id': id, 'o:title': title };
};

const renderLinks = () =>
  render(
    <Suspense fallback={null}>
      <LinkedFrom target={PERSON} />
    </Suspense>,
  );

describe('linked from', () => {
  it('states each kind and field with its count; an items row opens the links place', async () => {
    installFetch(route({ '/api/romeka/links/1': () => jsonResponse(loadFixture('places/links.json')) }));
    renderLinks();

    expect(await screen.findByRole('link', { name: 'Creator · 2 items' })).toHaveAttribute('href', '#/items?links=1&via=2');
    expect(screen.getByRole('button', { name: 'Subject · 1 item set' })).toHaveAttribute('aria-expanded', 'false');
  });

  it('lists an item set row inline, five at a time, through the res search its count totals', async () => {
    const fetchMock = installFetch(
      route({
        '/api/romeka/links/1': () =>
          jsonResponse({ links: [{ kind: 'item_sets', property_id: 3, term: 'dcterms:subject', label: 'Subject', count: 6 }] }),
        '/api/item_sets': (url) =>
          url.searchParams.get('page') === '2'
            ? listResponse([setNamed(16, 'Set 6')], 6)
            : listResponse([1, 2, 3, 4, 5].map((n) => setNamed(10 + n, `Set ${String(n)}`)), 6),
      }),
    );
    renderLinks();

    fireEvent.click(await screen.findByRole('button', { name: 'Subject · 6 item sets' }));
    expect(await screen.findByRole('link', { name: 'Set 1' })).toHaveAttribute('href', '#/items?set=11');
    expect(screen.getAllByRole('link', { name: /^Set / })).toHaveLength(5);
    fireEvent.click(screen.getByRole('button', { name: 'Show more' }));
    expect(await screen.findByRole('link', { name: 'Set 6' })).toHaveAttribute('href', '#/items?set=16');
    expect(screen.getAllByRole('link', { name: /^Set / })).toHaveLength(6);
    expect(screen.queryByRole('button', { name: 'Show more' })).not.toBeInTheDocument();

    const first = urlsFor(fetchMock, '/api/item_sets')[0];
    expect(first?.searchParams.get('property[0][type]')).toBe('res');
    expect(first?.searchParams.get('property[0][text]')).toBe('1');
    expect(first?.searchParams.get('property[0][property]')).toBe('3');
    expect(first?.searchParams.get('per_page')).toBe('5');
  });

  it('says so when nothing links to the record', async () => {
    installFetch(route({ '/api/romeka/links/1': () => jsonResponse(loadFixture('places/links-none.json')) }));
    renderLinks();
    expect(await screen.findByText('Nothing links to this record.')).toBeInTheDocument();
  });

  it('shows a loading line, then a failed read inline', async () => {
    let resolve: (response: Response) => void = () => undefined;
    const promise = new Promise<Response>((done) => {
      resolve = done;
    });
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>((input) =>
        requestUrl(input).pathname === '/api/romeka/links/1' ? promise : Promise.resolve(jsonResponse({}, { status: 404 })),
      ),
    );
    renderLinks();
    expect(await screen.findByRole('status')).toHaveTextContent('Counting what links here…');
    resolve(jsonResponse({ errors: { error: 'The links read failed' } }, { status: 500 }));
    expect(await screen.findByRole('alert')).toHaveTextContent('The links read failed');
  });
});

describe('linked from in the set card', () => {
  it('is collapsed until the reader opens it, and reads nothing before', async () => {
    window.history.replaceState(null, '', '/#/items?set=1');
    const fetchMock = installFetch(
      route({
        '/api/item_sets/1': () => jsonResponse(setNamed(1, 'Featured photographs')),
        '/api/items': () => listResponse([], 0),
        '/api/sites': () => listResponse([], 0),
        '/api/users': () => jsonResponse(loadFixture('users-page.json')),
        '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
        '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
        '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
        '/api/romeka/links/1': () => jsonResponse(loadFixture('places/links-none.json')),
      }),
    );
    render(<ItemsList />);

    const card = (await screen.findByRole('heading', { name: 'Featured photographs', level: 2 })).closest('section');
    if (card === null) {
      throw new Error('no set card');
    }
    const toggle = within(card).getByRole('button', { name: 'Linked from' });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(urlsFor(fetchMock, '/api/romeka/links/1')).toHaveLength(0);
    fireEvent.click(toggle);
    expect(await within(card).findByText('Nothing links to this record.')).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
  });
});
