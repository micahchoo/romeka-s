/**
 * What a list block's stored query reaches: how many resources it matches and,
 * when it matches none on a site, whether that site has any of the kind at all.
 * The inspector's count line and the Outline row's "Lists nothing yet" both
 * read it, so the two never disagree.
 *
 * Each count is `Omeka-S-Total-Results` on the list endpoint of the resource
 * searched (`/api/items`, `/api/item_sets` or `/api/media`), which the client
 * reads into `ApiPage.total`; a request asks for one row and uses only the
 * header. It sends the whole stored query, every pair in its stored order,
 * then what the public render adds (`browse_preview.rs::search`): the
 * attached-items test when the site lists only items placed on its pages, and
 * `site_id`. The public render searches with the same parameters the list
 * endpoints read, so the count is the block's own total. The site's own total
 * is asked only after a zero, with `site_id` alone (`site_items_probe`), so a
 * count above zero costs one request.
 */
import { api } from '../../../../api/client';
import type { ResourceQuery } from '../../../../api/client';
import { listMedia } from '../../../../api/media';
import type { ApiPage, JsonValue, SiteId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import type { AsyncState } from '../../../../app/useAsync';
import { useSettled } from '../../../../app/useSettled';
import { isJsonObject } from '../values';
import { parseQuery } from './query';

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

/** The site a block lists on, as the public render reads it. */
export interface ListSite {
  readonly id: SiteId;
  /** The site setting "Only items placed on pages" (`browse_attached_items`). */
  readonly attachedOnly: boolean;
}

/** The site setting "Only items placed on pages", read as the server's `SiteSettings` reads it. */
export const listsAttachedOnly = (siteSettings: JsonValue | null): boolean =>
  isJsonObject(siteSettings) && siteSettings['browse_attached_items'] === true;

/** The stored pairs in order; a repeated name keeps every value, in order, for the server to read. */
const storedQuery = (value: string): Record<string, string | readonly string[]> => {
  const query: Record<string, string | readonly string[]> = {};
  for (const pair of parseQuery(value)) {
    const prior = query[pair.key];
    query[pair.key] = prior === undefined ? pair.value : [...(typeof prior === 'string' ? [prior] : prior), pair.value];
  }
  return query;
};

/** The list request one stored query makes: the block's own search, one row, for the total header. */
const countQuery = (value: string, site: ListSite | undefined): ResourceQuery => ({
  ...storedQuery(value),
  ...(site?.attachedOnly === true ? { site_attachments_only: 'true' } : {}),
  ...(site === undefined ? {} : { site_id: site.id }),
  perPage: 1,
});

/** Asks the server what `value` reaches, scoped to `site` when given. */
export const readReach = async (
  resource: QueryResource,
  value: string,
  site: ListSite | undefined,
): Promise<ListReach> => {
  const { list } = LISTS[resource];
  const count = (await list(countQuery(value, site))).total;
  if (count > 0) {
    return { kind: 'matches', count };
  }
  if (site === undefined) {
    return { kind: 'none' };
  }
  const onSite = (await list({ perPage: 1, site_id: site.id })).total;
  return onSite === 0 ? { kind: 'siteEmpty' } : { kind: 'filterEmpty' };
};

/** How long a typed query waits before it is counted. */
const COUNT_DELAY_MS = 400;

/** What `value` reaches, asked again once an edit settles; `null` asks nothing. */
export const useListReach = (
  resource: QueryResource,
  value: string | null,
  site: ListSite | undefined,
): AsyncState<ListReach | null> => {
  const settled = useSettled(value, COUNT_DELAY_MS);
  const siteId = site?.id;
  const attachedOnly = site?.attachedOnly ?? false;
  return useAsync(
    async () =>
      settled === null
        ? null
        : readReach(resource, settled, siteId === undefined ? undefined : { id: siteId, attachedOnly }),
    [resource, settled, siteId, attachedOnly],
  );
};
