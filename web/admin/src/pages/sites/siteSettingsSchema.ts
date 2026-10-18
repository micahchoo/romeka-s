/**
 * The site settings schema format and its parser: the shape of
 * `src/generated/siteSettings.json`, which
 * `crates/omeka-site/src/settings.rs` generates from `SITE_SETTINGS` and a Rust
 * test keeps in step.
 *
 * The JSON arrives from outside the SPA, so it is `unknown` until
 * [`parseSiteSettingsRegistry`] narrows it here. A `labelKey` that is not in the
 * catalogue fails the parse, which keeps a generated key that nobody can
 * translate from reaching a form.
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

interface SiteSettingBase {
  /** The `site_setting.id` row this control edits. */
  readonly key: string;
  /** The catalogue key the form labels the control with. */
  readonly labelKey: MessageKey;
}

/** One site setting as the admin reads it, by the control it renders. */
export type SiteSettingSpec =
  | (SiteSettingBase & { readonly kind: 'text'; readonly default: string | null })
  | (SiteSettingBase & { readonly kind: 'number'; readonly default: number | null })
  | (SiteSettingBase & { readonly kind: 'boolean'; readonly default: boolean })
  | (SiteSettingBase & {
      readonly kind: 'choice';
      readonly choices: readonly string[];
      readonly default: string | null;
    })
  | (SiteSettingBase & {
      readonly kind: 'multiChoice';
      readonly choices: readonly string[];
      readonly default: readonly string[];
    })
  | (SiteSettingBase & {
      readonly kind: 'sort';
      readonly sortBy: string;
      readonly sortOrder: string;
    });

/** The whole generated registry. */
export interface SiteSettingsRegistry {
  readonly settings: readonly SiteSettingSpec[];
}

/** A setting's value while the form is open. */
export type SiteSettingDraft =
  | string
  | number
  | boolean
  | readonly string[]
  | { readonly sortBy: string; readonly sortOrder: string };

const parseSortedDefault = (
  value: unknown,
  path: Path,
): { readonly sortBy: string; readonly sortOrder: string } => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    sortBy: parseString(value.sortBy, [...path, 'sortBy']),
    sortOrder: parseString(value.sortOrder, [...path, 'sortOrder']),
  };
};

const parseSpec = (value: unknown, path: Path): SiteSettingSpec => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  const base = {
    key: parseString(value.key, [...path, 'key']),
    labelKey: parseMessageKey(value.labelKey, [...path, 'labelKey']),
  };
  switch (parseString(value.kind, [...path, 'kind'])) {
    case 'text':
      return {
        ...base,
        kind: 'text',
        default: value.default === null ? null : parseString(value.default, [...path, 'default']),
      };
    case 'number':
      return {
        ...base,
        kind: 'number',
        default:
          value.default === null ? null : parseNumber(value.default, [...path, 'default']),
      };
    case 'boolean':
      if (typeof value.default !== 'boolean') {
        throw new ParseError([...path, 'default'], 'boolean');
      }
      return { ...base, kind: 'boolean', default: value.default };
    case 'choice':
      return {
        ...base,
        kind: 'choice',
        choices: parseStringList(value.choices, [...path, 'choices']),
        default: value.default === null ? null : parseString(value.default, [...path, 'default']),
      };
    case 'multiChoice':
      return {
        ...base,
        kind: 'multiChoice',
        choices: parseStringList(value.choices, [...path, 'choices']),
        default: parseStringList(value.default, [...path, 'default']),
      };
    case 'sort': {
      const sorted = parseSortedDefault(
        { sortBy: value.sortBy, sortOrder: value.sortOrder },
        path,
      );
      return { ...base, kind: 'sort', ...sorted };
    }
    default:
      throw new ParseError([...path, 'kind'], 'a known setting kind');
  }
};

/** Narrows the generated `siteSettings.json` into the typed registry. */
export const parseSiteSettingsRegistry: (raw: unknown) => SiteSettingsRegistry = (raw) => {
  if (!isRecord(raw)) {
    throw new ParseError([], 'an object');
  }
  return { settings: parseListOf(parseSpec)(raw.settings, ['settings']) };
};

const isStringList = (value: unknown): value is readonly string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');

/** The draft a spec starts from: the stored value when it has the right shape. */
const draftValue = (spec: SiteSettingSpec, stored: unknown): SiteSettingDraft => {
  switch (spec.kind) {
    case 'text':
      return typeof stored === 'string' ? stored : (spec.default ?? '');
    case 'number':
      if (typeof stored === 'number') {
        return stored;
      }
      if (typeof stored === 'string' && stored.trim() !== '') {
        return Number(stored);
      }
      return spec.default ?? '';
    case 'boolean':
      return typeof stored === 'boolean' ? stored : spec.default;
    case 'choice':
      return typeof stored === 'string' ? stored : (spec.default ?? '');
    case 'multiChoice':
      return isStringList(stored) ? stored : spec.default;
    case 'sort':
      return isRecord(stored)
        ? {
            sortBy: typeof stored.sort_by === 'string' ? stored.sort_by : spec.sortBy,
            sortOrder: typeof stored.sort_order === 'string' ? stored.sort_order : spec.sortOrder,
          }
        : { sortBy: spec.sortBy, sortOrder: spec.sortOrder };
  }
};

/** The wire value a draft stands for; `null` removes the row. */
const wireValue = (spec: SiteSettingSpec, draft: SiteSettingDraft): JsonValue => {
  switch (spec.kind) {
    case 'text': {
      const text = typeof draft === 'string' ? draft.trim() : '';
      return text === '' ? null : text;
    }
    case 'number':
      return typeof draft === 'number' && Number.isFinite(draft) ? Math.trunc(draft) : null;
    case 'boolean':
      return draft === true;
    case 'choice':
      return typeof draft === 'string' && draft !== '' ? draft : null;
    case 'multiChoice':
      return isStringList(draft) ? draft : [];
    case 'sort':
      return isRecord(draft)
        ? {
            sort_by: typeof draft.sortBy === 'string' ? draft.sortBy : spec.sortBy,
            sort_order: typeof draft.sortOrder === 'string' ? draft.sortOrder : spec.sortOrder,
          }
        : { sort_by: spec.sortBy, sort_order: spec.sortOrder };
  }
};

/** Every setting's draft, from the stored map. */
export const siteSettingDrafts = (
  registry: SiteSettingsRegistry,
  stored: Readonly<Record<string, unknown>>,
): Readonly<Record<string, SiteSettingDraft>> => {
  const drafts: Record<string, SiteSettingDraft> = {};
  for (const spec of registry.settings) {
    drafts[spec.key] = draftValue(spec, stored[spec.key]);
  }
  return drafts;
};

/** Every setting's wire value, from the drafts. A missing draft takes the
 * spec's default, so the form always sends the full set. */
export const siteSettingsWire = (
  registry: SiteSettingsRegistry,
  drafts: Readonly<Record<string, SiteSettingDraft>>,
): Readonly<Record<string, JsonValue>> => {
  const values: Record<string, JsonValue> = {};
  for (const spec of registry.settings) {
    values[spec.key] = wireValue(spec, drafts[spec.key] ?? draftValue(spec, undefined));
  }
  return values;
};
