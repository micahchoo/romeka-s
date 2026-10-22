/**
 * The `proposal` section and the plan's `edits` (`src/api/import-proposal.ts`,
 * `src/api/import-plan-read.ts`) against every shape the server's types
 * define: a proposal with each kind of node, cut, set source, pair rule and
 * edit parses and serialises back to its JSON, and a member the server never
 * writes is refused. The recorded server answer is parsed in
 * `import-api-preview.test.ts`.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { parsePreview } from '../src/api/import-sections';
import { parseProposal } from '../src/api/import-proposal';
import { parseImportPlan } from '../src/api/import-plan-tree-read';
import { ParseError, isUnknownArray } from '../src/api/parse';
import { loadFixture } from './loadFixture';

const HERE = dirname(fileURLToPath(import.meta.url));

const roundTrip = (value: unknown): unknown => {
  const back: unknown = JSON.parse(JSON.stringify(value));
  return back;
};

const scope = { source: 's1', folder: 'Artifacts' };
const list = { source: 's1', file: 'BHC006_GMT_AF.xlsx' };
const column = { index: 1, header: 'Identifier' };
const share = { n: 692, of: 696 };
const fromPlace = { from: 'namePlace', scope, place: 4 };
const setCounts = { sets: [['a', 279], ['b', 273]], rows: null };
const candidate = {
  source: 's1',
  file: 'BHC006_GMT_AF.xlsx',
  inPlan: null,
  read: 'ok',
  rows: 690,
  columns: 8,
  best: { index: 1, header: 'Identifier', on: 'fileName', paired: 680, pairedNormalized: 680, empty: 0 },
  grain: { groupsWithManyRows: 272, rowsNamingOtherFilesOfAGroup: 367, columns: [] },
  cells: [],
  overlaps: [],
};

const EDITS = [
  { edit: 'leaveOut', scope: { source: 's1', folder: 'Extras' } },
  { edit: 'keep', scope },
  { edit: 'items', scope, cut: { cut: 'whole' } },
  { edit: 'items', scope, cut: { cut: 'places', places: 5 } },
  { edit: 'items', scope, cut: { cut: 'folder', depth: 1 } },
  { edit: 'list', scope, list },
  { edit: 'list', scope, list: null },
  { edit: 'grain', list, describes: 'media' },
  { edit: 'addSet', from: fromPlace },
  { edit: 'dropSet', from: { from: 'folder', scope } },
  { edit: 'addSet', from: { from: 'column', list, column } },
  { edit: 'addSet', from: { from: 'folderLevel', scope, depth: 1 } },
  { edit: 'pair', list, rule: { rule: 'folds', normalize: { caseFold: true, leadingZeros: false, separators: true } } },
  { edit: 'pair', list, rule: { rule: 'column', column, on: 'identifier' } },
  { edit: 'pair', list, rule: { rule: 'lastPlaces', places: 2 } },
  { edit: 'title', from: fromPlace, title: 'Artifacts' },
  { edit: 'own', scope },
];

const object = (raw: unknown): Record<string, unknown> => {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    throw new Error('an object');
  }
  return { ...raw };
};

const recorded = (): Record<string, unknown> => object(loadFixture('import/preview-proposal.json'));

/** The recorded answer with a proposal that holds one of everything. */
const everything = (): Record<string, unknown> => {
  const answer = recorded();
  const proposal = object(answer['proposal']);
  const scopeCounts = { scope, files: 1309, media: 696, kinds: [['image', 696]] };
  const series = {
    kind: 'series',
    level: 'artifacts',
    scope: scopeCounts,
    items: {
      chosen: {
        value: { cut: 'places', places: 5, rule: 'recurringParts' },
        counts: {
          items: 318,
          files: share,
          perItem: [{ files: '1', count: 38 }],
          parts: [['a', 279]],
          namesEndingHere: 38,
          otherShapes: 4,
        },
      },
      rival: {
        value: { cut: 'whole' },
        counts: { items: 696, files: share, perItem: [], parts: [], namesEndingHere: 696, otherShapes: 0 },
      },
      others: [
        {
          value: { cut: 'places', places: 6, rule: 'recurringParts' },
          counts: { items: 649, files: share, perItem: [], parts: [], namesEndingHere: 0, otherShapes: 0 },
        },
      ],
    },
    list: {
      chosen: { value: list, counts: candidate },
      rival: { value: null, counts: null },
      others: [{ value: { source: 's1', file: 'Artifacts/BHC006_GMT_AF.csv' }, counts: candidate }],
    },
    grain: {
      chosen: { value: 'media', counts: { ...candidate.grain, rowsNamingNoFile: 10, items: 318 } },
      rival: { value: 'item', counts: { ...candidate.grain, rowsNamingNoFile: 10, items: 328 } },
      others: [],
    },
    set: {
      chosen: { value: { from: 'folder', scope }, counts: setCounts },
      rival: null,
      others: [{ value: { from: 'column', list, column }, counts: { sets: [['torn', 1]], rows: { n: 1, of: 690 } } }],
    },
    splits: [{ value: fromPlace, counts: { sets: [['20170616', 11]], rows: { n: 15, of: 15 } } }],
    pairing: [{ value: { rule: 'lastPlaces', places: 2 }, counts: { rows: { n: 15, of: 15 }, more: 15 } }],
    kept: [
      { list: 'list-1', file: { source: 'staging', file: 'lists/1.csv' }, rows: { n: 1, of: 2 } },
      { list: 'list-2', file: { source: 'staging', file: 'lists/2.csv' }, rows: null },
    ],
  };
  const pages = { ...series, level: 'scans', items: { ...series.items, chosen: { ...series.items.chosen, value: { cut: 'places', places: 4, rule: 'recurringInItem' } } } };
  const rows = {
    ...series,
    level: 'docs',
    items: {
      chosen: { value: { cut: 'rows', list, key: column }, counts: series.items.chosen.counts },
      rival: { value: { cut: 'folder', depth: 1 }, counts: series.items.chosen.counts },
      others: [],
    },
  };
  const own = {
    kind: 'own',
    level: 'af',
    scope: scopeCounts,
    items: { items: 318, files: share, perItem: [{ files: '1', count: 43 }], parts: [], namesEndingHere: 0, otherShapes: 0 },
    kept: [{ list: 'list-3', file: { source: 'staging', file: 'lists/3.csv' }, rows: { n: 4, of: 4 } }],
  };
  return {
    ...answer,
    proposal: {
      ...proposal,
      nodes: [{ kind: 'set', level: 'top', from: { from: 'column', list, column }, counts: setCounts }, series, pages, rows, own],
      topSets: [{ value: { from: 'column', list, column }, counts: { sets: [['Citizen Curators', 920]], rows: { n: 920, of: 920 } } }],
      outside: [
        { ...scopeCounts, scope: { source: 's1', folder: 'tiny-iiif' }, kinds: [], media: 0, reason: 'noMedia' },
        { ...scopeCounts, scope: { source: 's1', folder: 'Extras' }, reason: 'edit' },
      ],
      unattached: [candidate],
      edits: EDITS.map((edit, at) => ({ edit, applied: at % 2 === 0, reason: at % 2 === 0 ? null : (['gone', 'notOffered', 'replaced'] as const)[at % 3] })),
    },
  };
};

