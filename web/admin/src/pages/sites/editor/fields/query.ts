/**
 * The browse query a block stores as one string (`browsePreview`'s `query`).
 *
 * Upstream serialises the whole advanced-search form, so the stored value is an
 * ordered list of pairs and the port keeps that order and every pair it does not
 * edit. The two names a builder controls are `fulltext_search`,
 * `item_set_id`, `resource_class_id`, `sort_by` and `sort_order`.
 *
 * An empty known pair is meaningful. `BlockLayout/BrowsePreview.php::render`
 * adds its default sort only when the stored query has no `sort_by` pair at all,
 * so the builder keeps a cleared select as `sort_by=` instead of dropping it.
 */

/** One `key=value` pair of the query string, in stored order. */
export interface QueryPair {
  readonly key: string;
  readonly value: string;
}

/** The advanced-search keys the builder edits, and where each lives in the string. */
export const QUERY_KEYS = {
  fulltext: 'fulltext_search',
  itemSet: 'item_set_id',
  resourceClass: 'resource_class_id',
  sortBy: 'sort_by',
  sortOrder: 'sort_order',
} as const;

/** A PHP `urldecode`: `+` is a space and `%XX` is a byte; a bad escape stays literal. */
const decode = (text: string): string => {
  const bytes: number[] = [];
  let index = 0;
  while (index < text.length) {
    const char = text[index];
    if (char === undefined) {
      break;
    }
    if (char === '+') {
      bytes.push(0x20);
      index += 1;
      continue;
    }
    if (char === '%' && index + 2 < text.length) {
      const hex = text.slice(index + 1, index + 3);
      const byte = /^[0-9a-fA-F]{2}$/.test(hex) ? Number.parseInt(hex, 16) : null;
      if (byte !== null) {
        bytes.push(byte);
        index += 3;
        continue;
      }
    }
    for (const point of char) {
      bytes.push(...new TextEncoder().encode(point));
    }
    index += 1;
  }
  return new TextDecoder().decode(Uint8Array.from(bytes));
};

const encode = (text: string): string => encodeURIComponent(text);

/** The stored query string as ordered pairs, PHP's `parse_str` without array keys. */
export const parseQuery = (text: string): readonly QueryPair[] =>
  text
    .split('&')
    .filter((part) => part !== '')
    .flatMap((part) => {
      const at = part.indexOf('=');
      const key = decode(at < 0 ? part : part.slice(0, at));
      return key === '' ? [] : [{ key, value: at < 0 ? '' : decode(part.slice(at + 1)) }];
    });

export const formatQuery = (pairs: readonly QueryPair[]): string =>
  pairs.map((pair) => `${encode(pair.key)}=${encode(pair.value)}`).join('&');

/** The value of `key`. `parse_str` keeps the last pair of a repeated name. */
export const queryValue = (pairs: readonly QueryPair[], key: string): string => {
  for (let index = pairs.length - 1; index >= 0; index -= 1) {
    const pair = pairs[index];
    if (pair !== undefined && pair.key === key) {
      return pair.value;
    }
  }
  return '';
};

/**
 * The pairs with `key` set to `value`. The last pair of that name keeps its
 * place, earlier duplicates are dropped so one name stays one value, and a name
 * the query does not carry is appended. A cleared value stays present.
 */
export const withQueryValue = (
  pairs: readonly QueryPair[],
  key: string,
  value: string,
): readonly QueryPair[] => {
  const last = pairs.reduce(
    (found, pair, index) => (pair.key === key ? index : found),
    -1,
  );
  if (last < 0) {
    return [...pairs, { key, value }];
  }
  return pairs
    .map((pair, index) =>
      pair.key === key ? (index === last ? { key, value } : null) : pair,
    )
    .filter((pair): pair is QueryPair => pair !== null);
};

/** The integer a stored id names, or `null` when it is empty or not a number. */
export const queryId = (pairs: readonly QueryPair[], key: string): number | null => {
  const raw = queryValue(pairs, key).trim();
  if (raw === '') {
    return null;
  }
  const value = Number(raw);
  return Number.isInteger(value) && value > 0 ? value : null;
};
