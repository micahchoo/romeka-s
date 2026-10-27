/**
 * The links place: `#/items?links=<id>&via=<property>` lists the items that link to one record
 * under that record's card, with a chip that names it. A hidden or missing record gets one
 * message and no rows. A stubbed fetch serves the fixtures through the real client and parsers.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { ItemsList } from '../src/pages/items/list';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestUrl, route, urlsFor } from './stub';
import type { Handler } from './stub';

const firstOf = (fixture: string): Record<string, unknown> => {
  const page = loadFixture(fixture);
  const first: unknown = isUnknownArray(page) ? page[0] : null;
  return isRecord(first) ? { ...first } : {};
};

const CREATOR = { 'o:id': 2, 'o:term': 'dcterms:creator', 'o:label': 'Creator' };

interface LinksServer {
  readonly record: 'ok' | 'missing';
  /** The linking items: `per_page=1` answers this total, the page two rows. */
  readonly count: number;
}

const handler = ({ record, count }: LinksServer): Handler =>
  route({
    '/api/resources/1': () =>
      record === 'ok'
        ? jsonResponse(loadFixture('places/record-item.json'))
        : jsonResponse({ errors: { error: 'Not found' } }, { status: 404 }),
    '/api/properties/2': () => jsonResponse({ ...firstOf('properties-page.json'), ...CREATOR }),
    '/api/items': (url) =>
      url.searchParams.get('per_page') === '1' ? listResponse([], count) : listResponse(loadFixture('items-page.json'), count),
    '/api/item_sets/4': () => jsonResponse({ ...firstOf('item-sets-page.json'), 'o:id': 4, 'o:title': 'Letters' }),
    '/api/sites': () => listResponse([], 0),
    '/api/users': () => jsonResponse(loadFixture('users-page.json')),
    '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
    '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
    '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  });

const open = (query: string, server: LinksServer) => {
  window.history.replaceState(null, '', `/#/items?${query}`);
  const fetchMock = installFetch(handler(server));
  render(<ItemsList />);
  return fetchMock;
};

describe('the links place', () => {
  it("heads the linking items with the record's card: its title, the field, the count, a link to it", async () => {
    const fetchMock = open('links=1&via=2', { record: 'ok', count: 2 });

    const heading = await screen.findByRole('heading', { name: 'Items that link to Person A', level: 2 });
    const card = heading.closest('section') ?? document.body;
    expect(within(card).getByText('As Creator')).toBeInTheDocument();
    expect(within(card).getByText('2 items')).toBeInTheDocument();
    expect(within(card).getByRole('link', { name: 'Open the record' })).toHaveAttribute('href', '#/items/1');
    expect(screen.getByText('Links to: Person A (as Creator)')).toBeInTheDocument();
    expect(screen.queryByText(/items match$/)).not.toBeInTheDocument();

    const list = urlsFor(fetchMock, '/api/items').find((url) => url.searchParams.get('per_page') === '24');
    expect(list?.searchParams.get('property[0][type]')).toBe('res');
    expect(list?.searchParams.get('property[0][text]')).toBe('1');
    expect(list?.searchParams.get('property[0][property]')).toBe('2');
  });

  it('names no field when the filter has none', async () => {
    open('links=1', { record: 'ok', count: 3 });
    expect(await screen.findByText('Through any field')).toBeInTheDocument();
    expect(screen.getByText('Links to: Person A')).toBeInTheDocument();
  });

  it('names the record by its id while it loads', async () => {
    window.history.replaceState(null, '', '/#/items?links=1&via=2');
    const served = handler({ record: 'ok', count: 2 });
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>((input, init) => {
        const url = requestUrl(input);
        return url.pathname === '/api/resources/1' ? new Promise<Response>(() => undefined) : Promise.resolve(served(url, init));
      }),
    );
    render(<ItemsList />);

    expect(await screen.findByText('Loading the record…')).toBeInTheDocument();
    expect(await screen.findByText('Links to: Record #1 (as #2)')).toBeInTheDocument();
  });

  it('a hidden or missing record gets one message, a chip that says so, and no rows', async () => {
    open('links=1&via=2', { record: 'missing', count: 2 });

    expect(await screen.findByText('This record is not available.')).toBeInTheDocument();
    expect(await screen.findByText('Links to: Record #1 (not available)')).toBeInTheDocument();
    expect(document.querySelector('.items-results')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Change items…' })).not.toBeInTheDocument();
  });

  it('beside a set filter shows both cards, the set first, each counting what the other narrows', async () => {
    open('set=4&links=1&via=2', { record: 'ok', count: 2 });

    const links = await screen.findByRole('heading', { name: 'Items that link to Person A', level: 2 });
    const set = await screen.findByRole('heading', { name: 'Letters', level: 2 });
    expect(set.compareDocumentPosition(links) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const linksCard = links.closest('section') ?? document.body;
    expect(await within(linksCard).findByText('2 match the other filters')).toBeInTheDocument();
  });
});
