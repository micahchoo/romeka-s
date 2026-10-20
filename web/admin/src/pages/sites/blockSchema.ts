/**
 * The block schema format and its parser: the shape of
 * `src/generated/blocks.json`, which `crates/omeka-site/src/blocks/schema.rs`
 * generates from the layout modules and a Rust test keeps in step.
 *
 * The JSON arrives from outside the SPA, so it is `unknown` until
 * [`parseBlockRegistry`] narrows it here. A layout's name or description that is
 * not in the catalogue, or a field's label, help, example or choice name that is
 * not in the page editor's table (`editor/messages.ts`), fails the parse, which
 * keeps a generated key that nobody can translate from reaching a form.
 */
import { ParseError, isRecord, isUnknownArray, parseListOf } from '../../api/parse';
import type { Path } from '../../api/types';
import type { KeyOf } from '../../i18n';
import { sites } from '../../i18n/namespaces/sites';
import { isEditorMessageKey } from './editor/messages';
import type { EditorMessageKey } from './editor/messages';


/** A message id of the `sites` namespace, the table these keys are read from. */
type MessageKey = KeyOf<typeof sites>;

const isMessageKey = sites.has;

/** A value a field starts from: the JSON kind that carries it. */
export type FieldDefault = string | number | boolean | readonly string[];

/** One accepted value of a choice field, with its label key. */
export interface BlockFieldOption {
  /** The stored value; the empty string is a real choice. */
  readonly value: string;
  readonly label: EditorMessageKey;
}

/** A section of the block styling form, in the order the form shows them. */
export const FIELD_GROUPS = ['spacing', 'size', 'background', 'alignment', 'grid'] as const;
export type FieldGroup = (typeof FIELD_GROUPS)[number];

/**
 * When a control shows: a test on one sibling's value. The sibling's default
 * stands in for an absent value, and a hidden control keeps what it stores.
 */
export type FieldCondition =
  | { readonly field: string; readonly test: 'equals'; readonly value: string }
  | { readonly field: string; readonly test: 'set' };

/** The four stored keys of a `sides` control. */
export interface SideKeys {
  readonly top: string;
  readonly right: string;
  readonly bottom: string;
  readonly left: string;
}

interface FieldBase {
  /** The `o:data` key, or a group's own name. */
  readonly key: string;
  readonly label: EditorMessageKey;
  /** The one line under the control that says what the visitor sees. */
  readonly help: EditorMessageKey;
  /** The value a form starts from, or `null` for an empty control. */
  readonly default: FieldDefault | null;
  readonly required: boolean;
  /** The sibling test that shows the control, or `null` to always show it. */
  readonly when: FieldCondition | null;
  /** The styling form section, layout data only. */
  readonly group: FieldGroup | null;
  /** Whether the form tucks the field under "Advanced". */
  readonly advanced: boolean;
}

/** One field of a layout's form, by the control it renders. */
export type BlockField =
  | (FieldBase & {
      readonly type: 'text';
      /** An example the empty input shows. */
      readonly placeholder: EditorMessageKey | null;
    })
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
  | (FieldBase & {
      readonly type: 'itemQuery';
      /** The sibling choice naming the resource type searched; `null` for items. */
      readonly resourceKey: string | null;
    })
  | (FieldBase & { readonly type: 'attachments'; readonly media: boolean })
  | (FieldBase & { readonly type: 'repeat'; readonly fields: readonly BlockField[] })
  | (FieldBase & { readonly type: 'pageList' })
  | (FieldBase & {
      /** A hex colour under `key`, or a theme colour role under `tokenKey`. */
      readonly type: 'color';
      /** Where a theme role is stored, or `null` for a field that takes hex only. */
      readonly tokenKey: string | null;
    })
  | (FieldBase & { readonly type: 'anchor' })
  | (FieldBase & { readonly type: 'sides'; readonly keys: SideKeys })
  | (FieldBase & {
      /** A page under `key`, or a URL under `urlKey`; one is set at a time. */
      readonly type: 'link';
      readonly urlKey: string;
    });

