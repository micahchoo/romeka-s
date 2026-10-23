/**
 * The plan edits behind "Each row describes" and "Its values go on"
 * (`simple/list-plan.ts`), which Simple and Advanced both call. A level is
 * read back from where the column's sources are, so each edit is checked by
 * reading the plan it returns.
 */
import { describe, expect, it } from 'vitest';
import type { ColumnRef, ImportPlan } from '../src/api/import-plan';
import type { ColumnRollUp } from '../src/api/import-proposal';
import { parseImportPlan } from '../src/api/import-plan-tree-read';
import { asTarget, levelOf, targetOf, withColumnLevel, withCountedTarget, withFileRows, withRowKind, withTarget } from '../src/pages/import/simple/list-plan';
import type { Level } from '../src/pages/import/simple/list-plan';
import { loadFixture } from './loadFixture';
import { seriesOf, setsOf, slotsOf, withSeries, withSets, withSlots } from '../src/pages/import/series';

const FILE: ColumnRef = { index: 0, header: 'File' };
const KEYWORDS: ColumnRef = { index: 1, header: 'Keywords' };
const YEAR: ColumnRef = { index: 2, header: 'Year' };
const TYPE: ColumnRef = { index: 3, header: 'Item Type' };

const column = (sheet: string, ref: ColumnRef) => ({ from: 'column' as const, sheet, column: ref });

/** The proposer's pick for a column whose rows share no value in every shared item: each file. */
const onEachFile = (ref: ColumnRef): ColumnRollUp => ({
  ...ref,
  varies: { groups: 2, disjoint: 2, of: 2 },
  pick: { chosen: { value: 'file', counts: { items: 0, files: 4 } }, rival: { value: 'item', counts: { items: 2, files: 0 } }, others: [] },
});

/** One list, joined on `File` by name, one item a row: Keywords and Year feed fields, Item Type the item type. */
const plan = (): ImportPlan => {
  const draft = loadFixture('import/import-draft.json');
  const raw: unknown = typeof draft === 'object' && draft !== null ? Reflect.get(draft, 'plan') : null;
  const base = parseImportPlan(raw);
  const options = { split: ',', type: 'literal' as const, lang: null, private: false };
  const listed = withSeries(base, 'all', (series) => ({
    ...series,
    lists: [
      {
        id: 'af',
        source: 's1',
        file: 'BHC006_GMT_AF.xlsx',
        hash: null,
        read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
        rowKind: 'item',
        join: { on: 'identifier', column: FILE, normalize: { caseFold: true, leadingZeros: false, separators: true } },
        itemKey: null,
        columns: [FILE, KEYWORDS, YEAR, TYPE].map((ref) => ({ column: ref, options })),
      },
    ],
  }));
  return withSlots(listed, 'all', {
      item: {
        'dcterms:subject': { sources: [column('af', KEYWORDS)], combine: 'all', map: { 'front view': 'Front' } },
        'dcterms:date': { sources: [column('af', YEAR)], combine: 'first' },
        template: { sources: [column('af', TYPE)], combine: 'first' },
      },
      media: {},
      itemSet: {},
  });
};

const sheetOf = (edited: ImportPlan) => seriesOf(edited, 'all').lists[0];

describe('withRowKind', () => {
  it('reads back the kind it set; rows that describe files join by file name with the same column and folds', () => {
    const files = withRowKind(plan(), 'af', 'media');
    expect(sheetOf(files)?.rowKind).toBe('media');
    expect(sheetOf(files)?.join).toEqual({ on: 'fileName', column: FILE, normalize: { caseFold: true, leadingZeros: false, separators: true } });
    expect(sheetOf(withRowKind(files, 'af', 'itemSet'))?.rowKind).toBe('itemSet');
    expect(sheetOf(withRowKind(files, 'af', 'item'))?.rowKind).toBe('item');
  });

  it('brings the columns it put on each file back to the item when rows describe items again', () => {
    const onFiles = withColumnLevel(withRowKind(plan(), 'af', 'media'), 'af', KEYWORDS, 'file');
    expect(levelOf(onFiles, 'af', KEYWORDS.index)).toBe('file');
    const back = withRowKind(onFiles, 'af', 'item');
    expect(levelOf(back, 'af', KEYWORDS.index)).toBe('item');
    expect(slotsOf(back, 'all').media).toEqual({});
    expect(slotsOf(back, 'all').item['dcterms:subject']).toEqual({ sources: [column('af', KEYWORDS)], combine: 'all', map: { 'front view': 'Front' } });
  });
});

