/**
 * The item picker, through what a curator sees and presses: rows with their
 * metadata from the list response, the keyboard across the three columns,
 * paging by "Show more", the tray of picks, whole-set picks, and the empty,
 * slow and failed states.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactElement } from 'react';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import type { ItemId, ItemSetId } from '../src/api/types';
import { ItemPicker } from '../src/pages/sites/editor/fields/ItemPicker';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestUrl, requestsFor } from './stub';
import type { Handler } from './stub';

const literal = (term: string, label: string, text: string): Record<string, unknown> => ({
  [term]: [{ type: 'literal', property_id: 1, property_label: label, is_public: true, '@value': text }],
});

/** One item as `/api/items` answers it. */
const itemJson = (id: number, title: string, extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  '@context': 'http://localhost/api-context',
  '@id': `http://localhost/api/items/${String(id)}`,
  '@type': ['o:Item', 'dctype:StillImage'],
  'o:id': id,
  'o:is_public': true,
  'o:owner': null,
  'o:resource_class': { '@id': 'http://localhost/api/resource_classes/13', 'o:id': 13 },
  'o:resource_template': null,
  'o:thumbnail': null,
  'o:title': title,
  thumbnail_display_urls: { large: null, medium: null, square: null },
  'o:created': { '@value': '2026-10-16T09:30:00+00:00', '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
  'o:modified': null,
  'o:primary_media': null,
  'o:media': [],
  'o:item_set': [],
  'o:site': [],
  ...literal('dcterms:title', 'Title', title),
  ...extra,
});

const MAP = itemJson(12, 'Harbour map', {
  ...literal('dcterms:creator', 'Creator', 'A. Surveyor'),
  ...literal('dcterms:date', 'Date', '1881'),
  'o:media': [
    { '@id': 'http://localhost/api/media/7', 'o:id': 7 },
    { '@id': 'http://localhost/api/media/8', 'o:id': 8 },
  ],
  'o:item_set': [{ '@id': 'http://localhost/api/item_sets/1', 'o:id': 1 }],
});
const LETTER = itemJson(9, 'Letter home', literal('dcterms:identifier', 'Identifier', 'L-9'));

/** `count` items, ids counting down from 100. */
const many = (count: number): Record<string, unknown>[] =>
  Array.from({ length: count }, (_, at) => itemJson(100 - at, `Item ${String(100 - at)}`));

interface Archive {
  /** Every item the archive holds, for counts and lookups by id. */
  readonly rows: readonly Record<string, unknown>[];
  /** The answer to a results query, before paging; `null` answers `rows`. */
  readonly results?: (url: URL) => readonly Record<string, unknown>[] | Response | null;
}

/** The API the picker reads, over one archive. */
const archive =
  ({ rows, results }: Archive): Handler =>
  (url) => {
    const params = url.searchParams;
    switch (url.pathname) {
      case '/api/item_sets':
        return listResponse(loadFixture('item-sets-page.json'), 2);
      case '/api/resource_classes':
        return listResponse(loadFixture('resource-classes-page.json'), 2);
      case '/api/media':
        return listResponse([], 0);
      case '/api/items': {
        if (params.has('id')) {
          const ids = (params.get('id') ?? '').split(',');
          const found = rows.filter((entry) => ids.includes(String(entry['o:id'])));
          return listResponse(found, found.length);
        }
        if (params.get('per_page') === '1') {
          return listResponse(rows.slice(0, 1), params.has('item_set_id') ? 1 : rows.length);
        }
        const answer = results?.(url) ?? rows;
        if (answer instanceof Response) {
          return answer;
        }
        const page = Number(params.get('page') ?? '1');
        const size = Number(params.get('per_page') ?? '24');
        return listResponse(answer.slice((page - 1) * size, page * size), answer.length);
      }
      default:
        return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
    }
  };

interface Picks {
  items: readonly ItemId[];
  sets: readonly ItemSetId[];
}

const Many = ({
  picks,
  initial = [],
  withSets = false,
}: {
  readonly picks: Picks;
  readonly initial?: readonly ItemId[];
  readonly withSets?: boolean;
}): ReactElement => {
  const [items, setItems] = useState<readonly ItemId[]>(initial);
  const [sets, setSets] = useState<readonly ItemSetId[]>([]);
  const onItems = (next: readonly ItemId[]): void => {
    setItems(next);
    picks.items = next;
  };
  return withSets ? (
    <ItemPicker
      id="picker"
      label="Find items"
      mode="many"
      picked={items}
      onChange={onItems}
      sets={{
        picked: sets,
        onChange: (next) => {
          setSets(next);
          picks.sets = next;
        },
      }}
    />
  ) : (
    <ItemPicker id="picker" label="Find items" mode="many" picked={items} onChange={onItems} />
  );
};

