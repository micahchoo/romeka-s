/**
 * The header's jump-to box: `#<id>` opens that record, other text opens the one record whose
 * identifier, then title, is that text, or lists up to ten. Every read is the caller's own
 * (`/api/resources/<id>` and the `eq`/`in` property searches), so a hidden record answers like a
 * missing one.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { JumpBox } from '../src/app/jump-box';
import { installFetch, jsonResponse, listResponse, route, urlsFor } from './stub';
import type { Handler } from './stub';

type Kind = 'items' | 'item_sets' | 'media';
const TYPE: Readonly<Record<Kind, string>> = { items: 'o:Item', item_sets: 'o:ItemSet', media: 'o:Media' };

const doc = (kind: Kind, id: number, title: string | null): Record<string, unknown> => ({
  '@id': `http://localhost/api/${kind}/${String(id)}`,
  '@type': TYPE[kind],
  'o:id': id,
  'o:title': title,
});

/** Records by kind, each with the identifier and title the property search compares. */
type Shelf = Readonly<Record<Kind, readonly { readonly id: number; readonly identifier?: string; readonly title: string | null }[]>>;

const search = (kind: Kind, shelf: Shelf): Handler => (url) => {
  const term = url.searchParams.get('property[0][property]');
  const type = url.searchParams.get('property[0][type]');
  const text = url.searchParams.get('property[0][text]') ?? '';
  const matches = shelf[kind].filter((record) => {
    const value = term === 'dcterms:identifier' ? record.identifier : record.title;
    if (value === undefined || value === null) {
      return false;
    }
    return type === 'eq' ? value === text : value.toLowerCase().includes(text.toLowerCase());
  });
  const perPage = Number(url.searchParams.get('per_page') ?? '25');
  return listResponse(matches.slice(0, perPage).map((record) => doc(kind, record.id, record.title)), matches.length);
};

const install = (shelf: Shelf) =>
  installFetch(
    route({
      '/api/items': search('items', shelf),
      '/api/item_sets': search('item_sets', shelf),
      '/api/media': search('media', shelf),
      '/api/resources/12': () => jsonResponse(doc('items', 12, 'Ledger')),
      '/api/resources/30': () => jsonResponse(doc('media', 30, 'Scan of the ledger')),
      '/api/resources/77': () => jsonResponse({ errors: { error: 'Permission denied' } }, { status: 403 }),
    }),
  );

const SHELF: Shelf = {
  items: [
    { id: 12, identifier: 'AF_1', title: 'Ledger' },
    { id: 13, identifier: 'AF_2', title: 'Letter to Mary' },
    { id: 14, title: 'Letter to Mary' },
    { id: 15, title: 'Box 1' },
    { id: 16, title: 'Box 10' },
  ],
  item_sets: [{ id: 20, title: 'Letters' }],
  media: [{ id: 30, identifier: 'AF_2/1', title: 'Scan of the ledger' }],
};

const box = (): HTMLElement => screen.getByRole('searchbox', { name: 'Jump to a record' });

const jump = (text: string): void => {
  fireEvent.change(box(), { target: { value: text } });
  fireEvent.submit(box());
};

describe('jump-to box', () => {
  beforeEach(() => {
    window.history.replaceState(null, '', '/#/');
  });

  afterEach(() => {
    window.history.replaceState(null, '', '/#/');
  });

  it('opens a record by #id, whatever its kind, and clears the box', async () => {
    install(SHELF);
    render(<JumpBox />);
    jump('#30');
    await waitFor(() => {
      expect(window.location.hash).toBe('#/media/30');
    });
    expect(box()).toHaveValue('');
    jump('#12');
    await waitFor(() => {
      expect(window.location.hash).toBe('#/items/12');
    });
  });

  it('answers a hidden id as it answers a missing one', async () => {
    install(SHELF);
    render(<JumpBox />);
    jump('#77');
    expect(await screen.findByText('No record that you can see matches “#77”.')).toBeInTheDocument();
    jump('#404');
    expect(await screen.findByText('No record that you can see matches “#404”.')).toBeInTheDocument();
  });

  it('opens the one record whose identifier is the text, before any title', async () => {
    const fetchMock = install(SHELF);
    render(<JumpBox />);
    jump('AF_2');
    await waitFor(() => {
      expect(window.location.hash).toBe('#/items/13');
    });
    expect(urlsFor(fetchMock, '/api/items').every((url) => url.searchParams.get('property[0][type]') === 'eq')).toBe(true);
  });

  it('lists every record of one exact title, with its kind', async () => {
    install(SHELF);
    render(<JumpBox />);
    jump('Letter to Mary');
    const list = await screen.findByRole('list', { name: '2 records match “Letter to Mary”' });
    expect(list.querySelectorAll('a')).toHaveLength(2);
    expect(screen.getAllByRole('link', { name: /Letter to Mary/ })[0]).toHaveAttribute('href', '#/items/13');
    expect(list).toHaveTextContent('Item #13');
    expect(list).toHaveTextContent('Item #14');
    expect(window.location.hash).toBe('#/');
  });

  it('answers the next text after the reader follows a listed record', async () => {
    install(SHELF);
    render(<JumpBox />);
    jump('Letter to Mary');
    const list = await screen.findByRole('list', { name: '2 records match “Letter to Mary”' });
    fireEvent.click(within(list).getAllByRole('link')[0] ?? list);
    await waitFor(() => {
      expect(screen.queryByRole('list')).toBeNull();
    });
    jump('zzz');
    expect(await screen.findByText('No record that you can see matches “zzz”.')).toBeInTheDocument();
  });

  it('falls back to titles that contain the text, and an item set opens its place', async () => {
    install(SHELF);
    render(<JumpBox />);
    jump('letter');
    const list = await screen.findByRole('list', { name: '3 records match “letter”' });
    expect(list).toHaveTextContent('Item set');
    expect(screen.getByRole('link', { name: /Letters/ })).toHaveAttribute('href', '#/items?set=20');
    jump('box 1');
    expect(await screen.findByRole('list', { name: '2 records match “box 1”' })).toBeInTheDocument();
  });

  it('shows ten at most and says how many matched', async () => {
    const many = Array.from({ length: 14 }, (_, n) => ({ id: 100 + n, title: `Folder ${String(n)}` }));
    install({ items: many, item_sets: [], media: [] });
    render(<JumpBox />);
    jump('folder');
    const list = await screen.findByRole('list', { name: '14 records match “folder”' });
    expect(list.querySelectorAll('a')).toHaveLength(10);
    expect(screen.getByText('The first 10 are listed. Type more of the title to narrow them.')).toBeInTheDocument();
  });

  it('says so when nothing matches, and Escape clears the box', async () => {
    install(SHELF);
    render(<JumpBox />);
    jump('zzz');
    expect(await screen.findByText('No record that you can see matches “zzz”.')).toBeInTheDocument();
    fireEvent.keyDown(box(), { key: 'Escape' });
    expect(box()).toHaveValue('');
    expect(screen.queryByText(/No record/)).toBeNull();
  });

  it('takes focus on "/" unless the reader is typing in a field', () => {
    install(SHELF);
    render(
      <>
        <input aria-label="Other field" />
        <JumpBox />
      </>,
    );
    fireEvent.keyDown(document.body, { key: '/' });
    expect(box()).toHaveFocus();
    const other = screen.getByRole('textbox', { name: 'Other field' });
    other.focus();
    fireEvent.keyDown(other, { key: '/' });
    expect(other).toHaveFocus();
  });
});
