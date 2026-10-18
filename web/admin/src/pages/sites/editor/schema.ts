/**
 * The schema side of the draft: the defaults a new block starts from and the
 * field rules `validate` enforces. Both read the generated registry, so a
 * layout's form and its validation cannot disagree.
 */
import type { JsonValue } from '../../../api/types';
import type { BlockField, BlockLayoutSchema, BlockRegistry, RomkaBlockSchema } from '../blockSchema';
import type { BlockKey, ValidationIssue } from './draft';
import { isJsonArray, isJsonObject } from './values';

export const schemaFor = (
  registry: BlockRegistry,
  layout: string,
): BlockLayoutSchema | undefined =>
  registry.layouts.find((entry) => entry.name === layout);

/** The registered Romeka block named `name`, `undefined` for any other name. */
export const romkaSchemaFor = (
  registry: BlockRegistry,
  name: string,
): RomkaBlockSchema | undefined => registry.romeka.find((entry) => entry.name === name);

const defaultsFrom = (fields: readonly BlockField[]): Readonly<Record<string, JsonValue>> => {
  const data: Record<string, JsonValue> = {};
  for (const field of fields) {
    if (field.default !== null) {
      data[field.key] = field.default;
    }
  }
  return data;
};

export const defaultsFor = (
  registry: BlockRegistry,
  layout: string,
): Readonly<Record<string, JsonValue>> => {
  const schema = schemaFor(registry, layout);
  return schema === undefined ? {} : defaultsFrom(schema.fields);
};

/** The settings a new Romeka block starts from. */
export const settingsDefaults = (
  registry: BlockRegistry,
  name: string,
): Readonly<Record<string, JsonValue>> => {
  const schema = romkaSchemaFor(registry, name);
  return schema === undefined ? {} : defaultsFrom(schema.fields);
};

const integerValue = (value: JsonValue): number | null => {
  if (typeof value === 'number') {
    return Number.isInteger(value) ? value : null;
  }
  if (typeof value === 'string') {
    const trimmed = value.trim();
    return trimmed === '' ? null : Number.isInteger(Number(trimmed)) ? Number(trimmed) : null;
  }
  return null;
};

export const validateFields = (
  block: BlockKey,
  fields: readonly BlockField[],
  record: Readonly<Record<string, JsonValue>>,
  prefix: string,
): readonly ValidationIssue[] => {
  const issues: ValidationIssue[] = [];
  for (const field of fields) {
    const name = prefix === '' ? field.key : `${prefix}.${field.key}`;
    const value = record[field.key];
    if (value === undefined || value === null) {
      if (field.required) {
        issues.push({ kind: 'required', blockKey: block, field: name });
      }
      continue;
    }
    if (field.type === 'repeat') {
      const entries = isJsonArray(value)
        ? value
        : isJsonObject(value)
          ? Object.values(value)
          : [];
      entries.forEach((entry, index) => {
        if (isJsonObject(entry)) {
          issues.push(...validateFields(block, field.fields, entry, `${name}.${String(index)}`));
        }
      });
      continue;
    }
    if (field.required && value === '') {
      issues.push({ kind: 'required', blockKey: block, field: name });
      continue;
    }
    if (field.type === 'integer') {
      const number = integerValue(value);
      if (number === null) {
        issues.push({ kind: 'type', blockKey: block, field: name });
        continue;
      }
      if (field.min !== null && number < field.min) {
        issues.push({ kind: 'min', blockKey: block, field: name, min: field.min });
      }
      if (field.max !== null && number > field.max) {
        issues.push({ kind: 'max', blockKey: block, field: name, max: field.max });
      }
    }
  }
  return issues;
};
