/**
 * The theme token schema format and its parser: the shape of
 * `src/generated/themeSettings.json`, which `crates/omeka-site/src/theme/tokens.rs`
 * generates from `SETTINGS` and a Rust test keeps in step.
 *
 * The JSON arrives from outside the SPA, so it is `unknown` until
 * [`parseThemeSettingsRegistry`] narrows it. A `labelKey` outside the catalogue
 * fails the parse, the way the site settings registry does.
 */
import { ParseError, isRecord, isUnknownArray, parseListOf } from '../../api/parse';
import type { JsonValue, Path } from '../../api/types';
import { MESSAGE_KEYS } from '../../i18n';
import type { MessageKey } from '../../i18n';

const MESSAGE_KEY_SET: ReadonlySet<string> = new Set(MESSAGE_KEYS);

const isMessageKey = (value: string): value is MessageKey => MESSAGE_KEY_SET.has(value);

const parseString = (value: unknown, path: Path): string => {
  if (typeof value !== 'string') {
    throw new ParseError(path, 'string');
  }
  return value;
};

const parseMessageKey = (value: unknown, path: Path): MessageKey => {
  const key = parseString(value, path);
  if (!isMessageKey(key)) {
    throw new ParseError(path, `a catalogue key, got ${key}`);
  }
  return key;
};

const parseNumber = (value: unknown, path: Path): number => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new ParseError(path, 'number');
  }
  return value;
};

const parseStringList = (value: unknown, path: Path): readonly string[] => {
  if (!isUnknownArray(value) || !value.every((item) => typeof item === 'string')) {
    throw new ParseError(path, 'a list of strings');
  }
  return value;
};

interface ThemeSettingBase {
  /** The key in the theme settings map. */
  readonly key: string;
  /** The catalogue key the form labels the control with. */
  readonly labelKey: MessageKey;
}

/** One theme token as the admin reads it, by the control it renders. */
export type ThemeSettingSpec =
  | (ThemeSettingBase & { readonly kind: 'colour'; readonly default: string; readonly dark: string })
  | (ThemeSettingBase & {
      readonly kind: 'font';
      readonly choices: readonly string[];
      readonly default: string;
    })
  | (ThemeSettingBase & {
      readonly kind: 'number';
      readonly min: number;
      readonly max: number;
      readonly step: number;
      readonly default: number;
    })
  | (ThemeSettingBase & {
      readonly kind: 'choice';
      readonly choices: readonly string[];
      readonly default: string;
    })
  | (ThemeSettingBase & { readonly kind: 'asset'; readonly default: number | null })
  | (ThemeSettingBase & { readonly kind: 'text'; readonly default: string | null })
  | (ThemeSettingBase & { readonly kind: 'html'; readonly default: string | null });

/** The whole generated registry. */
export interface ThemeSettingsRegistry {
  readonly settings: readonly ThemeSettingSpec[];
}

/** A token's value while the form is open. */
export type ThemeSettingDraft = string | number;

const parseSpec = (value: unknown, path: Path): ThemeSettingSpec => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  const base = {
    key: parseString(value.key, [...path, 'key']),
    labelKey: parseMessageKey(value.labelKey, [...path, 'labelKey']),
  };
  switch (parseString(value.kind, [...path, 'kind'])) {
    case 'colour':
      return {
        ...base,
        kind: 'colour',
        default: parseString(value.default, [...path, 'default']),
        dark: parseString(value.dark, [...path, 'dark']),
      };
    case 'font':
      return {
        ...base,
        kind: 'font',
        choices: parseStringList(value.choices, [...path, 'choices']),
        default: parseString(value.default, [...path, 'default']),
      };
    case 'number':
      return {
        ...base,
        kind: 'number',
        min: parseNumber(value.min, [...path, 'min']),
        max: parseNumber(value.max, [...path, 'max']),
        step: parseNumber(value.step, [...path, 'step']),
        default: parseNumber(value.default, [...path, 'default']),
      };
    case 'choice':
      return {
        ...base,
        kind: 'choice',
        choices: parseStringList(value.choices, [...path, 'choices']),
        default: parseString(value.default, [...path, 'default']),
      };
    case 'asset':
      return {
        ...base,
        kind: 'asset',
        default:
          value.default === null ? null : parseNumber(value.default, [...path, 'default']),
      };
    case 'text':
      return {
        ...base,
        kind: 'text',
        default: value.default === null ? null : parseString(value.default, [...path, 'default']),
      };
    case 'html':
      return {
        ...base,
        kind: 'html',
        default: value.default === null ? null : parseString(value.default, [...path, 'default']),
      };
    default:
      throw new ParseError([...path, 'kind'], 'a known token kind');
  }
};

