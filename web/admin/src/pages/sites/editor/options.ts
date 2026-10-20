/**
 * The option lists every block field reads: the site's pages, the item sets and
 * the resource classes. The editor loads them once and passes them down, so a
 * page with several `pageRef` or `itemQuery` blocks makes one request per list
 * rather than one per control.
 */
import { api } from '../../../api/client';
import type { ItemSet, JsonValue, ResourceClass, SiteId, SitePage } from '../../../api/types';

export interface EditorOptions {
  readonly pages: readonly SitePage[];
  readonly itemSets: readonly ItemSet[];
  readonly classes: readonly ResourceClass[];
  /** The site's theme settings, which colour a colour field's theme swatches. */
  readonly themeSettings?: JsonValue | null;
}

export const loadEditorOptions = async (siteId: SiteId): Promise<EditorOptions> => {
  const [pages, itemSets, classes] = await Promise.all([
    api.listSitePages({ siteId, perPage: 200 }),
    api.listItemSets({ perPage: 200, sortBy: 'title' }),
    api.listResourceClasses({ perPage: 200, sortBy: 'label' }),
  ]);
  return { pages: pages.items, itemSets: itemSets.items, classes: classes.items };
};
