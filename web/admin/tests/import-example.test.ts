/**
 * One marked example for both modes (`src/pages/import/example.ts`): a mark
 * Simple writes is the one Advanced lists, and Advanced removing a mark or
 * the example keeps what Simple stores in `plan.simple` under the series'
 * id (`answered`, `notLists`).
 */
import { describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import type { ImportPlan } from '../src/api/import-plan';
import { parseImportPlan } from '../src/api/import-plan-tree-read';
import { exampleOf, patternOf, removeExample, removeMark, withExample } from '../src/pages/import/example';
import type { Mark } from '../src/pages/import/example';
import { readState, withState } from '../src/pages/import/simple/example-state';
import { seriesOf, withSeries } from '../src/pages/import/series';
import { loadFixture } from './loadFixture';

const draft = (): ImportPlan => parseImportPlan(asRecord(loadFixture('import/import-draft.json'), [])['plan']);

const PAGE: Mark = { kind: 'words', words: ['front', 'back'], sep: '_' };
const OLD = { source: 's1', file: 'old.csv' };
const NUMBER: Mark = { kind: 'number', text: '001', sep: '-' };

/** The series `all` read by these marks, as Structure's "Mark an example" writes it. */
const withMarks = (plan: ImportPlan, marks: readonly Mark[], path: string): ImportPlan =>
  withSeries(withExample(plan, 'all', path, marks), 'all', (series) => ({ ...series, grouping: { ...series.grouping, pattern: patternOf(marks) } }));

const marksOf = (plan: ImportPlan): readonly Mark[] => exampleOf(plan, 'all')?.marks ?? [];

const simpleAnswered = (): ImportPlan => withState(withMarks(draft(), [PAGE, NUMBER], 'scans/a_front.tif'), { answered: ['date-form'], notLists: [OLD] });

describe('one example, read by both modes', () => {
  it('with no mark writes the pattern a new draft starts with on the server', () => {
    expect(patternOf([])).toBe(seriesOf(draft(), 'all').grouping.pattern);
  });

  it('a plan saved before glued parts reads the same pattern from its marks', () => {
    const saved = '^(?:[^/]*/)*(?<identifier>[^/]+?)(?:[_-](?<part>front|back|\\d+))?(?:\\.[^./]*)?$';
    const old: ImportPlan = withSeries(draft(), 'all', (series) => ({ ...series, grouping: { ...series.grouping, pattern: saved }, example: { path: 'scans/a_front.tif', marks: [{ kind: 'words', words: ['front', 'back'], sep: '_' }, { kind: 'number', text: '001', sep: '-' }] } }));
    const example = exampleOf(old, 'all');
    expect(example?.marks).toEqual([PAGE, NUMBER]);
    expect(patternOf(example?.marks ?? [])).toBe(saved);
    expect(marksOf(old)).toEqual([PAGE, NUMBER]);
  });

  it('Advanced reads the marks Simple wrote', () => {
    const plan = simpleAnswered();
    expect(exampleOf(plan, 'all')).toEqual({ path: 'scans/a_front.tif', marks: [PAGE, NUMBER] });
    expect(seriesOf(plan, 'all').grouping.pattern).toBe(patternOf([PAGE, NUMBER]));
  });

  it('Advanced removing a mark keeps what Simple stores beside it, and Simple reads the rest', () => {
    const plan = removeMark(simpleAnswered(), 'all', 0);
    expect(marksOf(plan)).toEqual([NUMBER]);
    expect(readState(plan)).toEqual({ answered: ['date-form'], notLists: [OLD] });
    expect(seriesOf(plan, 'all').grouping.pattern).toBe(patternOf([NUMBER]));
  });

  it('removing the last mark or the example keeps Simple’s answers', () => {
    for (const plan of [removeMark(removeMark(simpleAnswered(), 'all', 0), 'all', 0), removeExample(simpleAnswered(), 'all')]) {
      expect(exampleOf(plan, 'all')).toBeNull();
      expect(readState(plan)).toEqual({ answered: ['date-form'], notLists: [OLD] });
      expect(seriesOf(plan, 'all').grouping.pattern).toBe(patternOf([]));
    }
  });

  it('a plan with nothing else in its example has none once the marks are gone', () => {
    const plan = removeExample(withMarks(draft(), [PAGE], 'a_front.tif'), 'all');
    expect(seriesOf(plan, 'all').example).toBeNull();
  });

  it('Simple’s answers are in plan.simple under the series’ id, so a plan with no mark has no example', () => {
    const plan = withState(draft(), { answered: ['date-form'], notLists: [OLD] });
    expect(seriesOf(plan, 'all').example).toBeNull();
    expect(plan.simple).toEqual({ all: { answered: ['date-form'], notLists: [OLD] } });
    expect(readState(plan).notLists).toEqual([OLD]);
  });

  it('a draft saved with the answers inside example still reads them, and the first save moves them', () => {
    const old: ImportPlan = withSeries(draft(), 'all', (series) => ({ ...series, example: { path: 'scans/a_front.tif', marks: [{ kind: 'words', words: ['front', 'back'], sep: '_' }], groupsOk: 'p', answered: ['date-form'], notLists: ['old.csv'] } }));
    expect(marksOf(old)).toEqual([PAGE]);
    expect(readState(old)).toEqual({ answered: ['date-form'], notLists: [{ source: '', file: 'old.csv' }] });
    const saved = withState(old, { answered: ['date-form', 'identical'] });
    expect(seriesOf(saved, 'all').example).toEqual({ path: 'scans/a_front.tif', marks: [{ kind: 'words', words: ['front', 'back'], sep: '_' }] });
    expect(marksOf(saved)).toEqual([PAGE]);
    expect(readState(saved)).toEqual({ answered: ['date-form', 'identical'], notLists: [{ source: '', file: 'old.csv' }] });
    expect(readState(removeMark(old, 'all', 0)).answered).toEqual(['date-form']);
  });
});
