/**
 * The series seam (`src/pages/import/series.ts`) over the server's own v2
 * plan (`fixtures/full.json`): a v2 plan reads as the one series the server
 * lifts it to, and an edit made through that series writes the v2 plan an
 * edit of its members would.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { parseImportPlan } from '../src/api/import-plan-read';
import type { ImportPlan } from '../src/api/import-plan';
import { seriesOf, withSeries } from '../src/pages/import/series';

const HERE = dirname(fileURLToPath(import.meta.url));

const fullPlan = (): ImportPlan =>
  parseImportPlan(JSON.parse(readFileSync(resolve(HERE, '../../../crates/omeka-api/src/import/fixtures/full.json'), 'utf8')));

describe('the one series of a v2 plan', () => {
  it('is the series the server lifts the plan to: all, over every file, with no set', () => {
    const plan = fullPlan();
    expect(seriesOf(plan)).toEqual({
      kind: 'series',
      id: 'all',
      set: null,
      scope: 'all',
      grouping: plan.grouping,
      example: plan.example,
      lists: plan.sheets,
      slots: plan.slots,
    });
    expect(seriesOf(plan, 'all')).toEqual(seriesOf(plan));
    expect(() => seriesOf(plan, 'letters')).toThrow('letters');
  });

  it('an edit through the one series writes the same v2 plan', () => {
    const plan = fullPlan();
    const [sheet] = plan.sheets;
    if (sheet === undefined) {
      throw new Error('full.json has a sheet');
    }
    const copy = { ...sheet, id: 'copy' };
    const grouping = { pattern: '^(?<identifier>[^/]+)$', caseFold: false };
    const slots = { ...plan.slots, media: {} };
    const edited = withSeries(plan, (series) => ({ ...series, grouping, example: null, lists: [...series.lists, copy], slots }));
    const direct: ImportPlan = { ...plan, grouping, example: null, sheets: [...plan.sheets, copy], slots };
    expect(edited).toEqual(direct);
    expect(JSON.stringify(edited)).toBe(JSON.stringify(direct));
    expect(parseImportPlan(JSON.parse(JSON.stringify(edited)))).toEqual(direct);
  });

  it('a change that keeps the series leaves the plan itself', () => {
    const plan = fullPlan();
    expect(withSeries(plan, (series) => series)).toBe(plan);
  });
});
