/**
 * Schema keys as stored paths. A key such as `romeka.anchor`,
 * `romeka.section.layout` or `romeka.height` names a Romeka setting of the
 * block, stored at that path under `o:layout_data.romeka.settings` by the
 * carrier module ([`romeka.ts`](./romeka.ts)). Any other key is one of
 * upstream's own members. The block-data form and the layout-data forms read
 * and write through here, so a control never knows where its value is stored.
 */
import type { JsonValue } from '../../../api/types';
import type { BlockField } from '../blockSchema';
import { readSetting, writeSettings } from './romeka';
import type { CarrierOwner } from './romeka';

type JsonRecord = Readonly<Record<string, JsonValue>>;

const ROMEKA = 'romeka.';

/** The setting path a schema key names, `null` for an upstream member. */
export const settingPath = (key: string): string | null =>
  key.startsWith(ROMEKA) ? key.slice(ROMEKA.length) : null;

/** The schema key of a Romeka setting path. */
export const romekaKey = (path: string): string => `${ROMEKA}${path}`;

/** The value a schema key names in a layout-data record, `undefined` when it
 * is not stored. */
export const readPath = (record: JsonRecord, key: string): JsonValue | undefined => {
  const path = settingPath(key);
  return path === null ? record[key] : readSetting(record, path);
};

/**
 * `record` with each of `values` written at its key: an upstream member as
 * given, a Romeka setting through the carrier writer.
 */
export const writePaths = (record: JsonRecord, values: JsonRecord, owner: CarrierOwner): JsonRecord => {
  const upstream: Record<string, JsonValue> = {};
  const settings: Record<string, JsonValue> = {};
  for (const [key, value] of Object.entries(values)) {
    const path = settingPath(key);
    if (path === null) {
      upstream[key] = value;
    } else {
      settings[path] = value;
    }
  }
  const next = { ...record, ...upstream };
  return Object.keys(settings).length === 0 ? next : writeSettings(next, settings, owner);
};

/** Every stored key a field reads or writes, its own and a colour's token key. */
const fieldPaths = (field: BlockField): readonly string[] =>
  field.type === 'color' && field.tokenKey !== null ? [field.key, field.tokenKey] : [field.key];

/**
 * The flat record a layout-data form reads: upstream's own members as stored,
 * and the value of every Romeka setting the fields name.
 */
export const formValues = (record: JsonRecord, fields: readonly BlockField[]): JsonRecord => {
  const values: Record<string, JsonValue> = { ...record };
  for (const key of fields.flatMap(fieldPaths)) {
    if (settingPath(key) !== null) {
      const value = readPath(record, key);
      if (value !== undefined) {
        values[key] = value;
      }
    }
  }
  return values;
};
