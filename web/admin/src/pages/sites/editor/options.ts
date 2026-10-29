/**
 * The option lists every block field reads: the site's pages, the item sets
 * and the classes items use. The editor loads them once
 * and passes them down, so a page with several `pageRef` or `itemQuery` blocks
 * makes one request per list rather than one per control.
 */
import { api } from '../../../api/client';
import { placesApi } from '../../../api/places';
import type { ClassCount } from '../../../api/places';
import type { ItemSet, JsonValue, SiteId, SitePage } from '../../../api/types';

export interface EditorOptions {
  readonly pages: readonly SitePage[];
  readonly itemSets: readonly ItemSet[];
  /** The classes items use, with their counts, for a query over items. */
  readonly classCounts?: readonly ClassCount[];
  /** The site lists only items placed on its pages, so a block's count does too. */
  readonly attachedOnly?: boolean;
  /** The site's theme settings, which colour a colour field's theme swatches. */
  readonly themeSettings?: JsonValue | null;
}

export const loadEditorOptions = async (siteId: SiteId): Promise<EditorOptions> => {
  const [pages, itemSets, classCounts] = await Promise.all([
    api.listSitePages({ siteId, perPage: 200 }),
    api.listItemSets({ perPage: 200, sortBy: 'title' }),
    placesApi.classCounts(),
  ]);
  return { pages: pages.items, itemSets: itemSets.items, classCounts };
};
