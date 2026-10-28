/**
 * The dashboard's work counts (`GET /api/romeka/dashboard`, `crates/omeka-api/src/read/place_counts.rs`).
 * The server reads each count through the Items list's own matching, as the caller, so each equals
 * the total of the Items list it opens.
 */
import { api } from './client';
import { asCount, member, strict } from './import-read';
import { asString } from './parse';
import type { Parser, Path } from './types';

export interface DashboardCounts {
  readonly noItemSet: number;
  readonly noItemType: number;
  readonly changedByOthers: {
    /** The time the count reaches back to, which the list it opens filters by. */
    readonly since: string;
    readonly items: number;
  };
}

const parseChanged = (raw: unknown, path: Path): DashboardCounts['changedByOthers'] => {
  const object = strict(raw, path, ['since', 'items']);
  return { since: member(object, 'since', path, asString), items: member(object, 'items', path, asCount) };
};

export const parseDashboardCounts: Parser<DashboardCounts> = (raw, path = []) => {
  const object = strict(raw, path, ['no_item_set', 'no_resource_template', 'changed_by_others']);
  return {
    noItemSet: member(object, 'no_item_set', path, asCount),
    noItemType: member(object, 'no_resource_template', path, asCount),
    changedByOthers: member(object, 'changed_by_others', path, parseChanged),
  };
};

export const dashboardApi = {
  async counts(): Promise<DashboardCounts> {
    return parseDashboardCounts(await api.requestJson('/api/romeka/dashboard'));
  },
};
