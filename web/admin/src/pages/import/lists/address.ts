/**
 * A list screen opened with a filter: `#/import/<id>/<mode>/<screen>/<filter>`,
 * the filter one URI-encoded JSON segment of plain strings, so the chart and
 * the groups can link to "these items" and the address can be kept.
 */
import type { FilterValue } from '../filters';
import type { ImportId } from '../../../api/import-plan';
import { isRecord } from '../../../api/parse';
import { placeHref } from '../screens';
import type { Place } from '../screens';

export const listHref = <K extends string>(id: ImportId, place: Place, filter: FilterValue<K>): string =>
  `${placeHref(id, place)}/${encodeURIComponent(JSON.stringify(filter))}`;

/** The filter the address names for the keys `keys`; anything else in it is dropped. */
export const filterFromHash = <K extends string>(hash: string, keys: readonly K[]): FilterValue<K> => {
  const segments = hash.split('/');
  const last = segments.length > 5 ? segments[5] : undefined;
  if (last === undefined || last === '') {
    return {};
  }
  let raw: unknown;
  try {
    raw = JSON.parse(decodeURIComponent(last));
  } catch {
    return {};
  }
  const value: { [P in K]?: string } = {};
  if (isRecord(raw)) {
    for (const key of keys) {
      const text = raw[key];
      if (typeof text === 'string' && text !== '') {
        value[key] = text;
      }
    }
  }
  return value;
};
