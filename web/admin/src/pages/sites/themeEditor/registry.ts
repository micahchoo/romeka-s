/**
 * The theme registry: the tokens the Theme page edits, the five presets with
 * every value, the colour roles and the contrast pairs.
 *
 * `src/generated/themeSettings.json` is written by
 * `crates/omeka-site/src/theme/tokens_schema.rs`, and a Rust test keeps it
 * byte-identical. It arrives from outside the SPA's types, so it is `unknown`
 * until `parseThemeRegistry` narrows it; a message key `./messages.ts` lacks
 * fails the parse.
 */
import { ParseError, isRecord, isUnknownArray, parseListOf } from '../../../api/parse';
import type { Path } from '../../../api/types';
import { isThemeMessageKey } from './messages';
import type { ThemeMessageKey } from './messages';
import raw from '../../../generated/themeSettings.json';
import { COLOUR_ROLES } from './palette';
import type { ColourRole, Palette, Recipe, Role } from './palette';

interface SettingBase {
  readonly key: string;
  readonly labelKey: ThemeMessageKey;
  readonly helpKey: ThemeMessageKey;
}

/** One token as the Theme page renders it, by control. */
export type ThemeSetting =
  | (SettingBase & { readonly kind: 'toggle' | 'colour' | 'asset' | 'text' | 'html' })
  | (SettingBase & { readonly kind: 'font' | 'choice'; readonly choices: readonly string[] })
  | (SettingBase & {
      readonly kind: 'number';
      readonly min: number;
      readonly max: number;
      readonly step: number;
    })
  | (SettingBase & { readonly kind: 'css'; readonly maxBytes: number });

/** A preset's tokens other than its palette, by stored key. */
export type PresetTokens = Readonly<Record<string, string | number>>;

/** One preset with every value. */
export interface Preset {
  readonly id: string;
  readonly labelKey: ThemeMessageKey;
  readonly descriptionKey: ThemeMessageKey;
  readonly recipe: Recipe;
  readonly light: Palette;
  readonly dark: Palette;
  readonly tokens: PresetTokens;
}

export interface ThemeRegistry {
  readonly newSitePreset: string;
  readonly settings: readonly ThemeSetting[];
  readonly presets: readonly Preset[];
  /**
   * Each colour role with the key a site stores it under, and the key of its
   * dark value under the automatic scheme (`null` for the brand).
   */
  readonly colours: readonly {
    readonly role: ColourRole;
    readonly key: string;
    readonly darkKey: string | null;
  }[];
  readonly pairs: readonly (readonly [Role, Role])[];
  readonly aa: number;
}

const str = (value: unknown, path: Path): string => {
  if (typeof value !== 'string') {
    throw new ParseError(path, 'string');
  }
  return value;
};

const num = (value: unknown, path: Path): number => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new ParseError(path, 'number');
  }
  return value;
};

const messageKey = (value: unknown, path: Path): ThemeMessageKey => {
  const key = str(value, path);
  if (!isThemeMessageKey(key)) {
    throw new ParseError(path, `a catalogue key, got ${key}`);
  }
  return key;
};

const strings = (value: unknown, path: Path): readonly string[] => {
  if (!isUnknownArray(value) || !value.every((item) => typeof item === 'string')) {
    throw new ParseError(path, 'a list of strings');
  }
  return value;
};

const isColourRole = (value: string): value is ColourRole =>
  COLOUR_ROLES.some((role) => role === value);

const colourRole = (value: unknown, path: Path): ColourRole => {
  const role = str(value, path);
  if (!isColourRole(role)) {
    throw new ParseError(path, 'a colour role');
  }
  return role;
};

const role = (value: unknown, path: Path): Role => {
  const name = str(value, path);
  return name === 'on-brand' ? name : colourRole(name, path);
};

const record = (value: unknown, path: Path): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return value;
};

