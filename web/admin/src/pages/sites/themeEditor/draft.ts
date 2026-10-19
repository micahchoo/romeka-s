/**
 * The Theme page's draft: what a curator has chosen, the effective tokens it
 * resolves to, the contrast pairs it passes or fails, and the map a save
 * writes.
 *
 * The draft is the stored shape `crates/omeka-site/src/theme/tokens.rs`
 * reads: a preset, whether the palette derives from the brand colour, the
 * tokens set by hand, the header and footer content, and the custom CSS.
 * `resolve` repeats `tokens::resolve`, and the resolution samples in
 * `src/generated/themeSamples.json` pin the two together.
 */
import type { JsonValue } from '../../../api/types';
import { isRecord } from '../../../api/parse';
import { isHexColour, nearestPassing, normaliseHex, contrast } from './colour';
import { derive, roleColour } from './palette';
import type { ColourRole, Palette, PairCheck, Shade } from './palette';
import { THEME_REGISTRY, presetFor } from './registry';
import type { ThemeSetting } from './registry';

/** The header and footer content keys, edited beside the tokens. */
export const CONTENT_KEYS = ['logo', 'banner', 'banner_text', 'footer_text'] as const;
export type ContentKey = (typeof CONTENT_KEYS)[number];

/** A token value set by hand: a hex colour, a choice key or a number. */
export type TokenValue = string | number;

export interface ThemeDraft {
  readonly preset: string;
  readonly derive: boolean;
  /** The tokens set by hand, by stored key. Absent means "the preset's". */
  readonly overrides: Readonly<Record<string, TokenValue>>;
  readonly content: Readonly<Record<ContentKey, string>>;
  readonly customCss: string;
}

/** Every token a curator may set by hand: colours, choices and numbers. */
const isToken = (setting: ThemeSetting): boolean =>
  setting.key !== 'preset' &&
  (setting.kind === 'colour' ||
    setting.kind === 'font' ||
    setting.kind === 'choice' ||
    setting.kind === 'number');

export const TOKEN_SETTINGS: readonly ThemeSetting[] = THEME_REGISTRY.settings.filter(isToken);

const PRESET_IDS: ReadonlySet<string> = new Set(THEME_REGISTRY.presets.map((preset) => preset.id));

/** A stored value read the way `Choices::from_value` reads it, `null` when refused. */
export const tokenValue = (setting: ThemeSetting, stored: unknown): TokenValue | null => {
  switch (setting.kind) {
    case 'colour':
      return typeof stored === 'string' ? normaliseHex(stored) : null;
    case 'font':
    case 'choice':
      return typeof stored === 'string' && setting.choices.includes(stored) ? stored : null;
    case 'number': {
      const number =
        typeof stored === 'number'
          ? stored
          : typeof stored === 'string' && /^\s*\d+\s*$/.test(stored)
            ? Number(stored)
            : Number.NaN;
      return Number.isInteger(number) && number >= setting.min && number <= setting.max
        ? number
        : null;
    }
    default:
      return null;
  }
};

const contentValue = (stored: unknown): string => {
  if (typeof stored === 'number' && Number.isFinite(stored)) {
    return String(stored);
  }
  return typeof stored === 'string' ? stored : '';
};

/**
 * The draft a stored map opens as. A map with no preset is one stored before
 * presets: the plain preset with derivation off, as the theme reads it.
 * Overrides equal to what the preset already gives are dropped, so a map the
 * old Theme page saved in full opens with only its real changes marked.
 */
