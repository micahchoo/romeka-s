/**
 * The six list files of BHC006 as `listCandidates` counts them: the Artifacts
 * and Photo Narratives workbooks at the top, an older CSV and ODS copy of the
 * Artifacts list in `Artifacts/`, and two lists that pair no file. The
 * Artifacts rows describe files (301 rows name another view of an item);
 * Photo Narratives pairs 86 of 215 rows exactly and 189 with every name fold
 * on. `IN_USE` is a plan that reads the two workbooks, as "Your list" shows it
 * after both were taken.
 */
import type { ColumnCells, Grain } from '../src/api/import-sections';
import { scenario } from './import-simple-scenarios';
import type { Scenario, ScenarioList } from './import-simple-scenarios';

export const AF = 'BHC006_GMT_AF.xlsx';
export const PN = 'BHC006_GMT_PN.xlsx';
export const CSV = 'Artifacts/BHC006_GMT_AF.csv';
export const ODS = 'Artifacts/BHC006_GMT_AF.ods';
export const D = 'BHC006_GMT_D.xlsx';
export const RAO = 'BHC006_CC_RAO_AF.xlsx';

export const COLUMNS = ['File', 'Collection', 'Curator', 'Item Type', 'Year Of Digitization', 'Digitized by', 'Keywords'];
export const KEYWORDS = 6;

const AF_GRAIN: Grain = {
  groupsWithManyRows: 251,
  rowsNamingOtherFilesOfAGroup: 301,
  columns: COLUMNS.map((header, index) => ({ index, header, varies: { groups: index === 0 ? 251 : index === KEYWORDS ? 42 : index === 5 ? 1 : 0, of: 251 } })),
};

const cells = (keywords: number, digitized: number): readonly ColumnCells[] =>
  COLUMNS.map((header, index) => ({
    index,
    header,
    separator: index === KEYWORDS ? { text: ',', cells: keywords } : index === 5 ? { text: ',', cells: digitized } : null,
  }));

const workbook = (file: string, first: string): Pick<ScenarioList, 'columns' | 'lines'> => ({
  columns: COLUMNS,
  lines: [COLUMNS.join(','), `${file.replace('.xlsx', '')}_${first}`],
});

export const LISTS: readonly ScenarioList[] = [
  {
    file: AF,
    rows: 690,
    paired: 680,
    normalized: 680,
    overlaps: [{ file: CSV, sharedTargets: 292 }, { file: ODS, sharedTargets: 292 }],
    grain: AF_GRAIN,
    cells: cells(610, 21),
    ...workbook(AF, '010a.jpg,Citizen Curators,M. L. A. Khan,Artifact,2017,Shreyas Srivatsa,"Coral, Natural History, Fossil"'),
  },
  { file: CSV, rows: 607, paired: 603, normalized: 604, overlaps: [{ file: AF, sharedTargets: 292 }] },
  { file: ODS, rows: 607, paired: 603, normalized: 604, overlaps: [{ file: AF, sharedTargets: 292 }] },
  {
    file: PN,
    rows: 215,
    paired: 86,
    normalized: 189,
    cells: cells(213, 215),
    ...workbook(PN, '001.JPG,Citizen Curators,M. L. A. Khan,Photo Narrative,2018,"S. Nandgouli, D. Patil","Public Event, Bidar, Sepia"'),
  },
  { file: D, rows: 15, paired: 0, normalized: 0 },
  { file: RAO, rows: 29, paired: 0, normalized: 0 },
];

export const bhc006 = (lists: readonly ScenarioList[] = LISTS): Scenario => ({
  ...scenario('bhc', 9),
  lists,
  unpairedRows: 92,
  unpairedItems: 210,
  flags: [...scenario('bhc', 9).flags, { kind: 'listed-not-media', slot: null, count: 82, detail: 'BHC006_GMT_AF_296a.NEF' }],
});

const ref = (index: number) => ({ index, header: COLUMNS[index] ?? '' });

const sheetPlan = (id: string, file: string, rowKind: 'item' | 'media') => ({
  id,
  source: 's1',
  file,
  hash: null,
  read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
  rowKind,
  join: { on: 'fileName', column: ref(0), normalize: { caseFold: false, leadingZeros: false, separators: false } },
  itemKey: null,
  columns: COLUMNS.map((_, index) => ({ column: ref(index), options: { split: index === KEYWORDS ? ',' : null, type: 'literal', lang: null, private: false } })),
});

export const fromColumn = (sheet: string, index: number) => ({ from: 'column', sheet, column: ref(index) });

/** The two workbooks in use: Artifacts rows describe files, Photo Narratives rows items. */
export const IN_USE: Record<string, unknown> = {
  sheets: [sheetPlan('folder-list-1', AF, 'media'), sheetPlan('folder-list-2', PN, 'item')],
  slots: {
    item: {
      sets: { sources: [fromColumn('folder-list-1', 1), fromColumn('folder-list-2', 1)], combine: 'first' },
      template: { sources: [fromColumn('folder-list-1', 3), fromColumn('folder-list-2', 3)], combine: 'first' },
      'dcterms:date': { sources: [fromColumn('folder-list-1', 4), fromColumn('folder-list-2', 4)], combine: 'first' },
      'dcterms:subject': { sources: [fromColumn('folder-list-1', KEYWORDS), fromColumn('folder-list-2', KEYWORDS)], combine: 'all' },
    },
    media: {},
    itemSet: {},
  },
};
