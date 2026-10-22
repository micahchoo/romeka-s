/**
 * The series a screen of one series reads and edits. The address keeps the
 * person's pick as `series` in its last segment
 * (`#/import/<id>/<mode>/<screen>/{"series":"<level id>"}`, `lists/address.ts`),
 * so a reload opens on it. With no pick, or a pick the plan no longer has,
 * it is the plan's first series; on a plan of one series, that one.
 */
import type { ImportId, LevelId, Series } from '../../api/import-plan';
import { useHash } from '../../app/router';
import { useDraft } from './draft-context';
import { filterFromHash, listHref } from './lists/address';
import type { Place } from './screens';
import { allSeries } from './series';

const KEYS = ['series'] as const;

/** `null` only for a plan with no series, which the server refuses to save. */
export const usePickedSeries = (): Series | null => {
  const { plan } = useDraft();
  const picked = filterFromHash(useHash(), KEYS).series;
  const all = allSeries(plan);
  return all.find((series) => series.id === picked) ?? all[0] ?? null;
};

/** The address of `place` with the series `level` picked. */
export const pickedHref = (id: ImportId, place: Place, level: LevelId): string => listHref(id, place, { series: level });
