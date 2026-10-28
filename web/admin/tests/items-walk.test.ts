/**
 * Walking a result: the item page's address names the list it was opened from
 * (`in`, the browse query) and the item's place in it (`at`). These are the
 * pure halves: the address codec, the rows the item page reads, and where the
 * item is found among them.
 */
import { describe, expect, it } from 'vitest';
import { brandId, isRecord, parseItem } from '../src/api/parse';
import type { Item, ItemId } from '../src/api/types';
import { browseQuery, parseBrowse } from '../src/pages/items/browse';
import { itemPageHref, locate, parseWalk, WINDOW, windowPages, windowQuery } from '../src/pages/items/walk';
import type { Walk } from '../src/pages/items/walk';
import { loadFixture } from './loadFixture';

const itemId = (id: number): ItemId => brandId<'ItemId'>(id);

const walkOf = (query: string, at: number): Walk => ({ browse: parseBrowse(new URLSearchParams(query), 'table'), at });

const queryOf = (href: string): URLSearchParams => new URLSearchParams(href.slice(href.indexOf('?') + 1));

describe('the item page address', () => {
  it('carries the browse query and the position, and reads back to the same walk', () => {
    const walk = walkOf('set=4&q=harbour&sort=title', 12);
    const href = itemPageHref(itemId(7), { walk, edit: false });
    expect(href.startsWith('#/items/7?')).toBe(true);
    expect(queryOf(href).get('at')).toBe('12');
    expect(parseWalk(queryOf(href))).toEqual(walk);
  });

  it('drops the list page, which the position replaces', () => {
    const href = itemPageHref(itemId(7), { walk: walkOf('set=4&page=3', 50), edit: false });
    expect(queryOf(href).get('in')).toBe('set=4');
  });

  it('the plain Items list is an empty `in`, which is still a walk', () => {
    const href = itemPageHref(itemId(7), { walk: walkOf('', 1), edit: false });
    expect(queryOf(href).get('in')).toBe('');
    expect(parseWalk(queryOf(href))).toEqual(walkOf('', 1));
  });

  it('an address with no `in`, or a malformed position, is no walk', () => {
    expect(parseWalk(new URLSearchParams(''))).toBeNull();
    expect(parseWalk(new URLSearchParams('at=3'))).toBeNull();
    expect(parseWalk(new URLSearchParams('in=set%3D4'))).toBeNull();
    expect(parseWalk(new URLSearchParams('in=set%3D4&at=0'))).toBeNull();
    expect(parseWalk(new URLSearchParams('in=set%3D4&at=two'))).toBeNull();
  });

  it('asks for the edit form only when told to', () => {
    expect(queryOf(itemPageHref(itemId(7), { walk: walkOf('', 2), edit: true })).get('edit')).toBe('1');
    expect(queryOf(itemPageHref(itemId(7), { walk: walkOf('', 2), edit: false })).has('edit')).toBe(false);
    expect(itemPageHref(itemId(7), { walk: null, edit: false })).toBe('#/items/7');
  });
});

describe('the rows around the position', () => {
  it('reads the two rows each side in pages of the window size', () => {
    expect(WINDOW).toBe(5);
    expect(windowPages(1)).toEqual([1]);
    expect(windowPages(3)).toEqual([1]);
    expect(windowPages(4)).toEqual([1, 2]);
    expect(windowPages(8)).toEqual([2]);
    expect(windowPages(12)).toEqual([2, 3]);
  });

  it('sends the browse query with the window page in place of the list page', () => {
    const walk = walkOf('set=4&class=13&sort=title&page=9', 12);
    expect(windowQuery(walk, 3)).toEqual({ ...browseQuery(walk.browse), page: 3, perPage: WINDOW });
  });
});

const ITEM = loadFixture('item-1.json');

const row = (id: number): Item => {
  if (!isRecord(ITEM)) {
    throw new Error('expected an item fixture');
  }
  return parseItem({ ...ITEM, 'o:id': id, 'o:title': `Item ${String(id)}` });
};

/** Rows 1..n of a list whose item at position p has id 100 + p. */
const rows = (first: number, last: number, skip: number | null = null): ReadonlyMap<number, Item> => {
  const ids = Array.from({ length: last - first + 2 }, (_, index) => 100 + first + index).filter((id) => id !== skip);
  return new Map(ids.slice(0, last - first + 1).map((id, index) => [first + index, row(id)]));
};

describe('finding the item in the rows', () => {
  it('finds it at its position with the rows each side', () => {
    const found = locate(rows(10, 14), itemId(112), 12);
    expect(found?.at).toBe(12);
    expect(found?.previous?.id).toBe(itemId(111));
    expect(found?.next?.id).toBe(itemId(113));
  });

  it('the first row has no previous, the last no next', () => {
    expect(locate(rows(1, 3), itemId(101), 1)?.previous).toBeNull();
    const last = locate(rows(1, 3), itemId(103), 3);
    expect(last?.at).toBe(3);
    expect(last?.next).toBeNull();
  });

  it('finds it one place earlier when a row before it left the list', () => {
    // Item 111 was saved out of the list; 112 moved up to position 11.
    const found = locate(rows(10, 14, 111), itemId(112), 12);
    expect(found?.at).toBe(11);
    expect(found?.previous?.id).toBe(itemId(110));
    expect(found?.next?.id).toBe(itemId(113));
  });

  it('finds it one place later when a row came in before it', () => {
    const found = locate(rows(10, 14), itemId(112), 11);
    expect(found?.at).toBe(12);
  });

  it('finds nothing when the item is not near its position, so no number shows', () => {
    expect(locate(rows(10, 14), itemId(999), 12)).toBeNull();
    expect(locate(rows(10, 14), itemId(110), 13)).toBeNull();
    expect(locate(new Map(), itemId(112), 12)).toBeNull();
  });
});