/** A layout's editable `data` members, in form order. */
export interface LayoutSchema {
  readonly fields: readonly BlockField[];
}

/** One registered block layout, as the admin reads it. */
export interface BlockLayoutSchema extends LayoutSchema {
  readonly name: string;
  readonly label: MessageKey;
  /** The one line the insert picker shows under the label. */
  readonly description: MessageKey;
}

/** One registered Romeka block, as the admin reads it. */
export interface RomkaBlockSchema extends LayoutSchema {
  readonly name: string;
  readonly label: MessageKey;
  readonly description: MessageKey;
  /** The upstream layout the block is stored under. */
  readonly carrier: string;
}

/** The plain-hash algorithm and one worked sample the hash test checks against. */
export interface RomkaHash {
  readonly algorithm: string;
  readonly input: string;
  readonly hash: string;
  /** The raw-byte algorithm's name. */
  readonly legacyAlgorithm: string;
  /** The raw-byte hash of the sample input, as carrier version 1 stored it. */
  readonly legacyHash: string;
}

/** One theme colour role a colour field may store. */
export interface ColorToken {
  /** The stored value, the role's `--color-<value>` suffix. */
  readonly value: string;
  readonly label: EditorMessageKey;
}

/** The whole block registry. */
export interface BlockRegistry {
  readonly layouts: readonly BlockLayoutSchema[];
  readonly romeka: readonly RomkaBlockSchema[];
  /** The carrier version a new carrier writes. */
  readonly romekaVersion: number;
  readonly romekaHash: RomkaHash;
  readonly blockLayoutData: LayoutSchema;
  readonly pageLayoutData: LayoutSchema;
  /** A block group's section, under `o:layout_data.romeka.section`. */
  readonly sectionLayoutData: LayoutSchema;
  readonly colorTokens: readonly ColorToken[];
}

const parseString = (value: unknown, path: Path): string => {
  if (typeof value !== 'string') {
    throw new ParseError(path, 'string');
  }
  return value;
};

/** A field's label, help, example or choice name: a page editor message id. */
const parseFieldText = (value: unknown, path: Path): EditorMessageKey => {
  const key = parseString(value, path);
  if (!isEditorMessageKey(key)) {
    throw new ParseError(path, `a page editor message key, got ${key}`);
  }
  return key;
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

/** A required integer, e.g. the carrier version. */
const parseInteger = (value: unknown, path: Path): number => {
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

/** A catalogue key the generator omits when absent. */
const parseOptionalLabel = (value: unknown, path: Path): EditorMessageKey | null =>
  value === undefined ? null : parseFieldText(value, path);

const parseOptionalString = (value: unknown, path: Path): string | null =>
  value === undefined ? null : parseString(value, path);

const parseCondition = (value: unknown, path: Path): FieldCondition | null => {
  if (value === undefined) {
    return null;
  }
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  const field = parseString(value.field, [...path, 'field']);
  switch (parseString(value.test, [...path, 'test'])) {
    case 'equals':
      return { field, test: 'equals', value: parseString(value.value, [...path, 'value']) };
    case 'set':
      return { field, test: 'set' };
    default:
      throw new ParseError([...path, 'test'], 'equals or set');
  }
};

const isFieldGroup = (value: string): value is FieldGroup =>
  FIELD_GROUPS.some((group) => group === value);

const parseGroup = (value: unknown, path: Path): FieldGroup | null => {
  if (value === undefined) {
    return null;
  }
  const group = parseString(value, path);
  if (!isFieldGroup(group)) {
    throw new ParseError(path, `a styling group, got ${group}`);
  }
  return group;
};

const parseSideKeys = (value: unknown, path: Path): SideKeys => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    top: parseString(value.top, [...path, 'top']),
    right: parseString(value.right, [...path, 'right']),
    bottom: parseString(value.bottom, [...path, 'bottom']),
    left: parseString(value.left, [...path, 'left']),
  };
};

