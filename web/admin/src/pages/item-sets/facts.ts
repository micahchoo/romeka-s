/**
 * What the admin states about one item set beside the set itself: how many
 * items it holds and which sites show it. Every read runs as the caller, so a
 * count is the total of the list it stands for, and a set the caller may not
 * read is "not available" whether it is hidden or gone.
 */
import { api, ApiError } from '../../api/client';
import type { ItemSet, ItemSetId, SiteId } from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';

export interface SiteName {
  readonly id: SiteId;
  readonly title: string;
}

export interface SetCounts {
  /** `GET /api/items?item_set_id=<id>` total for the caller. */
  readonly itemCount: number;
  /** The sites whose `o:site_item_set` names the set, among those the caller may see. */
  readonly sites: readonly SiteName[];
}

export interface SetFacts extends SetCounts {
  readonly set: ItemSet;
}

/** The address of a set: the Items screen filtered to it, under its card. */
export const setPlaceHref = (id: ItemSetId): string => `#/items?set=${String(id)}`;

export const setEditHref = (id: ItemSetId): string => `#/item-sets/${String(id)}/edit`;

export const loadSetCounts = async (id: ItemSetId): Promise<SetCounts> => {
  const [items, sites] = await Promise.all([
    api.listItems({ itemSetId: id, perPage: 1 }),
    allPages((page) => api.listSites({ page, perPage: ALL_PAGES_PER_PAGE })),
  ]);
  return {
    itemCount: items.total,
    sites: sites
      .filter((site) => site.siteItemSets.some((entry) => entry.itemSet.id === id))
      .map((site) => ({ id: site.id, title: site.title })),
  };
};

export const loadSetFacts = async (id: ItemSetId): Promise<SetFacts> => {
  const [set, counts] = await Promise.all([api.getItemSet(id), loadSetCounts(id)]);
  return { set, ...counts };
};

/** A refusal and a missing row answer the same, so neither tells the caller the set exists. */
export const isUnavailable = (error: Error): boolean =>
  error instanceof ApiError && (error.status === 403 || error.status === 404);
