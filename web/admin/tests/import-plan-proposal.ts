/**
 * The `proposal` section of a test server that stands in for no proposer:
 * the plan's own tree, each series kept as the plan holds it (an `own`
 * node, as the server sends for a series an `own` edit keeps), nothing
 * offered, and every edit applied. Both zooms of Structure draw their tree
 * from it, so the servers' one-series scenarios open on the series they
 * edit. It names no counts the scenarios do not state: `files` is every
 * file, and no item is counted.
 */
import { isRecord } from '../src/api/parse';

type Json = Record<string, unknown>;

const records = (value: unknown): Json[] => (Array.isArray(value) ? value.filter(isRecord) : []);

const seriesIn = (levels: unknown): Json[] =>
  records(levels).flatMap((level) => (level['kind'] === 'series' ? [level] : seriesIn(level['levels'])));

export const planProposal = (plan: Json, files: number): Json => {
  const structure = isRecord(plan['structure']) ? plan['structure'] : {};
  const [first] = records(plan['sources']);
  const whole = { source: first?.['id'] ?? 's1', folder: '' };
  return {
    structure,
    nodes: seriesIn(structure['levels']).map((series) => {
      const [scope] = records(series['scope']);
      return {
        kind: 'own',
        level: series['id'],
        scope: { scope: scope ?? whole, files, media: files, kinds: [] },
        items: { items: 0, files: { n: 0, of: files }, perItem: [], parts: [], namesEndingHere: 0, otherShapes: 0 },
      };
    }),
    topSets: [],
    outside: [],
    unattached: [],
    edits: records(plan['edits']).map((edit) => ({ edit, applied: true, reason: null })),
    matchesPlan: true,
  };
};
