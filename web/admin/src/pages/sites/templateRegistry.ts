/**
 * The generated built-in page templates, narrowed once at module load.
 *
 * `src/generated/templates.json` is written by
 * `crates/omeka-site/src/blocks/templates.rs` from the registry beside the block
 * layouts, and a Rust test keeps it byte-identical. It arrives from outside the
 * SPA, so the parser here narrows it and rejects a `label` that is not in the
 * catalogue. The shapes live in `templates.ts`; this module owns the parse, so
 * the editor shell does not carry it.
 */
import { ParseError, isRecord, isUnknownArray, parseListOf } from '../../api/parse';
import type { JsonValue, Path } from '../../api/types';
import raw from '../../generated/templates.json';
import type { KeyOf } from '../../i18n';
import { editor } from '../../i18n/namespaces/editor';
import type { BuiltinSection, BuiltinTemplate, TemplateBlock, TemplateLayout } from './templates';


/** A message id of the `editor` namespace, the table these keys are read from. */
type MessageKey = KeyOf<typeof editor>;

const isMessageKey = editor.has;

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

/** A JSON value from outside the process, narrowed recursively. */
const parseJson = (value: unknown, path: Path): JsonValue => {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return value;
  }
  if (isUnknownArray(value)) {
    return value.map((entry, index) => parseJson(entry, [...path, index]));
  }
  if (isRecord(value)) {
    const record: Record<string, JsonValue> = {};
    for (const key of Object.keys(value)) {
      record[key] = parseJson(value[key], [...path, key]);
    }
    return record;
  }
  throw new ParseError(path, 'a JSON value');
};

/** A block's `o:data` or a page's `o:layout_data`, which is an object or a list. */
const parseData = (value: unknown, path: Path): JsonValue => {
  const parsed = parseJson(value, path);
  if (parsed === null || typeof parsed !== 'object') {
    throw new ParseError(path, 'an object or a list');
  }
  return parsed;
};

const parseLayout = (value: unknown, path: Path): TemplateLayout => {
  const text = parseString(value, path);
  if (text !== 'normal' && text !== 'grid') {
    throw new ParseError(path, `a page layout, got ${text}`);
  }
  return text;
};

const parseTemplateBlock = (value: unknown, path: Path): TemplateBlock => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  const layoutData = value.layoutData;
  return {
    layout: parseString(value.layout, [...path, 'layout']),
    data: parseData(value.data, [...path, 'data']),
    layoutData:
      layoutData === undefined || layoutData === null
        ? null
        : parseJson(layoutData, [...path, 'layoutData']),
  };
};

const parseBuiltinTemplate = (value: unknown, path: Path): BuiltinTemplate => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    id: parseString(value.id, [...path, 'id']),
    label: parseMessageKey(value.label, [...path, 'label']),
    description: parseMessageKey(value.description, [...path, 'description']),
    layout: parseLayout(value.layout, [...path, 'layout']),
    layoutData: parseData(value.layoutData, [...path, 'layoutData']),
    blocks: parseListOf(parseTemplateBlock)(value.blocks, [...path, 'blocks']),
  };
};

const parseBuiltinSection = (value: unknown, path: Path): BuiltinSection => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'an object');
  }
  return {
    id: parseString(value.id, [...path, 'id']),
    label: parseMessageKey(value.label, [...path, 'label']),
    description: parseMessageKey(value.description, [...path, 'description']),
    blocks: parseListOf(parseTemplateBlock)(value.blocks, [...path, 'blocks']),
  };
};

/** The built-in page templates and section templates `templates.json` carries. */
export interface TemplateRegistry {
  readonly templates: readonly BuiltinTemplate[];
  readonly sections: readonly BuiltinSection[];
}

/** Narrows the generated `templates.json` into the built-in templates and sections. */
export const parseTemplateRegistry = (raw: unknown): TemplateRegistry => {
  if (!isRecord(raw)) {
    throw new ParseError([], 'an object');
  }
  return {
    templates: parseListOf(parseBuiltinTemplate)(raw.templates, ['templates']),
    sections: parseListOf(parseBuiltinSection)(raw.sections, ['sections']),
  };
};

const REGISTRY = parseTemplateRegistry(raw);

/** The built-in templates, parsed at this boundary. */
export const BUILTIN_TEMPLATES: readonly BuiltinTemplate[] = REGISTRY.templates;

/** The built-in section templates the block picker offers. */
export const BUILTIN_SECTIONS: readonly BuiltinSection[] = REGISTRY.sections;