const parseSetting = (value: unknown, path: Path): ThemeSetting => {
  const object = record(value, path);
  const base = {
    key: str(object.key, [...path, 'key']),
    labelKey: messageKey(object.labelKey, [...path, 'labelKey']),
    helpKey: messageKey(object.helpKey, [...path, 'helpKey']),
  };
  const kind = str(object.kind, [...path, 'kind']);
  switch (kind) {
    case 'toggle':
    case 'colour':
    case 'asset':
    case 'text':
    case 'html':
      return { ...base, kind };
    case 'font':
    case 'choice':
      return { ...base, kind, choices: strings(object.choices, [...path, 'choices']) };
    case 'number':
      return {
        ...base,
        kind,
        min: num(object.min, [...path, 'min']),
        max: num(object.max, [...path, 'max']),
        step: num(object.step, [...path, 'step']),
      };
    case 'css':
      return { ...base, kind, maxBytes: num(object.maxBytes, [...path, 'maxBytes']) };
    default:
      throw new ParseError([...path, 'kind'], 'a known token kind');
  }
};

const parsePalette = (value: unknown, path: Path): Palette => {
  const object = record(value, path);
  const colour = (name: ColourRole): string => str(object[name], [...path, name]);
  return {
    brand: colour('brand'),
    accent: colour('accent'),
    background: colour('background'),
    surface: colour('surface'),
    text: colour('text'),
    muted: colour('muted'),
    link: colour('link'),
  };
};

const parsePreset = (value: unknown, path: Path): Preset => {
  const object = record(value, path);
  const recipe = record(object.recipe, [...path, 'recipe']);
  const tokens = record(object.tokens, [...path, 'tokens']);
  const parsedTokens: Record<string, string | number> = {};
  for (const [key, token] of Object.entries(tokens)) {
    parsedTokens[key] = typeof token === 'number' ? token : str(token, [...path, 'tokens', key]);
  }
  return {
    id: str(object.id, [...path, 'id']),
    labelKey: messageKey(object.labelKey, [...path, 'labelKey']),
    descriptionKey: messageKey(object.descriptionKey, [...path, 'descriptionKey']),
    recipe: {
      tint: num(recipe.tint, [...path, 'recipe', 'tint']),
      paper: num(recipe.paper, [...path, 'recipe', 'paper']),
      ink: num(recipe.ink, [...path, 'recipe', 'ink']),
    },
    light: parsePalette(object.light, [...path, 'light']),
    dark: parsePalette(object.dark, [...path, 'dark']),
    tokens: parsedTokens,
  };
};

/** Narrow the generated `themeSettings.json` into the typed registry. */
export const parseThemeRegistry = (value: unknown): ThemeRegistry => {
  const object = record(value, []);
  return {
    newSitePreset: str(object.newSitePreset, ['newSitePreset']),
    settings: parseListOf(parseSetting)(object.settings, ['settings']),
    presets: parseListOf(parsePreset)(object.presets, ['presets']),
    colours: parseListOf((entry: unknown, path: Path) => {
      const colour = record(entry, path);
      return {
        role: colourRole(colour.role, [...path, 'role']),
        key: str(colour.key, [...path, 'key']),
        darkKey: colour.darkKey === null ? null : str(colour.darkKey, [...path, 'darkKey']),
      };
    })(object.colours, ['colours']),
    pairs: parseListOf((entry: unknown, path: Path): readonly [Role, Role] => {
      if (!isUnknownArray(entry) || entry.length !== 2) {
        throw new ParseError(path, 'a pair');
      }
      return [role(entry[0], [...path, 0]), role(entry[1], [...path, 1])];
    })(object.pairs, ['pairs']),
    aa: num(object.aa, ['aa']),
  };
};

/** The registry, narrowed once at module load. */
export const THEME_REGISTRY: ThemeRegistry = parseThemeRegistry(raw);

/** A token by key. */
export const settingFor = (key: string): ThemeSetting | undefined =>
  THEME_REGISTRY.settings.find((setting) => setting.key === key);

/** A preset by id, falling back to the first (plain). */
export const presetFor = (id: string): Preset => {
  const found = THEME_REGISTRY.presets.find((preset) => preset.id === id);
  const fallback = THEME_REGISTRY.presets[0];
  if (found !== undefined) {
    return found;
  }
  if (fallback === undefined) {
    throw new Error('the theme registry lists no preset');
  }
  return fallback;
};
