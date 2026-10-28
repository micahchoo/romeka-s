/**
 * Walking a result: an item opened from the Items list keeps that list in its
 * address, `#/items/<id>?in=<browse query>&at=<position>`, so its page can say
 * "12 of 382" and link the items before and after it. `in` is the list's own
 * URL query (`browse.ts`), so a filter the list learns walks with no change here.
 * Every function is pure; the item page owns the reads.
 */
import type { ResourceQuery } from '../../api/client';
import type { Item, ItemId } from '../../api/types';
import { browseQuery, formatBrowse, parseBrowse } from './browse';
import type { ItemBrowse } from './browse';

/** A list and a 1-based position in it. */
export interface Walk {
  readonly browse: ItemBrowse;
  readonly at: number;
}

/** The item page's own state in its address: the walk, and whether the edit form opens. */
export interface ItemPageState {
  readonly walk: Walk | null;
  readonly edit: boolean;
}

/** Rows read around a position, two each side, in pages of this size: at most two requests. */
export const WINDOW = 5;

const positionOf = (text: string | null): number | null =>
  text !== null && /^[1-9]\d{0,14}$/.test(text) ? Number(text) : null;

/** The walk an item page's query names, or null when it names none or a broken one. */
export const parseWalk = (query: URLSearchParams): Walk | null => {
  const list = query.get('in');
  const at = positionOf(query.get('at'));
  return list === null || at === null ? null : { browse: parseBrowse(new URLSearchParams(list), 'table'), at };
};

export const itemPageHref = (itemId: ItemId, { walk, edit }: ItemPageState): string => {
  const query = new URLSearchParams();
  if (walk !== null) {
    // The position names the row; the list page it sat on is not part of the list.
    query.set('in', formatBrowse({ ...walk.browse, page: 1 }).toString());
    query.set('at', String(walk.at));
  }
  if (edit) {
    query.set('edit', '1');
  }
  const search = query.toString();
  return `#/items/${String(itemId)}${search === '' ? '' : `?${search}`}`;
};

/** The window pages that hold the rows from two before `at` to two after it. */
export const windowPages = (at: number): readonly number[] => {
  const first = Math.ceil(Math.max(1, at - 2) / WINDOW);
  const last = Math.ceil((at + 2) / WINDOW);
  return first === last ? [first] : [first, last];
};

export const windowQuery = (walk: Walk, page: number): ResourceQuery => ({
  ...browseQuery(walk.browse),
  page,
  perPage: WINDOW,
});

/** The item's position found in the rows read around it, with its neighbours. */
export interface Located {
  readonly at: number;
  readonly previous: Item | null;
  readonly next: Item | null;
}

/**
 * Where `itemId` sits among `rows` (keyed by position): at `at`, or one before
 * or after it when a row ahead left or joined the list since the link was made,
 * as a save in a "field is empty" list does. Anywhere else is null, so the page
 * shows no number rather than a wrong one.
 */
export const locate = (rows: ReadonlyMap<number, Item>, itemId: ItemId, at: number): Located | null => {
  const found = [at, at - 1, at + 1].find((position) => rows.get(position)?.id === itemId);
  return found === undefined
    ? null
    : { at: found, previous: rows.get(found - 1) ?? null, next: rows.get(found + 1) ?? null };
};