export const draftFromStored = (stored: Readonly<Record<string, unknown>>): ThemeDraft => {
  const preset =
    typeof stored['preset'] === 'string' && PRESET_IDS.has(stored['preset'])
      ? stored['preset']
      : null;
  const deriveStored = stored['derive_palette'];
  const overrides: Record<string, TokenValue> = {};
  for (const setting of TOKEN_SETTINGS) {
    const value = tokenValue(setting, stored[setting.key]);
    if (value !== null) {
      overrides[setting.key] = value;
    }
  }
  const content = {
    logo: contentValue(stored['logo']),
    banner: contentValue(stored['banner']),
    banner_text: contentValue(stored['banner_text']),
    footer_text: contentValue(stored['footer_text']),
  };
  const draft: ThemeDraft = {
    preset: preset ?? 'plain',
    derive: preset === null ? false : typeof deriveStored === 'boolean' ? deriveStored : true,
    overrides,
    content,
    customCss: typeof stored['custom_css'] === 'string' ? stored['custom_css'] : '',
  };
  return withoutRedundantOverrides(draft);
};

/** The draft with every override that changes nothing removed. */
const withoutRedundantOverrides = (draft: ThemeDraft): ThemeDraft => {
  const kept: Record<string, TokenValue> = {};
  for (const [key, value] of Object.entries(draft.overrides)) {
    const before = resolve(setOverride(draft, key, null));
    const after = resolve(draft);
    if (JSON.stringify(before) !== JSON.stringify(after)) {
      kept[key] = value;
    }
  }
  return { ...draft, overrides: kept };
};

/** The effective tokens a draft resolves to. */
export interface Resolved {
  readonly scheme: string;
  readonly light: Palette;
  readonly dark: Palette;
  /** Every non-colour token, by stored key. */
  readonly tokens: Readonly<Record<string, TokenValue>>;
}

const colourKey = (role: ColourRole): string =>
  THEME_REGISTRY.colours.find((colour) => colour.role === role)?.key ?? `${role}_color`;

const override = (draft: ThemeDraft, key: string): TokenValue | undefined => draft.overrides[key];

/**
 * The preset, then a palette derived from the brand when derivation is on,
 * then the overrides, the way `tokens::resolve` does it. A colour override
 * lands on the palette the page shows first.
 */
export const resolve = (draft: ThemeDraft): Resolved => {
  const preset = presetFor(draft.preset);
  const schemeOverride = override(draft, 'color_scheme');
  const scheme =
    typeof schemeOverride === 'string' ? schemeOverride : String(preset.tokens['color_scheme']);
  const brandOverride = override(draft, colourKey('brand'));
  const brand = typeof brandOverride === 'string' ? normaliseHex(brandOverride) : null;
  let light: Record<ColourRole, string>;
  let dark: Record<ColourRole, string>;
  if (draft.derive) {
    const from = brand ?? preset.light.brand;
    light = { ...derive(from, preset.recipe, 'light') };
    dark = { ...derive(from, preset.recipe, 'dark') };
  } else {
    light = { ...preset.light };
    dark = { ...preset.dark };
    if (brand !== null) {
      light.brand = brand;
      dark.brand = brand;
    }
  }
  const shown = scheme === 'dark' ? dark : light;
  for (const { role, key } of THEME_REGISTRY.colours) {
    const value = override(draft, key);
    const colour = typeof value === 'string' ? normaliseHex(value) : null;
    if (colour !== null) {
      shown[role] = colour;
    }
  }
  const tokens: Record<string, TokenValue> = {};
  for (const setting of TOKEN_SETTINGS) {
    if (setting.kind !== 'colour') {
      const value = preset.tokens[setting.key];
      tokens[setting.key] = override(draft, setting.key) ?? value ?? '';
    }
  }
  return { scheme, light, dark, tokens };
};

/** The shades a scheme shows. */
export const shownShades = (scheme: string): readonly Shade[] =>
  scheme === 'auto' ? ['light', 'dark'] : scheme === 'dark' ? ['dark'] : ['light'];

/**
 * Every contrast pair in every shade the scheme shows. A failing pair whose
 * foreground a curator may set carries the nearest colour that passes against
 * every background that foreground sits on.
 */
