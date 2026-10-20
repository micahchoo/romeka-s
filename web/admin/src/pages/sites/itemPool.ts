/**
 * The site's item pool (`o:item_pool`) as the query string upstream edits it
 * as. `SiteAdmin/IndexController::resourcesAction` shows
 * `http_build_query($site->itemPool())` in the form and stores
 * `parse_str($formData['item_pool'])`, so the stored value is the nested
 * array `parse_str` builds: an empty pool is `[]`, every leaf is a string.
 */
import type { ItemSetId, JsonValue } from '../../api/types';
import { formatQuery, parseQuery } from './editor/fields/query';
import type { QueryPair } from './editor/fields/query';

const scalarText = (value: JsonValue): string | null => {
  if (value === null) {
    return null;
  }
  if (typeof value === 'boolean') {
    return value ? '1' : '0';
  }
  return typeof value === 'number' ? String(value) : typeof value === 'string' ? value : null;
};

const entriesOf = (value: JsonValue): readonly (readonly [string, JsonValue])[] | null => {
  if (Array.isArray(value)) {
    return value.map((entry, index) => [String(index), entry] as const);
  }
  if (typeof value === 'object' && value !== null) {
    return Object.entries(value);
  }
  return null;
};

const flatten = (prefix: string, value: JsonValue): readonly QueryPair[] => {
  const entries = entriesOf(value);
  if (entries === null) {
    const text = scalarText(value);
    return text === null ? [] : [{ key: prefix, value: text }];
  }
  return entries.flatMap(([key, entry]) => flatten(`${prefix}[${key}]`, entry));
};

/** `http_build_query` over the stored pool. */
export const itemPoolQuery = (pool: JsonValue | null): string => {
  const entries = pool === null ? null : entriesOf(pool);
  if (entries === null) {
    return '';
  }
  return formatQuery(entries.flatMap(([key, entry]) => flatten(key, entry)));
};

/** A `parse_str` node while it is built: a leaf, or keys in insertion order. */
type Node = string | Map<string, Node>;

/** `name[a][b]` as `['name', 'a', 'b']`; text after an unclosed `[` stays in the name. */
const keyPath = (key: string): readonly string[] => {
  const open = key.indexOf('[');
  if (open <= 0) {
    return [key];
  }
  const path = [key.slice(0, open)];
  let rest = key.slice(open);
  for (let match = /^\[([^\]]*)\]/.exec(rest); match !== null; match = /^\[([^\]]*)\]/.exec(rest)) {
    path.push(match[1] ?? '');
    rest = rest.slice(match[0].length);
  }
  return path;
};

/** The next index `[]` appends at: one past the largest integer key, as PHP does. */
const nextIndex = (node: Map<string, Node>): string => {
  const indexes = [...node.keys()].filter((key) => /^(0|[1-9]\d*)$/.test(key)).map(Number);
  return String(indexes.length === 0 ? 0 : Math.max(...indexes) + 1);
};

const assign = (root: Map<string, Node>, path: readonly string[], value: string): void => {
  let node = root;
  path.forEach((segment, depth) => {
    const key = segment === '' && depth > 0 ? nextIndex(node) : segment;
    if (depth === path.length - 1) {
      node.set(key, value);
      return;
    }
    const child = node.get(key);
    const next = child instanceof Map ? child : new Map<string, Node>();
    node.set(key, next);
    node = next;
  });
};

/** `json_encode` of a PHP array: a list when its keys are 0..n-1 in order. */
const toJson = (node: Node): JsonValue => {
  if (typeof node === 'string') {
    return node;
  }
  const keys = [...node.keys()];
  const values = [...node.values()].map(toJson);
  if (keys.every((key, index) => key === String(index))) {
    return values;
  }
  return Object.fromEntries(keys.map((key, index) => [key, values[index] ?? null]));
};

/** `parse_str` over the query the form holds. */
export const itemPoolFromQuery = (query: string): JsonValue => {
  const root = new Map<string, Node>();
  for (const pair of parseQuery(query)) {
    assign(root, keyPath(pair.key), pair.value);
  }
  return toJson(root);
};

/** The pool the Items page writes from the site's item sets: `item_set_id[]=<id>` for each. */
export const itemSetPool = (sets: readonly ItemSetId[]): JsonValue =>
  itemPoolFromQuery(formatQuery(sets.map((set) => ({ key: 'item_set_id[]', value: String(set) }))));

/**
 * What a site's pool lets join it when an item is created: every new item (the
 * empty pool, as upstream), the items made in its item sets, or the items a
 * saved search selects.
 */
export type PoolKind = 'every' | 'sets' | 'search';

export const poolKind = (pool: JsonValue | null): PoolKind => {
  const entries = pool === null ? [] : (entriesOf(pool) ?? []);
  if (entries.length === 0) {
    return 'every';
  }
  return entries.every(([key]) => key === 'item_set_id') ? 'sets' : 'search';
};