const parseOption = (value: unknown, path: Path): BlockFieldOption => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    value: parseString(value.value, [...path, 'value']),
    label: parseFieldText(value.label, [...path, 'label']),
  };
};

const parseField = (value: unknown, path: Path): BlockField => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  const base = {
    key: parseString(value.key, [...path, 'key']),
    label: parseFieldText(value.label, [...path, 'label']),
    help: parseFieldText(value.help, [...path, 'help']),
    default: parseDefault(value.default, [...path, 'default']),
    required: parseFlag(value.required, [...path, 'required']),
    when: parseCondition(value.when, [...path, 'when']),
    group: parseGroup(value.group, [...path, 'group']),
    advanced: parseFlag(value.advanced, [...path, 'advanced']),
  };
  switch (parseString(value.type, [...path, 'type'])) {
    case 'text':
      return {
        ...base,
        type: 'text',
        placeholder: parseOptionalLabel(value.placeholder, [...path, 'placeholder']),
      };
    case 'color':
      return {
        ...base,
        type: 'color',
        tokenKey: parseOptionalString(value.tokenKey, [...path, 'tokenKey']),
      };
    case 'anchor':
      return { ...base, type: 'anchor' };
    case 'sides':
      return { ...base, type: 'sides', keys: parseSideKeys(value.keys, [...path, 'keys']) };
    case 'link':
      return { ...base, type: 'link', urlKey: parseString(value.urlKey, [...path, 'urlKey']) };
    case 'html':
      return { ...base, type: 'html' };
    case 'boolean':
      return { ...base, type: 'boolean' };
    case 'pageRef':
      return { ...base, type: 'pageRef' };
    case 'assetRef':
      return { ...base, type: 'assetRef' };
    case 'itemQuery':
      return {
        ...base,
        type: 'itemQuery',
        resourceKey: parseOptionalString(value.resourceKey, [...path, 'resourceKey']),
      };
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
    description: parseLabel(value.description, [...path, 'description']),
    fields: parseFields(value.fields, [...path, 'fields']),
  };
};

const parseRomka = (value: unknown, path: Path): RomkaBlockSchema => {
  const layout = parseLayout(value, path);
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    ...layout,
    carrier: parseString(value.carrier, [...path, 'carrier']),
  };
};

const parseRomkaHash = (value: unknown, path: Path): RomkaHash => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    algorithm: parseString(value.algorithm, [...path, 'algorithm']),
    input: parseString(value.input, [...path, 'input']),
    hash: parseString(value.hash, [...path, 'hash']),
    legacyAlgorithm: parseString(value.legacyAlgorithm, [...path, 'legacyAlgorithm']),
    legacyHash: parseString(value.legacyHash, [...path, 'legacyHash']),
  };
};

/** Narrows the generated `blocks.json` into the typed registry. */
export const parseBlockRegistry: (raw: unknown) => BlockRegistry = (raw) => {
  if (!isRecord(raw)) {
    throw new ParseError([], 'an object');
  }
  return {
    layouts: parseListOf(parseLayout)(raw.layouts, ['layouts']),
    romeka: parseListOf(parseRomka)(raw.romeka, ['romeka']),
    romekaVersion: parseInteger(raw.romekaVersion, ['romekaVersion']),
    romekaHash: parseRomkaHash(raw.romekaHash, ['romekaHash']),
    blockLayoutData: parseSchema(raw.blockLayoutData, ['blockLayoutData']),
    pageLayoutData: parseSchema(raw.pageLayoutData, ['pageLayoutData']),
    sectionLayoutData: parseSchema(raw.sectionLayoutData, ['sectionLayoutData']),
    colorTokens: parseListOf(parseOption)(raw.colorTokens, ['colorTokens']),
  };
};
