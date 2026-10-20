/**
 * The site's theme settings as one write: what the Theme page and the page
 * editor's site-wide inspector both read from the site and PATCH back as
 * `o:theme_settings`. The map replaces the stored one whole, so both writers
 * send every key the Theme page owns, the resource page sections included.
 */
import { api } from '../../../api/client';
import { isRecord } from '../../../api/parse';
import type { JsonValue, Site, SiteId } from '../../../api/types';
import { encodeSiteThemeSettings } from '../../../api/write';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from '../resourcePageBlocksRegistry';
import { resourcePageBlocksWire } from '../resourcePageBlocksSchema';
import type { ResourcePageBlocksDraft } from '../resourcePageBlocksSchema';
import { draftWire } from './draft';
import type { ThemeDraft } from './draft';

/** The stored map, or an empty one when the site stores none. */
export const storedThemeSettings = (value: JsonValue | null): Readonly<Record<string, unknown>> =>
  isRecord(value) ? value : {};

/** The map a save writes and a preview sends: the draft plus the resource page sections. */
export const themeSettingsWrite = (
  draft: ThemeDraft,
  blocks: ResourcePageBlocksDraft,
): Record<string, JsonValue> => ({
  ...draftWire(draft),
  resource_page_blocks: resourcePageBlocksWire(RESOURCE_PAGE_BLOCKS_REGISTRY, blocks),
});

/** PATCH the site's theme settings; the answer is the site as stored. */
export const saveThemeSettings = (
  siteId: SiteId,
  settings: Readonly<Record<string, JsonValue>>,
): Promise<Site> => api.patchSite(siteId, encodeSiteThemeSettings(settings));
