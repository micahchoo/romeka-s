/**
 * The generated site settings registry, narrowed once at module load.
 *
 * `src/generated/siteSettings.json` is written by
 * `crates/omeka-site/src/settings.rs` from `SITE_SETTINGS`, and a Rust test
 * keeps it byte-identical. The parser in `siteSettingsSchema` rejects a key
 * that is not in the catalogue.
 */
import raw from '../../generated/siteSettings.json';
import { parseSiteSettingsRegistry } from './siteSettingsSchema';
import type { SiteSettingsRegistry } from './siteSettingsSchema';

export const SITE_SETTINGS_REGISTRY: SiteSettingsRegistry = parseSiteSettingsRegistry(raw);
