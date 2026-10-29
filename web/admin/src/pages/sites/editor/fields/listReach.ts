/**
 * What a list block's stored query reaches: how many resources it matches and,
 * when it matches none on a site, whether that site has any of the kind at all.
 * The inspector's count line and the Outline row's "Lists nothing yet" both
 * read it, so the two never disagree.
 *
 * Each count is `Omeka-S-Total-Results` on the list endpoint of the resource
 * searched (`/api/items`, `/api/item_sets` or `/api/media`), which the client
 * reads into `ApiPage.total`; a request asks for one row and uses only the
 * header. The site's own total is asked only after a zero, with `site_id`
 * alone, so a count above zero costs one request. Only items can be filtered
 * by item set (`QueryFilter::push_item`), so only their count carries it.
 */
import { api } from '../../../../api/client';
import type { ResourceQuery } from '../../../../api/client';
import { listMedia } from '../../../../api/media';
import type { ApiPage, SiteId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import type { AsyncState } from '../../../../app/useAsync';
import { useSettled } from '../../../../app/useSettled';
import { QUERY_KEYS, parseQuery, queryId, queryValue } from './query';

/** The resource types a browse query can search (`browsePreview.resource_type`). */
export type QueryResource = 'items' | 'item_sets' | 'media';

export const isQueryResource = (value: unknown): value is QueryResource =>
  value === 'items' || value === 'item_sets' || value === 'media';

/** What one stored query reaches. A site-scoped zero says which zero it is. */
export type ListReach =
  | { readonly kind: 'matches'; readonly count: number }
  /** Zero, with no site to ask about. */
  | { readonly kind: 'none' }
  /** Zero, because the site has no resource of this kind. */
  | { readonly kind: 'siteEmpty' }
  /** Zero, though the site has some: the filter excludes them all. */
  | { readonly kind: 'filterEmpty' };

const LISTS: Readonly<
  Record<
    QueryResource,
    {
      readonly list: (query: ResourceQuery) => Promise<ApiPage<unknown>>;
      readonly byItemSet: boolean;
    }
  >
> = {
  items: { list: (query) => api.listItems(query), byItemSet: true },
  item_sets: { list: (query) => api.listItemSets(query), byItemSet: false },
  media: { list: (query) => listMedia(api, query), byItemSet: false },
};

/** Whether the resource's builder offers the item set filter. */
export const filtersByItemSet = (resource: QueryResource): boolean => LISTS[resource].byItemSet;

/** The list request one stored query makes: one row, for the total header. */
const countQuery = (value: string, siteId: SiteId | undefined, byItemSet: boolean): ResourceQuery => {
  const pairs = parseQuery(value);
  const fulltext = queryValue(pairs, QUERY_KEYS.fulltext).trim();
  const itemSet = queryId(pairs, QUERY_KEYS.itemSet);
  const resourceClass = queryId(pairs, QUERY_KEYS.resourceClass);
  const sortBy = queryValue(pairs, QUERY_KEYS.sortBy).trim();
  const sortOrder = queryValue(pairs, QUERY_KEYS.sortOrder).trim();
  return {
    perPage: 1,
    ...(siteId === undefined ? {} : { site_id: siteId }),
    ...(fulltext === '' ? {} : { fulltext_search: fulltext }),
    ...(itemSet === null || !byItemSet ? {} : { item_set_id: itemSet }),
    ...(resourceClass === null ? {} : { resource_class_id: resourceClass }),
    ...(sortBy === '' ? {} : { sortBy }),
    ...(sortOrder === 'asc' || sortOrder === 'desc' ? { sortOrder } : {}),
  };
};

/** Asks the server what `value` reaches, scoped to `siteId` when given. */
export const readReach = async (
  resource: QueryResource,
  value: string,
  siteId: SiteId | undefined,
): Promise<ListReach> => {
  const { list, byItemSet } = LISTS[resource];
  const count = (await list(countQuery(value, siteId, byItemSet))).total;
  if (count > 0) {
    return { kind: 'matches', count };
  }
  if (siteId === undefined) {
    return { kind: 'none' };
  }
  const onSite = (await list({ perPage: 1, site_id: siteId })).total;
  return onSite === 0 ? { kind: 'siteEmpty' } : { kind: 'filterEmpty' };
};

/**
 * How many resources `value` shows a visitor of the site: the same count with
 * `is_public=1`, one request. The site check asks this, not [`readReach`], so
 * a list that matches only private items counts as showing nothing.
 */
export const readVisitorCount = async (
  resource: QueryResource,
  value: string,
  siteId: SiteId,
): Promise<number> => {
  const { list, byItemSet } = LISTS[resource];
  return (await list({ ...countQuery(value, siteId, byItemSet), is_public: 1 })).total;
};

/** How long a typed query waits before it is counted. */
const COUNT_DELAY_MS = 400;

/** What `value` reaches, asked again once an edit settles; `null` asks nothing. */
export const useListReach = (
  resource: QueryResource,
  value: string | null,
  siteId: SiteId | undefined,
): AsyncState<ListReach | null> => {
  const settled = useSettled(value, COUNT_DELAY_MS);
  return useAsync(
    async () => (settled === null ? null : readReach(resource, settled, siteId)),
    [resource, settled, siteId],
  );
};
