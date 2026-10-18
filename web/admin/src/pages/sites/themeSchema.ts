/**
 * The bundled theme's settings schema, the controls the theme settings form
 * renders.
 *
 * The theme registry (`crates/omeka-site/src/theme/mod.rs`) declares its
 * settings as `ThemeSetting` values, and the bundled theme's list is
 * `crates/omeka-site/src/settings.rs::SETTINGS`. `omeka-api` and the SPA do not
 * read that crate, so the list is mirrored here the way `read/meta.rs` mirrors
 * the data type registry, with the same keys, kinds and defaults. A change to
 * `SETTINGS` needs a matching change here.
 */
import type { MessageKey } from '../../i18n';

export type ThemeSettingKind = 'text' | 'boolean' | 'choice';

export interface ThemeSettingSpec {
  /** The `site_setting` name, e.g. `installation_title`. */
  readonly key: string;
  readonly labelKey: MessageKey;
  readonly kind: ThemeSettingKind;
  /** The values a choice offers, in schema order. */
  readonly choices?: readonly string[];
  /** The accessor's default; `null` when the layer supplies no default. */
  readonly defaultValue: string | boolean | null;
}

export const THEME_SETTINGS_SCHEMA: readonly ThemeSettingSpec[] = [
  {
    key: 'installation_title',
    labelKey: 'themeSetting.installationTitle',
    kind: 'text',
    defaultValue: 'Romeka S',
  },
  {
    key: 'locale',
    labelKey: 'themeSetting.locale',
    kind: 'choice',
    choices: ['en', 'es'],
    defaultValue: 'en',
  },
  {
    key: 'attachment_link_type',
    labelKey: 'themeSetting.attachmentLinkType',
    kind: 'choice',
    choices: ['item', 'media', 'original'],
    defaultValue: 'item',
  },
  {
    key: 'browse_attached_items',
    labelKey: 'themeSetting.browseAttachedItems',
    kind: 'boolean',
    defaultValue: false,
  },
  {
    key: 'browse_heading_property_term',
    labelKey: 'themeSetting.browseHeadingProperty',
    kind: 'text',
    defaultValue: null,
  },
  {
    key: 'browse_body_property_term',
    labelKey: 'themeSetting.browseBodyProperty',
    kind: 'text',
    defaultValue: null,
  },
  {
    key: 'filter_locale_values',
    labelKey: 'themeSetting.filterLocaleValues',
    kind: 'boolean',
    defaultValue: false,
  },
  {
    key: 'exclude_resources_not_in_site',
    labelKey: 'themeSetting.excludeResourcesNotInSite',
    kind: 'boolean',
    defaultValue: false,
  },
  {
    key: 'iiif_viewer_theme',
    labelKey: 'themeSetting.iiifViewerTheme',
    kind: 'choice',
    choices: ['light', 'dark'],
    defaultValue: 'light',
  },
  {
    key: 'subnav_display',
    labelKey: 'themeSetting.subnavDisplay',
    kind: 'choice',
    choices: ['show', 'hide'],
    defaultValue: null,
  },
  {
    key: 'show_page_pagination',
    labelKey: 'themeSetting.showPagePagination',
    kind: 'boolean',
    defaultValue: true,
  },
];

/** A setting's value while the form is open. */
export type ThemeSettingDraft = string | boolean;

/**
 * The stored map as drafts. A key with no stored value takes the schema
 * default; an unset default is an empty control.
 */
export const themeSettingDrafts = (
  stored: Readonly<Record<string, unknown>> | null,
): Readonly<Record<string, ThemeSettingDraft>> => {
  const drafts: Record<string, ThemeSettingDraft> = {};
  for (const setting of THEME_SETTINGS_SCHEMA) {
    const value = stored?.[setting.key];
    if (typeof value === 'boolean') {
      drafts[setting.key] = value;
    } else if (typeof value === 'string') {
      drafts[setting.key] = value;
    } else if (typeof setting.defaultValue === 'boolean') {
      drafts[setting.key] = setting.defaultValue;
    } else {
      drafts[setting.key] = setting.defaultValue ?? '';
    }
  }
  return drafts;
};

/**
 * The drafts as the `o:theme_settings` object. An empty text or choice value
 * stores nothing for its key, so the accessor's default applies when the public
 * site reads it.
 */
export const themeSettingsJson = (
  drafts: Readonly<Record<string, ThemeSettingDraft>>,
): Readonly<Record<string, string | boolean>> => {
  const settings: Record<string, string | boolean> = {};
  for (const setting of THEME_SETTINGS_SCHEMA) {
    const value = drafts[setting.key];
    if (value === undefined) {
      continue;
    }
    if (typeof value === 'boolean') {
      settings[setting.key] = value;
      continue;
    }
    const trimmed = value.trim();
    if (trimmed !== '') {
      settings[setting.key] = trimmed;
    }
  }
  return settings;
};
