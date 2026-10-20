/**
 * A repeat group's slots and their Romeka settings. A repeat is a layout's
 * whole `o:data`: upstream's asset block stores its slots as a list, or as an
 * object keyed by slot index (`asset-block-form.phtml` posts
 * `o:data[<index>][id]`), never under a key of its own.
 *
 * A slot member keyed `romeka.<name>` (an asset slot's URL) belongs to the
 * same slot but is stored at
 * `o:layout_data.romeka.settings.<repeat key>[<slot>].<name>`, so the
 * upstream list never carries it and an upstream save keeps it. The form edits
 * the two as one merged list, so a reorder or a removal moves both together.
 */
import type { JsonValue } from '../../../api/types';
import { romekaKey, settingPath } from './layoutPaths';
import type { BlockData } from './draft';
import { isJsonArray, isJsonObject } from './values';

type JsonRecord = Readonly<Record<string, JsonValue>>;

const records = (value: JsonValue | undefined): readonly JsonRecord[] =>
  value !== undefined && isJsonArray(value)
    ? value.map((entry) => (isJsonObject(entry) ? entry : {}))
    : [];

/**
 * The slots a block's `o:data` holds: the list, or the index-keyed object in
 * index order. A draft this editor once saved under the repeat's own key
 * (`{attachments: [...]}`) is read too, so the next save stores it the
 * upstream way.
 */
export const storedSlots = (data: BlockData, key: string): readonly JsonValue[] => {
  if (isJsonArray(data)) {
    return data;
  }
  const nested = data[key];
  if (nested !== undefined && isJsonArray(nested)) {
    return nested;
  }
  return Object.keys(data)
    .filter((index) => /^\d+$/u.test(index))
    .sort((left, right) => Number(left) - Number(right))
    .flatMap((index) => {
      const slot = data[index];
      return slot === undefined ? [] : [slot];
    });
};

/** The slots of `data` with each one's Romeka settings merged in under its
 * `romeka.` keys. */
export const mergeSlots = (
  data: JsonValue | undefined,
  settings: JsonValue | undefined,
): readonly JsonRecord[] => {
  const extras = records(settings);
  return records(data).map((record, index) => {
    const merged: Record<string, JsonValue> = { ...record };
    for (const [name, value] of Object.entries(extras[index] ?? {})) {
      merged[romekaKey(name)] = value;
    }
    return merged;
  });
};

/** A value that leaves a slot setting unset. */
const isUnset = (value: JsonValue): boolean => value === null || value === '';

/**
 * The merged slots split back: the upstream records, and the settings list,
 * one object per slot, or `null` when no slot holds a setting, so a cleared
 * list leaves no trace in the carrier.
 */
export const splitSlots = (
  merged: readonly JsonValue[],
): { readonly data: readonly JsonRecord[]; readonly settings: readonly JsonRecord[] | null } => {
  const data: JsonRecord[] = [];
  const settings: JsonRecord[] = [];
  for (const record of records([...merged])) {
    const upstream: Record<string, JsonValue> = {};
    const extra: Record<string, JsonValue> = {};
    for (const [key, value] of Object.entries(record)) {
      const path = settingPath(key);
      if (path === null) {
        upstream[key] = value;
      } else if (!isUnset(value)) {
        extra[path] = value;
      }
    }
    data.push(upstream);
    settings.push(extra);
  }
  return {
    data,
    settings: settings.some((slot) => Object.keys(slot).length > 0) ? settings : null,
  };
};
