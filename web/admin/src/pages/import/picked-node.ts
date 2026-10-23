/**
 * The node of the Structure tree the address names, in both zooms: `node`,
 * a tree key, in its last segment
 * (`#/import/<id>/<mode>/structure/{"node":"series:Artifacts"}`,
 * `lists/address.ts`). Picking a node writes it there, so a reload and the
 * mode switch open on the same node. An older address names a series by its
 * level id as `series`; it reads as that series' node.
 */
import type { ImportId } from '../../api/import-plan';
import { useHash } from '../../app/router';
import { filterFromHash, listHref } from './lists/address';
import type { Place } from './screens';

const KEYS = ['node', 'series'] as const;

/** The tree key of a series' node. */
export const seriesKey = (level: string): string => `series:${level}`;

/** The node the address names, if it names one. */
export const usePickedNode = (): string | undefined => {
  const { node, series } = filterFromHash(useHash(), KEYS);
  return node ?? (series === undefined ? undefined : seriesKey(series));
};

/** The address of `place` with the node `key` picked. */
export const nodeHref = (id: ImportId, place: Place, key: string): string => listHref(id, place, { node: key });

/**
 * Writes the node picked into the address in place of the last one, so the
 * browser's back button skips each move through the tree; the hash readers
 * hear it as a change.
 */
export const pickNode = (id: ImportId, place: Place, key: string): void => {
  window.history.replaceState(window.history.state, '', nodeHref(id, place, key));
  window.dispatchEvent(new Event('hashchange'));
};