const results = () => screen.getByRole('listbox', { name: 'Find items' });
const row = (name: string) => within(results()).getByRole('option', { name });
const tray = () => screen.getByRole('region', { name: 'Picked' });
const noPicks = (): Picks => ({ items: [], sets: [] });

describe('ItemPicker', () => {
  it('shows each row with its key values, class, sets and media from the list response', async () => {
    const fetchMock = installFetch(archive({ rows: [MAP, LETTER] }));
    render(<Many picks={noPicks()} />);

    const map = await within(results()).findByRole('option', { name: 'Harbour map' });
    expect(map).toHaveTextContent('A. Surveyor · 1881');
    expect(
      await within(map).findByText('Still Image · In Featured photographs · Media: 2'),
    ).toBeInTheDocument();
    expect(row('Letter home')).toHaveTextContent('L-9');
    expect(screen.getByText('2 items match')).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(([input]) => requestUrl(input).pathname.startsWith('/api/items/')),
      'no row reads its item one by one',
    ).toBe(false);
  });

  it('moves with the arrows, picks with Space, and steps into the peek and the sources', async () => {
    installFetch(archive({ rows: [MAP, LETTER] }));
    const picks = noPicks();
    render(<Many picks={picks} />);

    const map = await within(results()).findByRole('option', { name: 'Harbour map' });
    expect(map).toHaveAttribute('tabindex', '0');
    expect(row('Letter home')).toHaveAttribute('tabindex', '-1');
    map.focus();
    fireEvent.keyDown(map, { key: 'ArrowDown' });
    const letter = row('Letter home');
    expect(letter).toHaveFocus();
    fireEvent.keyDown(letter, { key: ' ' });
    expect(picks.items).toEqual([9]);
    expect(letter).toHaveAttribute('aria-selected', 'true');

    const peek = screen.getByRole('region', { name: 'Details' });
    expect(within(peek).getByRole('heading', { name: 'Letter home' })).toBeInTheDocument();
    expect(within(peek).getByText('L-9')).toBeInTheDocument();
    expect(within(peek).getByRole('link', { name: 'Open in admin' })).toHaveAttribute('href', '#/items/9');
    fireEvent.keyDown(letter, { key: 'ArrowRight' });
    expect(peek).toHaveFocus();
    fireEvent.keyDown(peek, { key: 'ArrowLeft' });
    expect(letter).toHaveFocus();
    fireEvent.keyDown(letter, { key: 'ArrowLeft' });
    expect(screen.getByRole('button', { name: /^All items/ })).toHaveFocus();
  });

  it('shows the total, then adds the next page under the rows already shown', async () => {
    const fetchMock = installFetch(archive({ rows: many(30) }));
    render(<Many picks={noPicks()} />);

    expect(await screen.findByText('30 items match')).toBeInTheDocument();
    expect(screen.getByText('24 of 30')).toBeInTheDocument();
    fireEvent.click(row('Item 100'));
    fireEvent.click(screen.getByRole('button', { name: 'Show 6 more' }));

    expect(await screen.findByText('30 of 30')).toBeInTheDocument();
    expect(within(results()).getAllByRole('option')).toHaveLength(30);
    expect(row('Item 100')).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByRole('button', { name: /^Show \d+ more$/ })).not.toBeInTheDocument();
    const pages = requestsFor(fetchMock, '/api/items')
      .map((request) => request.url.searchParams)
      .filter((params) => params.get('per_page') === '24')
      .map((params) => params.get('page'));
    expect(pages).toEqual(['1', '2']);
  });

  it('keeps picks outside the results in the tray, in order, and moves and removes them', async () => {
    const shown = many(24);
    installFetch(
      archive({
        rows: [...shown, MAP, LETTER],
        results: (url) => (url.searchParams.get('search') === 'letter' ? [LETTER] : shown),
      }),
    );
    const picks = noPicks();
    render(<Many picks={picks} initial={[brandId<'ItemId'>(12)]} />);

    expect(await within(tray()).findByText('Harbour map')).toBeInTheDocument();
    expect(within(results()).queryByRole('option', { name: 'Harbour map' })).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Search titles and values'), { target: { value: 'letter' } });
    fireEvent.click(await within(results()).findByRole('option', { name: 'Letter home' }));
    expect(picks.items).toEqual([12, 9]);
    expect(within(tray()).getByText('Picked: 2')).toBeInTheDocument();

    const letterChip = within(tray()).getByText('Letter home');
    fireEvent.keyDown(letterChip, { key: 'ArrowLeft', altKey: true });
    expect(picks.items).toEqual([9, 12]);
    fireEvent.click(within(tray()).getByRole('button', { name: 'Remove Harbour map' }));
    expect(picks.items).toEqual([9]);
  });

  it('picks a whole set only when the caller asks for sets', async () => {
    installFetch(archive({ rows: [MAP, LETTER] }));
    const picks = noPicks();
    const { unmount } = render(<Many picks={picks} withSets />);

    fireEvent.click(
      await screen.findByRole('button', { name: 'Pick the whole set Featured photographs' }),
    );
    expect(picks.sets).toEqual([1]);
    expect(within(tray()).getByText('Set: Featured photographs')).toBeInTheDocument();
    fireEvent.click(
      within(tray()).getByRole('button', { name: 'Remove the set Featured photographs' }),
    );
    expect(picks.sets).toEqual([]);
    unmount();

    render(<Many picks={picks} />);
    expect(await screen.findByRole('button', { name: /^Featured photographs/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Pick the whole set/ })).not.toBeInTheDocument();
  });

  it('says when nothing matches and clears the filters', async () => {
    const fetchMock = installFetch(
      archive({
        rows: [MAP, LETTER],
        results: (url) => (url.searchParams.has('search') ? [] : null),
      }),
    );
    render(<Many picks={noPicks()} />);

    await within(results()).findByRole('option', { name: 'Harbour map' });
    fireEvent.change(screen.getByLabelText('Search titles and values'), { target: { value: 'zebra' } });
    expect(await screen.findByText('No item matches.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Clear the filters' }));

    expect(await within(results()).findByRole('option', { name: 'Harbour map' })).toBeInTheDocument();
    expect(screen.getByLabelText('Search titles and values')).toHaveValue('');
    const last = requestsFor(fetchMock, '/api/items')
      .filter((request) => request.url.searchParams.get('per_page') === '24')
      .at(-1);
    expect(last?.url.searchParams.has('search')).toBe(false);
  });

  it('keeps the picks through a failed read and retries it', async () => {
    let failing = true;
    installFetch(
      archive({
        rows: [MAP, LETTER],
        results: () =>
          failing ? jsonResponse({ errors: { error: 'database is locked' } }, { status: 500 }) : null,
      }),
    );
    render(<Many picks={noPicks()} initial={[brandId<'ItemId'>(9)]} />);

    expect(await screen.findByText('The items could not be read.')).toBeInTheDocument();
    expect(await within(tray()).findByText('Letter home')).toBeInTheDocument();
    failing = false;
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));

    expect(await within(results()).findByRole('option', { name: 'Letter home' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(screen.queryByText('The items could not be read.')).not.toBeInTheDocument();
    expect(within(tray()).getByText('Picked: 1')).toBeInTheDocument();
  });

  it('holds the rows with skeletons only once a read takes over a second', async () => {
    let answer: (response: Response) => void = () => undefined;
    const slow = new Promise<Response>((resolve) => {
      answer = resolve;
    });
    const handler = archive({ rows: [MAP] });
    const fetchMock = installFetch(handler);
    fetchMock.mockImplementation((input, init) => {
      const url = requestUrl(input);
      return url.pathname === '/api/items' && url.searchParams.get('per_page') === '24'
        ? slow
        : Promise.resolve(handler(url, init));
    });
    const { container } = render(<Many picks={noPicks()} />);
    const skeletons = () => container.querySelectorAll('.item-picker-skeleton');

    expect(await screen.findByText('Searching…')).toBeInTheDocument();
    expect(skeletons()).toHaveLength(0);
    await act(() => new Promise((resolve) => setTimeout(resolve, 1100)));
    expect(skeletons().length).toBeGreaterThan(0);

    answer(listResponse([MAP], 1));
    expect(await within(results()).findByRole('option', { name: 'Harbour map' })).toBeInTheDocument();
    expect(skeletons()).toHaveLength(0);
  });

  it('keeps one pick in single mode, the last one made', async () => {
    installFetch(archive({ rows: [MAP, LETTER] }));
    let picked: ItemId | null = null;
    const One = (): ReactElement => {
      const [value, setValue] = useState<ItemId | null>(null);
      return (
        <ItemPicker
          id="one"
          label="Find items"
          mode="one"
          picked={value}
          onChange={(next) => {
            setValue(next);
            picked = next;
          }}
        />
      );
    };
    render(<One />);

    fireEvent.click(await within(results()).findByRole('option', { name: 'Harbour map' }));
    fireEvent.click(row('Letter home'));
    expect(picked).toBe(9);
    expect(row('Harbour map')).toHaveAttribute('aria-selected', 'false');
    expect(results()).toHaveAttribute('aria-multiselectable', 'false');
    await waitFor(() => {
      expect(within(tray()).getByText('Picked: 1')).toBeInTheDocument();
    });
  });
});
