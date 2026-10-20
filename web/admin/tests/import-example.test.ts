/**
 * One marked example for both modes (`src/pages/import/example.ts`): a mark
 * Simple writes is the one Advanced lists, and Advanced removing a mark or
 * the example keeps what Simple stores beside it (`groupsOk`, `answered`,
 * `notLists`).
 */
import { describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import type { ImportPlan } from '../src/api/import-plan';
import { parseImportPlan } from '../src/api/import-plan-read';
import { exampleOf, patternOf, removeExample, removeMark } from '../src/pages/import/example';
import type { Mark } from '../src/pages/import/example';
import { readState, withMarks, withState } from '../src/pages/import/simple/example-state';
import { loadFixture } from './loadFixture';

const draft = (): ImportPlan => parseImportPlan(asRecord(loadFixture('import/import-draft.json'), [])['plan']);

const PAGE: Mark = { kind: 'words', words: ['front', 'back'], sep: '_' };
const NUMBER: Mark = { kind: 'number', text: '001', sep: '-' };

const simpleAnswered = (): ImportPlan => {
  const marked = withMarks(draft(), [PAGE, NUMBER], 'scans/a_front.tif');
  return withState(marked, { groupsOk: patternOf([PAGE, NUMBER]), answered: ['date-form'], notLists: ['old.csv'] });
};

describe('one example, read by both modes', () => {
  it('Advanced reads the marks Simple wrote', () => {
    const plan = simpleAnswered();
    expect(exampleOf(plan)).toEqual({ path: 'scans/a_front.tif', marks: [PAGE, NUMBER] });
    expect(plan.grouping.pattern).toBe(patternOf([PAGE, NUMBER]));
  });

  it('Advanced removing a mark keeps what Simple stores beside it, and Simple reads the rest', () => {
    const plan = removeMark(simpleAnswered(), 0);
    expect(readState(plan)).toEqual({ marks: [NUMBER], groupsOk: patternOf([PAGE, NUMBER]), answered: ['date-form'], notLists: ['old.csv'] });
    expect(plan.grouping.pattern).toBe(patternOf([NUMBER]));
  });

  it('removing the last mark or the example keeps Simple’s answers', () => {
    for (const plan of [removeMark(removeMark(simpleAnswered(), 0), 0), removeExample(simpleAnswered())]) {
      expect(exampleOf(plan)).toBeNull();
      expect(readState(plan)).toEqual({ marks: [], groupsOk: patternOf([PAGE, NUMBER]), answered: ['date-form'], notLists: ['old.csv'] });
      expect(plan.grouping.pattern).toBe(patternOf([]));
    }
  });

  it('a plan with nothing else in its example has none once the marks are gone', () => {
    const plan = removeExample(withMarks(draft(), [PAGE], 'a_front.tif'));
    expect(plan.example).toBeNull();
  });
});