export const contrastChecks = (resolved: Resolved): readonly PairCheck[] =>
  shownShades(resolved.scheme).flatMap((shade) => {
    const palette = shade === 'dark' ? resolved.dark : resolved.light;
    return THEME_REGISTRY.pairs.map(([foreground, background]): PairCheck => {
      const fg = roleColour(palette, foreground);
      const ratio = contrast(fg, roleColour(palette, background));
      const passes = ratio >= THEME_REGISTRY.aa;
      const grounds = THEME_REGISTRY.pairs
        .filter(([other]) => other === foreground)
        .map(([, ground]) => roleColour(palette, ground));
      return {
        shade,
        foreground,
        background,
        ratio,
        passes,
        suggestion:
          passes || foreground === 'on-brand'
            ? null
            : nearestPassing(fg, grounds, THEME_REGISTRY.aa),
      };
    });
  });

/** Whether a draft may be saved: every hand-set colour is a hex and every pair passes. */
export const draftProblems = (draft: ThemeDraft): { readonly badHex: boolean; readonly failing: number } => ({
  badHex: THEME_REGISTRY.colours.some(({ key }) => {
    const value = draft.overrides[key];
    return value !== undefined && !isHexColour(value);
  }),
  failing: contrastChecks(resolve(draft)).filter((check) => !check.passes).length,
});

const trimmedOrNull = (value: string): string | null => {
  const text = value.trim();
  return text === '' ? null : text;
};

const assetId = (value: string): number | null => {
  const id = Number(value.trim());
  return value.trim() !== '' && Number.isInteger(id) && id > 0 ? id : null;
};

/** The design half of the map: preset, derivation, overrides and custom CSS. */
const designWire = (draft: ThemeDraft): Record<string, JsonValue> => {
  const map: Record<string, JsonValue> = {
    preset: draft.preset,
    derive_palette: draft.derive,
  };
  for (const setting of TOKEN_SETTINGS) {
    const value = tokenValue(setting, draft.overrides[setting.key]);
    if (value !== null) {
      map[setting.key] = value;
    }
  }
  const css = draft.customCss.trim();
  if (css !== '') {
    map['custom_css'] = css;
  }
  return map;
};

/** The theme settings map a save writes and a preview sends. */
export const draftWire = (draft: ThemeDraft): Record<string, JsonValue> => ({
  ...designWire(draft),
  logo: assetId(draft.content.logo),
  banner: assetId(draft.content.banner),
  banner_text: trimmedOrNull(draft.content.banner_text),
  footer_text: trimmedOrNull(draft.content.footer_text),
});

/** Apply a preset: its every value, with no token set by hand. */
export const applyPreset = (draft: ThemeDraft, preset: string): ThemeDraft => ({
  ...draft,
  preset,
  overrides: {},
});

/** Set or clear one token by hand. */
export const setOverride = (
  draft: ThemeDraft,
  key: string,
  value: TokenValue | null,
): ThemeDraft => {
  const overrides = Object.fromEntries(
    Object.entries(draft.overrides).filter(([other]) => other !== key),
  );
  return { ...draft, overrides: value === null ? overrides : { ...overrides, [key]: value } };
};

/** The marker an exported theme file carries. */
const EXPORT_FORMAT = 'romeka-theme';

/** A theme file: the design half of the map, without the site's own assets. */
export const exportTheme = (draft: ThemeDraft): string =>
  `${JSON.stringify({ format: EXPORT_FORMAT, version: 1, theme: designWire(draft) }, null, 2)}\n`;

/**
 * The draft an exported theme file gives, keeping the site's own header and
 * footer content. `null` when the text is not a theme file.
 */
export const importTheme = (text: string, current: ThemeDraft): ThemeDraft | null => {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  if (!isRecord(parsed) || parsed['format'] !== EXPORT_FORMAT || !isRecord(parsed['theme'])) {
    return null;
  }
  const imported = draftFromStored(parsed['theme']);
  return { ...imported, content: current.content };
};
