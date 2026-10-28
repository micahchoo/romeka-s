/**
 * Which saved list the Items screen shows, for the nav and the Items screen's saved list line.
 * A link from the nav carries the list's name as `list=<name>`, and the screen keeps it through
 * filter and order changes, so the screen still knows where it came from after the reader
 * narrows it. Kept apart from `pages/items/browse.ts` because the nav is in the first paint.
 */
import type { SavedList } from '../api/types';

/** The Items address key that names the saved list the screen came from. */
export const LIST_KEY = 'list';

export interface OpenList {
  readonly list: SavedList;
  /** The filters or the order differ from the ones saved. */
  readonly changed: boolean;
}

/**
 * The saved list named by `name`, or else the one whose query is `query`. A name no list has
 * any more (deleted or renamed in another window) falls back to the query.
 */
export const openSavedList = (lists: readonly SavedList[], name: string | null, query: string): OpenList | null => {
  const named = name === null ? undefined : lists.find((list) => list.name === name);
  if (named !== undefined) {
    return { list: named, changed: named.query !== query };
  }
  const same = lists.find((list) => list.query === query);
  return same === undefined ? null : { list: same, changed: false };
};

/** The address that opens `list` as it was saved. */
export const savedListHref = (list: SavedList): string => {
  const query = new URLSearchParams(list.query);
  query.set(LIST_KEY, list.name);
  return `#/items?${query.toString()}`;
};