describe('the proposal section', () => {
  it('parses the proposal the server recorded', () => {
    const proposal = parseProposal(parsePreview(['proposal'])(recorded()).proposal);
    const series = proposal.structure.levels.filter((level) => level.kind === 'series');
    expect(proposal.nodes.map((node) => node.kind)).toEqual(series.map(() => 'series'));
    expect(proposal.edits).toEqual([]);
    expect(proposal.matchesPlan).toBe(false);
  });

  it('parses one of every node, cut, set source, pair rule and edit, and writes it back unchanged', () => {
    const raw = everything();
    const parsed = parseProposal(parsePreview(['proposal'])(raw).proposal);
    expect(roundTrip(parsed)).toEqual(raw['proposal']);
    expect(parsed.edits).toHaveLength(EDITS.length);
  });

  it('refuses a member the server never writes, at any depth', () => {
    const raw = everything();
    const proposal = object(raw['proposal']);
    const changes: ((value: Record<string, unknown>) => Record<string, unknown>)[] = [
      (value) => ({ ...value, ranked: true }),
      (value) => ({ ...value, edits: [{ edit: { edit: 'rename', scope }, applied: true, reason: null }] }),
      (value) => ({ ...value, edits: [{ edit: { edit: 'keep', scope }, applied: false, reason: 'lost' }] }),
      (value) => ({ ...value, topSets: [{ value: { from: 'everything' }, counts: setCounts }] }),
      (value) => ({ ...value, outside: [{ scope, files: 1, media: 0, kinds: [['image']], reason: 'noMedia' }] }),
      (value) => ({ ...value, nodes: [{ kind: 'set', level: 'top', from: { from: 'column', list, column }, counts: setCounts, others: [] }] }),
      (value) => ({ ...value, nodes: [{ ...object(isUnknownArray(value['nodes']) ? value['nodes'][4] : null), splits: [] }] }),
      (value) => ({
        ...value,
        nodes: [{ ...object(isUnknownArray(value['nodes']) ? value['nodes'][1] : null), kept: [{ list: 'list-1', file: list, rows: null, read: false }] }],
      }),
      (value) => ({
        ...value,
        nodes: [{ ...object(isUnknownArray(value['nodes']) ? value['nodes'][1] : null), grain: { chosen: { value: 'item', counts: candidate.grain }, rival: null, others: [] } }],
      }),
    ];
    for (const change of changes) {
      expect(() => parseProposal(parsePreview(['proposal'])({ ...raw, proposal: change(proposal) }).proposal)).toThrow(ParseError);
    }
  });
});

