/**
 * The write rule of Advanced (P5 design, section 1). A Structure click saves
 * the proposal's structure, and the proposer recomputes some members of a
 * suggested folder. So a write to such a member owns the folder: the same
 * save adds one `own` edit, and the proposer then keeps the folder as
 * written. A write to a member the proposer carries adds no edit. The plans
 * are BHC006's: the proposal's six folders (`proposal-bhc006.json`), and the
 * archive as found (`import-draft.json`).
 */
import { describe, expect, it } from 'vitest';
import type { Edit, ImportPlan } from '../src/api/import-plan';
import { asRecord } from '../src/api/parse';
import { parseImportPlan } from '../src/api/import-plan-tree-read';
import { setJoin, setPattern, setRead } from '../src/pages/import/advanced/plan-edits';
import { allSeries, seriesOf } from '../src/pages/import/series';
import { loadFixture } from './loadFixture';

const DRAFT = asRecord(asRecord(loadFixture('import/import-draft.json'), [])['plan'], []);
const PROPOSED = asRecord(asRecord(loadFixture('import/proposal-bhc006.json'), [])['structure'], []);

const asFound = (): ImportPlan => parseImportPlan(DRAFT);
const suggested = (edits?: readonly Edit[]): ImportPlan => parseImportPlan({ ...DRAFT, structure: PROPOSED, ...(edits === undefined ? {} : { edits }) });

const TYPED = String.raw`^(?<identifier>BHC006_GMT_AF_\d+)(?<part>[a-z]*)\.\w+$`;
const OWN_ARTIFACTS: Edit = { edit: 'own', scope: { source: 's1', folder: 'Artifacts' } };
const LEAVE_EXTRAS: Edit = { edit: 'leaveOut', scope: { source: 's1', folder: 'Extras' } };

const othersOf = (plan: ImportPlan) => allSeries(plan).filter((series) => series.id !== 'Artifacts');

describe('an Advanced write', () => {
  it('a typed pattern on a suggested folder writes the folder and one own edit', () => {
    const plan = suggested([LEAVE_EXTRAS]);
    const next = setPattern(plan, 'Artifacts', TYPED, true);
    expect(seriesOf(next, 'Artifacts').grouping).toEqual({ pattern: TYPED, caseFold: true });
    expect(next.edits).toEqual([LEAVE_EXTRAS, OWN_ARTIFACTS]);
    expect(othersOf(next)).toEqual(othersOf(plan));
  });

  it('a second write on a folder written by hand adds no second own edit', () => {
    const once = setPattern(suggested(), 'Artifacts', TYPED, false);
    const twice = setPattern(once, 'Artifacts', `${TYPED}x`, false);
    expect(twice.edits).toEqual([OWN_ARTIFACTS]);
  });

  it('a write keyed by a list owns the folder that holds the list', () => {
    const next = setJoin(suggested(), 'BHC006_GMT_AF.xlsx', { normalize: { caseFold: true, leadingZeros: false, separators: false } });
    expect(next.edits).toEqual([OWN_ARTIFACTS]);
  });

  it('a write the proposer carries adds no edit', () => {
    const next = setRead(suggested(), 'BHC006_GMT_AF.xlsx', { trim: false });
    expect(seriesOf(next, 'Artifacts').lists[0]?.read.trim).toBe(false);
    expect(next.edits).toBeUndefined();
  });

  it('on the archive as found, a typed pattern adds no edit: no proposal holds that folder yet', () => {
    const plan = asFound();
    const [only] = allSeries(plan);
    const next = setPattern(plan, only?.id ?? '', TYPED, false);
    expect(allSeries(next)[0]?.grouping.pattern).toBe(TYPED);
    expect(next.edits).toBeUndefined();
  });
});
