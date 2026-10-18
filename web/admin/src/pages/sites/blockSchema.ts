/**
 * The block schema format and its parser: the shape of
 * `src/generated/blocks.json`, which `crates/omeka-site/src/blocks/schema.rs`
 * generates from the layout modules and a Rust test keeps in step.
 *
 * The JSON arrives from outside the SPA, so it is `unknown` until
 * [`parseBlockRegistry`] narrows it here. A `label` that is not in the catalogue
 * fails the parse, which keeps a generated key that nobody can translate from
 * reaching a form.
 */
import { ParseError, isRecord, isUnknownArray, parseListOf } from '../../api/parse';
import type { Path } from '../../api/types';
import { MESSAGE_KEYS } from '../../i18n';
import type { MessageKey } from '../../i18n';

const MESSAGE_KEY_SET: ReadonlySet<string> = new Set(MESSAGE_KEYS);

const isMessageKey = (value: string): value is MessageKey => MESSAGE_KEY_SET.has(value);

/** A value a field starts from: the JSON kind that carries it. */
export type FieldDefault = string | number | boolean | readonly string[];

/** One accepted value of a choice field, with its label key. */
export interface BlockFieldOption {
  /** The stored value; the empty string is a real choice. */
  readonly value: string;
  readonly label: MessageKey;
}

interface FieldBase {
  /** The `o:data` key, or a group's own name. */
  readonly key: string;
  readonly label: MessageKey;
  /** The value a form starts from, or `null` for an empty control. */
  readonly default: FieldDefault | null;
  readonly required: boolean;
}

/** One field of a layout's form, by the control it renders. */
export type BlockField =
  | (FieldBase & { readonly type: 'text' })
  | (FieldBase & { readonly type: 'html' })
  | (FieldBase & {
      readonly type: 'integer';
      /** The smallest accepted value, or `null` for no bound. */
      readonly min: number | null;
      /** The largest accepted value, or `null` for no bound. */
      readonly max: number | null;
    })
  | (FieldBase & { readonly type: 'boolean' })
  | (FieldBase & {
      readonly type: 'choice';
      readonly options: readonly BlockFieldOption[];
      /** Whether the field holds a list of the values. */
      readonly multiple: boolean;
    })
  | (FieldBase & { readonly type: 'pageRef' })
  | (FieldBase & { readonly type: 'assetRef' })
  | (FieldBase & { readonly type: 'itemQuery' })
  | (FieldBase & { readonly type: 'attachments'; readonly media: boolean })
  | (FieldBase & { readonly type: 'repeat'; readonly fields: readonly BlockField[] })
  | (FieldBase & { readonly type: 'pageList' });

/** A layout's editable `data` members, in form order. */
export interface LayoutSchema {
  readonly fields: readonly BlockField[];
}

/** One registered block layout, as the admin reads it. */
export interface BlockLayoutSchema extends LayoutSchema {
  readonly name: string;
  readonly label: MessageKey;
}

/** The whole block registry. */
export interface BlockRegistry {
  readonly layouts: readonly BlockLayoutSchema[];
  readonly blockLayoutData: LayoutSchema;
  readonly pageLayoutData: LayoutSchema;
}

const parseString = (value: unknown, path: Path): string => {
  if (typeof value !== 'string') {
    throw new ParseError(path, 'string');
  }
  return value;
};

const parseLabel = (value: unknown, path: Path): MessageKey => {
  const key = parseString(value, path);
  if (!isMessageKey(key)) {
    throw new ParseError(path, `a catalogue key, got ${key}`);
  }
  return key;
};

/** A flag the generator omits when false. */
const parseFlag = (value: unknown, path: Path): boolean => {
  if (value === undefined) {
    return false;
  }
  if (typeof value !== 'boolean') {
    throw new ParseError(path, 'boolean');
  }
  return value;
};

/** A bound the generator omits when absent. */
const parseBound = (value: unknown, path: Path): number | null => {
  if (value === undefined) {
    return null;
  }
  if (typeof value !== 'number' || !Number.isInteger(value)) {
    throw new ParseError(path, 'integer');
  }
  return value;
};

const isStringArray = (value: unknown): value is readonly string[] =>
  isUnknownArray(value) && value.every((item) => typeof item === 'string');

const parseDefault = (value: unknown, path: Path): FieldDefault | null => {
  if (value === undefined || value === null) {
    return null;
  }
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    return value;
  }
  if (isStringArray(value)) {
    return value;
  }
  throw new ParseError(path, 'a string, number, boolean or list of strings');
};

const parseOption = (value: unknown, path: Path): BlockFieldOption => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    value: parseString(value.value, [...path, 'value']),
    label: parseLabel(value.label, [...path, 'label']),
  };
};

const parseField = (value: unknown, path: Path): BlockField => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  const base = {
    key: parseString(value.key, [...path, 'key']),
    label: parseLabel(value.label, [...path, 'label']),
    default: parseDefault(value.default, [...path, 'default']),
    required: parseFlag(value.required, [...path, 'required']),
  };
  switch (parseString(value.type, [...path, 'type'])) {
    case 'text':
      return { ...base, type: 'text' };
    case 'html':
      return { ...base, type: 'html' };
    case 'boolean':
      return { ...base, type: 'boolean' };
    case 'pageRef':
      return { ...base, type: 'pageRef' };
    case 'assetRef':
      return { ...base, type: 'assetRef' };
    case 'itemQuery':
      return { ...base, type: 'itemQuery' };
    case 'pageList':
      return { ...base, type: 'pageList' };
    case 'integer':
      return {
        ...base,
        type: 'integer',
        min: parseBound(value.min, [...path, 'min']),
        max: parseBound(value.max, [...path, 'max']),
      };
    case 'choice':
      return {
        ...base,
        type: 'choice',
        options: parseListOf(parseOption)(value.options, [...path, 'options']),
        multiple: parseFlag(value.multiple, [...path, 'multiple']),
      };
    case 'attachments':
      return {
        ...base,
        type: 'attachments',
        media: parseFlag(value.media, [...path, 'media']),
      };
    case 'repeat':
      return {
        ...base,
        type: 'repeat',
        fields: parseListOf(parseField)(value.fields, [...path, 'fields']),
      };
    default:
      throw new ParseError([...path, 'type'], 'a known field type');
  }
};

const parseFields = parseListOf(parseField);

const parseSchema = (value: unknown, path: Path): LayoutSchema => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return { fields: parseFields(value.fields, [...path, 'fields']) };
};

const parseLayout = (value: unknown, path: Path): BlockLayoutSchema => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    name: parseString(value.name, [...path, 'name']),
    label: parseLabel(value.label, [...path, 'label']),
    fields: parseFields(value.fields, [...path, 'fields']),
  };
};

/** Narrows the generated `blocks.json` into the typed registry. */
export const parseBlockRegistry: (raw: unknown) => BlockRegistry = (raw) => {
  if (!isRecord(raw)) {
    throw new ParseError([], 'an object');
  }
  return {
    layouts: parseListOf(parseLayout)(raw.layouts, ['layouts']),
    blockLayoutData: parseSchema(raw.blockLayoutData, ['blockLayoutData']),
    pageLayoutData: parseSchema(raw.pageLayoutData, ['pageLayoutData']),
  };
};