describe('the preview', () => {
  /**
   * The answer of a plan of two series (`preview/tests.rs`,
   * `a_plan_of_several_series_answers_each_series_pattern`): each series'
   * pattern, and the series of each record and of each list in a series.
   */
  const severalSeries = (): Record<string, unknown> => {
    const records = object(object(loadFixture('import/preview-records.json'))['records']);
    const lists = object(loadFixture('import/preview-list-candidates.json'));
    const [record] = isUnknownArray(records['items']) ? records['items'] : [];
    const [inPlan, unused] = isUnknownArray(lists['listCandidates']) ? lists['listCandidates'] : [];
    return {
      ...lists,
      patterns: [
        { series: 'letters', pattern: { groups: ['identifier', 'part'] } },
        { series: 'photos', pattern: 'missing' },
      ],
      records: { ...records, items: [{ ...object(record), series: 'letters' }] },
      listCandidates: [{ ...object(inPlan), series: 'letters' }, ...(unused === undefined ? [] : [unused])],
    };
  };

  it('parses the preview of a plan of several series', () => {
    const raw = severalSeries();
    const preview = parsePreview(['records', 'listCandidates'])(raw);
    expect(preview.patterns).toEqual([
      { series: 'letters', pattern: { groups: ['identifier', 'part'] } },
      { series: 'photos', pattern: 'missing' },
    ]);
    expect(preview.records.items.map((record) => record.series)).toEqual(['letters']);
    expect(preview.listCandidates[0]?.series).toBe('letters');
    expect(roundTrip(preview)).toEqual(raw);
  });

  it('answers no series member for a plan of one series', () => {
    const preview = parsePreview(['records', 'listCandidates'])({
      ...object(loadFixture('import/preview-list-candidates.json')),
      records: object(loadFixture('import/preview-records.json'))['records'],
    });
    expect('patterns' in preview).toBe(false);
    expect(preview.records.items.some((record) => 'series' in record)).toBe(false);
    expect(preview.listCandidates.some((candidate) => 'series' in candidate)).toBe(false);
  });

  it('refuses a series member that is not a level id', () => {
    const raw = severalSeries();
    expect(() => parsePreview(['records', 'listCandidates'])({ ...raw, patterns: [{ series: 3, pattern: 'missing' }] })).toThrow(ParseError);
    expect(() => parsePreview(['records', 'listCandidates'])({ ...raw, patterns: [{ series: 'a', pattern: 'missing', extra: 1 }] })).toThrow(ParseError);
  });

  it('refuses a proposal that is not an object, and a member it was not asked for', () => {
    const raw = recorded();
    expect(() => parsePreview(['proposal'])({ ...raw, proposal: null })).toThrow(ParseError);
    expect(() => parsePreview(['proposal'])({ ...raw, ranked: true })).toThrow(ParseError);
  });
});

describe('the plan edits', () => {
  /** The server's plan of an archive as found (`plan/structure_tests.rs`). */
  const v3 = (): Record<string, unknown> =>
    object(JSON.parse(readFileSync(resolve(HERE, '../../../crates/omeka-api/src/import/fixtures/v3/archive_as_found.json'), 'utf8')));

  it('are absent from a plan without them, and kept in order and unchanged in one with them', () => {
    const plain = v3();
    expect('edits' in parseImportPlan(plain)).toBe(false);
    const withEdits = { ...plain, edits: EDITS };
    expect(roundTrip(parseImportPlan(withEdits))).toEqual(withEdits);
  });

  it('refuse an edit the server refuses', () => {
    for (const edit of [
      { edit: 'leaveOut', scope, extra: 1 },
      { edit: 'list', scope },
      { edit: 'items', scope, cut: { cut: 'places' } },
      { edit: 'pair', list, rule: { rule: 'lastPlaces', places: -1 } },
    ]) {
      expect(() => parseImportPlan({ ...v3(), edits: [edit] })).toThrow(ParseError);
    }
  });
});