/** Narrows the generated `themeSettings.json` into the typed registry. */
export const parseThemeSettingsRegistry: (raw: unknown) => ThemeSettingsRegistry = (raw) => {
  if (!isRecord(raw)) {
    throw new ParseError([], 'an object');
  }
  return { settings: parseListOf(parseSpec)(raw.settings, ['settings']) };
};

/** The hex colour a control accepts, matching `HexColor::parse`. */
const HEX_COLOUR = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;

/** Whether a draft value is a hex colour the token accepts. */
export const isHexColour = (value: unknown): value is string =>
  typeof value === 'string' && HEX_COLOUR.test(value.trim());

const isStoredNumber = (value: unknown): value is number =>
  typeof value === 'number' && Number.isFinite(value);

const specOf = (registry: ThemeSettingsRegistry, key: string): ThemeSettingSpec | undefined =>
  registry.settings.find((spec) => spec.key === key);

/** The colour scheme the stored map asks for, or the schema's default. */
const schemeOf = (
  registry: ThemeSettingsRegistry,
  stored: Readonly<Record<string, unknown>>,
): string => {
  const spec = specOf(registry, 'color_scheme');
  if (spec === undefined || spec.kind !== 'choice') {
    return 'light';
  }
  const value = stored['color_scheme'];
  return typeof value === 'string' && spec.choices.includes(value) ? value : spec.default;
};

/** The draft one token starts from: the stored value when it has the right shape. */
const draftValue = (
  spec: ThemeSettingSpec,
  stored: unknown,
  dark: boolean,
): ThemeSettingDraft => {
  switch (spec.kind) {
    case 'colour':
      return isHexColour(stored) ? stored : dark ? spec.dark : spec.default;
    case 'font':
    case 'choice':
      return typeof stored === 'string' && spec.choices.includes(stored)
        ? stored
        : spec.default;
    case 'number':
      return isStoredNumber(stored) ? stored : spec.default;
    case 'asset': {
      if (isStoredNumber(stored)) {
        return String(stored);
      }
      if (typeof stored === 'string' && stored.trim() !== '') {
        return stored;
      }
      return spec.default === null ? '' : String(spec.default);
    }
    case 'text':
    case 'html':
      return typeof stored === 'string' ? stored : (spec.default ?? '');
  }
};

/** Every token's draft, from the stored map. */
export const themeSettingDrafts = (
  registry: ThemeSettingsRegistry,
  stored: Readonly<Record<string, unknown>>,
): Readonly<Record<string, ThemeSettingDraft>> => {
  const dark = schemeOf(registry, stored) === 'dark';
  const drafts: Record<string, ThemeSettingDraft> = {};
  for (const spec of registry.settings) {
    drafts[spec.key] = draftValue(spec, stored[spec.key], dark);
  }
  return drafts;
};

/** The token a control edits, by key. */
export const themeSettingSpec = (
  registry: ThemeSettingsRegistry,
  key: string,
): ThemeSettingSpec | undefined => specOf(registry, key);

const trimOrNull = (value: ThemeSettingDraft): string | null => {
  const text = typeof value === 'string' ? value.trim() : '';
  return text === '' ? null : text;
};

const wireValue = (spec: ThemeSettingSpec, draft: ThemeSettingDraft): JsonValue => {
  switch (spec.kind) {
    case 'colour':
      return typeof draft === 'string' && isHexColour(draft) ? draft.toLowerCase() : spec.default;
    case 'font':
    case 'choice':
      return typeof draft === 'string' && spec.choices.includes(draft) ? draft : spec.default;
    case 'number': {
      const number = isStoredNumber(draft) ? draft : spec.default;
      return Math.min(spec.max, Math.max(spec.min, Math.trunc(number)));
    }
    case 'asset': {
      if (typeof draft !== 'string' || draft.trim() === '') {
        return null;
      }
      const id = Number(draft);
      return Number.isInteger(id) && id > 0 ? id : null;
    }
    case 'text':
      return trimOrNull(draft);
    case 'html':
      return trimOrNull(draft);
  }
};

/** Every token's wire value, from the drafts. A missing draft takes the spec's
 * default, so the form always sends the full map. */
export const themeSettingsWire = (
  registry: ThemeSettingsRegistry,
  drafts: Readonly<Record<string, ThemeSettingDraft>>,
): Readonly<Record<string, JsonValue>> => {
  const dark = drafts['color_scheme'] === 'dark';
  const values: Record<string, JsonValue> = {};
  for (const spec of registry.settings) {
    const draft =
      drafts[spec.key] ?? draftValue(spec, undefined, dark);
    values[spec.key] = wireValue(spec, draft);
  }
  return values;
};