describe('withColumnLevel', () => {
  it.each<Level>(['file', 'item', 'out', 'file'])('reads back %s as the level it set', (level) => {
    const files = withRowKind(plan(), 'af', 'media');
    expect(levelOf(withColumnLevel(files, 'af', KEYWORDS, level), 'af', KEYWORDS.index)).toBe(level);
  });

  it('moves the column into the same slot on each file with its combine and value map, and back', () => {
    const onFiles = withColumnLevel(plan(), 'af', KEYWORDS, 'file');
    expect(slotsOf(onFiles, 'all').media).toEqual({ 'dcterms:subject': { sources: [column('af', KEYWORDS)], combine: 'all', map: { 'front view': 'Front' } } });
    expect(slotsOf(onFiles, 'all').item['dcterms:subject']).toBeUndefined();
    expect(slotsOf(onFiles, 'all').item['dcterms:date']).toEqual({ sources: [column('af', YEAR)], combine: 'first' });
    const back = withColumnLevel(onFiles, 'af', KEYWORDS, 'item');
    expect(slotsOf(back, 'all')).toEqual(slotsOf(plan(), 'all'));
  });

  it('leaves the other sources of a slot where they are', () => {
    const shared: ImportPlan = withSlots(plan(), 'all', { ...slotsOf(plan(), 'all'), item: { 'dcterms:date': { sources: [column('af', YEAR), { from: 'file', fact: 'name' }], combine: 'first' } } });
    const onFiles = withColumnLevel(shared, 'af', YEAR, 'file');
    expect(slotsOf(onFiles, 'all').item['dcterms:date']).toEqual({ sources: [{ from: 'file', fact: 'name' }], combine: 'first' });
    expect(slotsOf(onFiles, 'all').media['dcterms:date']).toEqual({ sources: [column('af', YEAR)], combine: 'first' });
  });

  it('refuses each file for a column feeding the item set or the item type', () => {
    const sets: ImportPlan = withSlots(plan(), 'all', { ...slotsOf(plan(), 'all'), item: { sets: { sources: [column('af', TYPE)], combine: 'first' } } });
    expect(withColumnLevel(sets, 'af', TYPE, 'file')).toEqual(sets);
    expect(withColumnLevel(plan(), 'af', TYPE, 'file')).toEqual(plan());
  });

  it('leaving a column out removes its sources, and a rule left with none', () => {
    const out = withColumnLevel(plan(), 'af', YEAR, 'out');
    expect(levelOf(out, 'af', YEAR.index)).toBe('out');
    expect(slotsOf(out, 'all').item['dcterms:date']).toBeUndefined();
  });

  it('keeps a column on each file when its field changes', () => {
    const onFiles = withColumnLevel(plan(), 'af', KEYWORDS, 'file');
    const moved = withTarget(onFiles, 'af', KEYWORDS, 'field:dcterms:description');
    expect(slotsOf(moved, 'all').media['dcterms:description']).toEqual({ sources: [column('af', KEYWORDS)], combine: 'first' });
    expect(slotsOf(moved, 'all').item['dcterms:description']).toBeUndefined();
    expect(levelOf(moved, 'af', KEYWORDS.index)).toBe('file');
  });
});

