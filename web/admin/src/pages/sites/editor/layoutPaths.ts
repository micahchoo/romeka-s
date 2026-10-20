/**
 * Layout-data keys as paths. A schema key such as `romeka.anchor` or
 * `romeka.section.layout` names a Romeka setting of the block; a key without a
 * dot is one of upstream's own members.
 *
 * Every Romeka setting lives in one shape, `o:layout_data.romeka = {block, v,
 * settings}` (`crates/omeka-site/src/blocks/style.rs`): a Romeka block's
 * carrier names that block, and an upstream block names its own layout. So
 * `romeka.anchor` is stored at `romeka.settings.anchor`, and the first write
 * to a block without a carrier creates one.
 *
 * The forms read and write flat records keyed by those paths, so a control
 * never knows where its value is stored. This module is the one place a path
 * meets the nested object.
 */
import type { JsonValue } from '../../../api/types';
import type { BlockField } from '../blockSchema';
import { isJsonObject } from './values';

type JsonRecord = Readonly<Record<string, JsonValue>>;

const ROMEKA = 'romeka.';

/** Where a schema path is stored: a Romeka path sits under `romeka.settings`. */
const storedPath = (path: string): string =>
  path.startsWith(ROMEKA) ? `romeka.settings.${path.slice(ROMEKA.length)}` : path;

const readStored = (record: JsonRecord, path: string): JsonValue | undefined => {
  let current: JsonValue | undefined = record;
  for (const step of path.split('.')) {
    if (current === undefined || !isJsonObject(current)) {
      return undefined;
    }
    current = current[step];
  }
  return current;
};

/** The value a schema path names, `undefined` when it is not stored. */
export const readPath = (record: JsonRecord, path: string): JsonValue | undefined =>
  readStored(record, storedPath(path));

/** A Romeka value that says "not set": the member is removed rather than stored. */
const isUnset = (value: JsonValue): boolean => value === null || value === '' || value === false;

/** `record` without `key`. */
const without = (record: JsonRecord, key: string): JsonRecord =>
  Object.fromEntries(Object.entries(record).filter(([name]) => name !== key));

const writeNested = (
  record: JsonRecord,
  steps: readonly string[],
  value: JsonValue,
): JsonRecord => {
  const [step, ...rest] = steps;
  if (step === undefined) {
    return record;
  }
  if (rest.length === 0) {
    return isUnset(value) ? without(record, step) : { ...record, [step]: value };
  }
  const child = record[step];
  const written = writeNested(child !== undefined && isJsonObject(child) ? child : {}, rest, value);
  return Object.keys(written).length === 0 ? without(record, step) : { ...record, [step]: written };
};

/** The block a write belongs to: its layout names a carrier it creates. */
export interface PathOwner {
  readonly layout: string;
  /** The carrier version a new carrier records. */
  readonly version: number;
}

/**
 * `romeka` as the shared shape: a carrier that names a block keeps its name
 * and version; settings written to a block without one get a carrier naming
 * the block's own layout. A carrier this module made and left with no
 * settings is removed, so a cleared setting stores no trace.
 */
const settledCarrier = (record: JsonRecord, owner: PathOwner): JsonRecord => {
  const romeka = record.romeka;
  if (romeka === undefined || !isJsonObject(romeka)) {
    return record;
  }
  const named = typeof romeka.block === 'string' && romeka.block !== '';
  const settings = romeka.settings;
  const empty = settings === undefined || (isJsonObject(settings) && Object.keys(settings).length === 0);
  if (empty && (!named || romeka.block === owner.layout) && Object.keys(romeka).every(
    (key) => key === 'block' || key === 'v' || key === 'settings',
  )) {
    return without(record, 'romeka');
  }
  if (named) {
    return record;
  }
  return { ...record, romeka: { block: owner.layout, v: owner.version, ...romeka } };
};

/**
 * `record` with each of `values` written at its path. An upstream key is set
 * as given; a Romeka path that is set to nothing is removed, and an object it
 * leaves empty goes with it.
 */
export const writePaths = (
  record: JsonRecord,
  values: JsonRecord,
  owner: PathOwner,
): JsonRecord => {
  let next: JsonRecord = record;
  let romeka = false;
  for (const [path, value] of Object.entries(values)) {
    if (path.includes('.')) {
      romeka = romeka || path.startsWith(ROMEKA);
      next = writeNested(next, storedPath(path).split('.'), value);
    } else {
      next = { ...next, [path]: value };
    }
  }
  return romeka ? settledCarrier(next, owner) : next;
};

/** Every stored key a field reads or writes, its own and a colour's token key. */
const fieldPaths = (field: BlockField): readonly string[] =>
  field.type === 'color' && field.tokenKey !== null ? [field.key, field.tokenKey] : [field.key];

/**
 * The flat record a layout-data form reads: upstream's own members as stored,
 * and the value at every dotted path the fields name.
 */
export const formValues = (record: JsonRecord, fields: readonly BlockField[]): JsonRecord => {
  const values: Record<string, JsonValue> = { ...record };
  for (const path of fields.flatMap(fieldPaths)) {
    if (path.includes('.')) {
      const value = readPath(record, path);
      if (value !== undefined) {
        values[path] = value;
      }
    }
  }
  return values;
};
