/**
 * The generated theme token registry, narrowed once at module load.
 *
 * `src/generated/themeSettings.json` is written by
 * `crates/omeka-site/src/theme/tokens.rs` from `SETTINGS`, and a Rust test
 * keeps it byte-identical. The parser in `themeSettingsSchema` rejects a key
 * that is not in the catalogue.
 */
import raw from '../../generated/themeSettings.json';
import { parseThemeSettingsRegistry } from './themeSettingsSchema';
import type { ThemeSettingsRegistry } from './themeSettingsSchema';

export const THEME_SETTINGS_REGISTRY: ThemeSettingsRegistry = parseThemeSettingsRegistry(raw);