describe('withFileRows', () => {
  it('puts descriptive columns on the item, whatever their counts, and the item type on the item', () => {
    const files = withFileRows(plan(), 'af', []);
    expect(sheetOf(files)?.rowKind).toBe('media');
    expect(sheetOf(files)?.join?.on).toBe('fileName');
    expect([KEYWORDS, YEAR, TYPE].map((ref) => levelOf(files, 'af', ref.index))).toEqual(['item', 'item', 'item']);
    expect(levelOf(files, 'af', FILE.index)).toBe('out');
  });

  it('brings Keywords, already on each file, back to the item', () => {
    const onFiles = withColumnLevel(withRowKind(plan(), 'af', 'media'), 'af', KEYWORDS, 'file');
    expect(levelOf(onFiles, 'af', KEYWORDS.index)).toBe('file');
    const files = withFileRows(onFiles, 'af', []);
    expect(levelOf(files, 'af', KEYWORDS.index)).toBe('item');
    expect(slotsOf(files, 'all').item['dcterms:subject']?.sources).toEqual([column('af', KEYWORDS)]);
    expect(slotsOf(files, 'all').media).toEqual({});
  });

  it('puts a column on each file only when the proposer picks it so, whatever its field', () => {
    const named: ImportPlan = withSlots(plan(), 'all', { ...slotsOf(plan(), 'all'), item: { 'dcterms:format': { sources: [column('af', YEAR)], combine: 'first' } } });
    expect(levelOf(withFileRows(named, 'af', []), 'af', YEAR.index)).toBe('item');
    const files = withFileRows(named, 'af', [onEachFile(YEAR)]);
    expect(levelOf(files, 'af', YEAR.index)).toBe('file');
    expect(slotsOf(files, 'all').media['dcterms:format']?.sources).toEqual([column('af', YEAR)]);
  });

  it('keeps the item type on the item though the proposer picks each file for its column', () => {
    expect(levelOf(withFileRows(plan(), 'af', [onEachFile(TYPE)]), 'af', TYPE.index)).toBe('item');
  });
});

describe('withCountedTarget', () => {
  const files = (): ImportPlan => withTarget(withRowKind(plan(), 'af', 'media'), 'af', YEAR, '');
  const counts = (rollUps: readonly ColumnRollUp[]) => ({ grain: null, matches: undefined, cells: [], rollUps });

  it('puts a column’s first field where the proposer picks', () => {
    expect(levelOf(files(), 'af', YEAR.index)).toBe('out');
    const placed = withCountedTarget(files(), 'af', YEAR, 'field:dcterms:format', counts([onEachFile(YEAR)]));
    expect(levelOf(placed, 'af', YEAR.index)).toBe('file');
    expect(levelOf(withCountedTarget(files(), 'af', YEAR, 'field:dcterms:format', counts([])), 'af', YEAR.index)).toBe('item');
  });

  it('reads a pick only for the column it names, by place and header', () => {
    const moved = { ...onEachFile(YEAR), header: 'Year Of Digitization' };
    expect(levelOf(withCountedTarget(files(), 'af', YEAR, 'field:dcterms:format', counts([moved])), 'af', YEAR.index)).toBe('item');
  });
});

describe('a column’s field', () => {
  /** The plan with a free set that makes one set per Keywords value, as a v2 plan's sets column lifts to. */
  const withKeywordSets = (): ImportPlan => withSets(plan(), () => [{ each: { sources: [column('af', KEYWORDS)], combine: 'all', names: 'title' } }]);

  it('is never an item set: the free sets are the archive’s, and a column’s field leaves them as they are', () => {
    expect(asTarget('sets')).toBeNull();
    const sets = withKeywordSets();
    expect(targetOf(sets, 'af', KEYWORDS.index)).toBe('field:dcterms:subject');
    expect(targetOf(withTarget(sets, 'af', KEYWORDS, ''), 'af', KEYWORDS.index)).toBe('');
    expect(setsOf(withTarget(sets, 'af', KEYWORDS, 'field:dcterms:description'))).toEqual(setsOf(sets));
    expect(setsOf(withTarget(sets, 'af', KEYWORDS, ''))).toEqual(setsOf(sets));
  });

  it('maps the column in its list, as the server refuses a rule that reads a column the list does not map', () => {
    const bare = withSeries(plan(), 'all', (series) => ({ ...series, lists: series.lists.map((list) => ({ ...list, columns: [] })) }));
    const placed = withTarget(bare, 'af', YEAR, 'field:dcterms:date');
    expect(sheetOf(placed)?.columns).toEqual([{ column: YEAR, options: { split: null, type: 'literal', lang: null, private: false } }]);
    expect(sheetOf(withTarget(placed, 'af', YEAR, 'field:dcterms:issued'))?.columns).toHaveLength(1);
  });
});
