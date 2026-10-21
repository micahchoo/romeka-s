/**
 * The two folders of the round-4 prototype's drive (`proto4/drive4.mjs`,
 * `SCALE.md`), Meera's family papers and the BHC_006 archive, at 9 and at
 * 40,000 items, as the facts the server would answer for them: the groups
 * before and after each mark, the list and its column counts, the item type
 * values and the problems Check asks about. The groups, columns, values and
 * questions are the ones the prototype's first pass showed at each size
 * (its `seen` decisions), so the pass the test drives meets the same
 * questions; the item counts are made up and only have to add up.
 */
import type { Bucket, ColumnCells, Grain, MediaKind } from '../src/api/import-sections';

export interface ScenarioGroup {
  readonly files: Bucket;
  readonly folder: string;
  readonly media: MediaKind;
  readonly parts?: readonly string[];
  readonly pages?: readonly string[];
  readonly row: boolean;
  readonly count: number;
  /** The marks made before the group shows, and the last count of marks it shows under. */
  readonly from: number;
  readonly until?: number;
}

export interface ScenarioMark {
  /** Typed into "Find a file name". */
  readonly find: string;
  readonly pick: string;
  readonly parts: readonly string[];
  readonly ticks: readonly string[];
}

export interface Scenario {
  readonly name: 'meera' | 'bhc';
  readonly size: number;
  readonly files: number;
  readonly examples: readonly string[];
  /** For each example path, the words at each place of its name, with counts. */
  readonly words: Readonly<Record<string, Readonly<Record<number, readonly (readonly [string, number])[]>>>>;
  readonly marks: readonly ScenarioMark[];
  readonly groups: readonly ScenarioGroup[];
  readonly sheet: {
    readonly file: string;
    readonly headerLine: number;
    readonly lines: readonly string[];
    readonly rows: number;
    readonly columns: readonly { readonly header: string; readonly groupKeys: number; readonly fileNames: number; readonly empty: number }[];
  };
  /** The drive's answers on "Your list": the column and the option it chooses. */
  readonly mapping: readonly (readonly [string, string])[];
  readonly typeFrom: 'list' | 'top';
  readonly typeValues: readonly (readonly [string, number])[];
  readonly flags: readonly ScenarioFlag[];
  readonly identical: number;
  readonly unpairedRows: number;
  readonly unpairedFiles: number;
  /** The items the unpaired files are in; one a file when absent. */
  readonly unpairedItems?: number;
  /** Items to add with no title while no rule gives the item code or the file name as one; none when absent. */
  readonly untitled?: number;
  /** Files whose names the pattern does not fit; none when absent. */
  readonly unmatchedFiles?: number;
  /** A group the drive leaves out on "What we found", by words its facts sentence holds. */
  readonly leave: readonly string[] | null;
  /** The prototype's first pass at this size (`SCALE.md`). */
  readonly proto: { readonly decisions: number; readonly clicks: number };
  /** Every list file of the folder, in place of `sheet` alone, each read with its own counts. */
  readonly lists?: readonly ScenarioList[];
}

/** A flag kind the server counts; its sample's subject is a record unless it names a list row. */
export interface ScenarioFlag {
  readonly kind: string;
  readonly slot: string | null;
  readonly count: number;
  readonly detail: string | null;
  readonly row?: string;
  /** The Omeka items each flagged record matched; the records a `flag` filter returns carry them and the flag. */
  readonly matches?: readonly number[];
}

/**
 * A list file of the folder as `listCandidates` counts it: its `File` column
 * names `paired` files exactly and `normalized` with every name fold on.
 * One with an `error` did not read.
 */
export interface ScenarioList {
  readonly file: string;
  readonly rows: number;
  readonly paired: number;
  readonly normalized: number;
  readonly overlaps?: readonly { readonly file: string; readonly sharedTargets: number }[];
  readonly error?: string;
  /** Its header line; `File` and `Title` when absent. */
  readonly columns?: readonly string[];
  /** What `listCandidates` says its rows describe; none when absent. */
  readonly grain?: Grain;
  /** Which separator `listCandidates` says each column's cells hold; none when absent. */
  readonly cells?: readonly ColumnCells[];
  /** The sheet view's lines from line 1; when absent, the header and one row of `A`s, comma-separated. */
  readonly lines?: readonly string[];
  /** The line of `lines` that `columns` are on, which the server guesses; 1 when absent. */
  readonly headerLine?: number;
}

const big = (size: number, small: number, large: number): number => (size > 9 ? large : small);

const meera = (size: number): Scenario => {
  const n = (small: number, large: number): number => big(size, small, large);
  const folder = (name: string): string => (size > 9 ? name : '');
  const groups: ScenarioGroup[] = [
    { files: '1', folder: folder('letters'), media: 'image', row: true, count: n(2, 9000), from: 0 },
    { files: '1', folder: folder('letters'), media: 'image', row: false, count: n(1, 400), from: 0 },
    { files: '1', folder: '', media: 'pdf', row: true, count: n(1, 0), from: 0 },
    { files: '2', folder: folder('letters'), media: 'image', parts: ['back', 'front'], row: true, count: n(2, 12000), from: 1 },
    { files: '2', folder: folder('पत्रे'), media: 'image', parts: ['पुढे', 'मागे'], row: true, count: n(2, 6000), from: 1 },
  ];
  if (size > 9) {
    groups.push(
      { files: '1', folder: 'diary', media: 'image', row: true, count: 200, from: 0, until: 1 },
      { files: '1', folder: 'photos', media: 'image', row: false, count: 3000, from: 0 },
      { files: '1', folder: 'diary', media: 'image', row: false, count: 20, from: 0, until: 1 },
      { files: '1', folder: 'photos', media: 'image', row: true, count: 5000, from: 0 },
      { files: '1', folder: 'पत्रे', media: 'image', row: true, count: 2500, from: 0 },
      { files: '1', folder: 'पत्रे', media: 'image', row: false, count: 80, from: 0 },
      { files: '1', folder: 'certificates', media: 'image', row: true, count: 600, from: 0 },
      { files: '1', folder: 'newspapers', media: 'pdf', row: true, count: 1100, from: 0 },
      { files: '1', folder: 'phone', media: 'image', row: false, count: 99, from: 0 },
      { files: '1', folder: 'letters', media: 'image', parts: ['front'], row: false, count: 7, from: 1 },
      { files: '2', folder: 'letters', media: 'image', parts: ['back', 'front'], row: false, count: 40, from: 1 },
      { files: '1', folder: 'letters', media: 'image', parts: ['front'], row: true, count: 30, from: 1 },
      { files: '7+', folder: 'diary', media: 'image', pages: ['page 001', 'page 002', 'page 003'], row: true, count: 1, from: 2 },
    );
  }
  const front: (readonly [string, number])[] = [['front', n(3, 21000)], ['back', n(3, 21000)], ['पुढे', n(2, 6000)], ['मागे', n(2, 6000)], ['001', n(0, 200)]];
  const values: (readonly [string, number])[] = [['letter', n(3, 9000)], ['magazine', n(1, 0)], ['certificate', n(1, 600)], ['photo', n(2, 5000)]];
  const more: (readonly [string, number])[] = [['newspaper', 1100], ['पत्र', 2500], ['letters', 3000], ['photograph', 1900], ['photos', 1200], ['Kesari', 400], ['news paper', 80], ['diary', 1]];
  return {
    name: 'meera',
    size,
    files: n(14, 66_000),
    examples: ['letters/letter 12 front.jpg', 'letters/letter 12 back.jpg', 'पत्रे/पत्र 3 पुढे.jpg', 'diary/diary page 001.jpg', 'diary/diary page 002.jpg'].map((path) => (size > 9 ? path : path.slice(path.indexOf('/') + 1))),
    words: {
      [size > 9 ? 'letters/letter 12 front.jpg' : 'letter 12 front.jpg']: {
        2: front.filter(([, count]) => count > 0),
      },
      [size > 9 ? 'diary/diary page 001.jpg' : 'diary page 001.jpg']: { 1: [['page', 200]], 2: [['001', 1], ['002', 1]] },
    },
    marks: [
      { find: 'front', pick: 'letter 12 front.jpg', parts: ['front'], ticks: ['back', 'पुढे', 'मागे'] },
      ...(size > 9 ? [{ find: 'page 001', pick: 'diary page 001.jpg', parts: ['page'], ticks: [] }] : []),
    ],
    groups: groups.filter((group) => group.count > 0),
    sheet: {
      file: 'register.csv',
      headerLine: 1,
      lines: ['File,No.,Title,Year,Type,Donor', 'letter 12 front.jpg,1,A letter from Pune,1890,Letter,Joshi'],
      rows: n(6, 40_600),
      columns: [
        { header: 'File', groupKeys: 0, fileNames: n(5, 40_590), empty: 0 },
        { header: 'No.', groupKeys: 0, fileNames: 0, empty: 0 },
        { header: 'Title', groupKeys: 0, fileNames: 0, empty: n(0, 15) },
        { header: 'Year', groupKeys: 0, fileNames: 0, empty: 0 },
        { header: 'Type', groupKeys: 0, fileNames: 0, empty: 0 },
        { header: 'Donor', groupKeys: 0, fileNames: 0, empty: n(2, 9000) },
      ],
    },
    mapping: [
      ['Year', 'Date'],
      ['Type', 'The item type'],
    ],
    typeFrom: 'list',
    typeValues: size > 9 ? [...values.filter(([, count]) => count > 0), ...more] : values,
    flags:
      size > 9
        ? [
            { kind: 'cells-count', slot: null, count: 3, detail: 'letter 99 front.jpg,"unclosed' },
            { kind: 'no-file', slot: null, count: 7, detail: null },
            { kind: 'date-form', slot: 'dcterms:date', count: 120, detail: 'c. 1890' },
            { kind: 'unknown-type', slot: 'template', count: 9100, detail: 'Kesari' },
          ]
        : [
            { kind: 'no-file', slot: null, count: 1, detail: null },
            { kind: 'unknown-type', slot: 'template', count: 7, detail: 'letter' },
          ],
    identical: n(2, 40),
    unpairedRows: n(1, 7),
    unpairedFiles: n(2, 4000),
    ...(size > 9 ? { untitled: 15 } : {}),
    leave: null,
    proto: size > 9 ? { decisions: 47, clicks: 24 } : { decisions: 25, clicks: 18 },
  };
};

const bhc = (size: number): Scenario => {
  const n = (small: number, large: number): number => big(size, small, large);
  const groups: ScenarioGroup[] = [
    { files: '1', folder: 'Artifacts', media: 'image', row: false, count: n(1, 300), from: 0 },
    { files: '1', folder: 'Photographs', media: 'image', row: true, count: n(1, 6000), from: 0 },
    { files: '1', folder: '_scratch', media: 'image', row: false, count: n(1, 12), from: 0 },
    { files: '1', folder: 'Photographs', media: 'image', row: false, count: n(1, 200), from: 0 },
    { files: '2', folder: 'Artifacts', media: 'image', parts: ['back', 'front'], row: true, count: n(3, 14000), from: 1 },
    { files: '1', folder: 'Artifacts', media: 'image', parts: ['front'], row: true, count: n(1, 900), from: 1 },
    { files: '2', folder: 'Photographs', media: 'image', parts: ['verso'], row: true, count: n(1, 5000), from: 1 },
  ];
  if (size > 9) {
    groups.push(
      { files: '1', folder: 'Documents', media: 'image', row: false, count: 50, from: 0 },
      { files: '7+', folder: 'Documents', media: 'image', parts: ['p001', 'p002'], row: true, count: 300, from: 1, until: 1 },
      { files: '4-6', folder: 'Documents', media: 'image', parts: ['p001', 'p002', 'p003'], row: true, count: 200, from: 1, until: 1 },
      { files: '1', folder: 'Artifacts', media: 'image', parts: ['back'], row: true, count: 40, from: 1 },
      { files: '3', folder: 'Documents', media: 'image', pages: ['p001', 'p002', 'p003'], row: true, count: 4000, from: 2 },
      { files: '2', folder: 'Documents', media: 'image', pages: ['p001', 'p002'], row: true, count: 3000, from: 2 },
      { files: '4-6', folder: 'Documents', media: 'image', pages: ['p001', 'p002', 'p003', 'p004'], row: true, count: 6000, from: 2 },
    );
  }
  return {
    name: 'bhc',
    size,
    files: n(16, 70_000),
    examples: ['Artifacts/BHC_006_AF_001_front.jpg', 'Artifacts/BHC_006_AF_001_back.jpg', 'Photographs/BHC_006_PH_001_verso.jpg', ...(size > 9 ? ['Documents/BHC_006_DO_001_p001.jpg'] : [])],
    words: {
      'Artifacts/BHC_006_AF_001_front.jpg': { 4: [['front', n(4, 15000)], ['back', n(3, 14040)], ['verso', n(1, 5000)], ...(size > 9 ? [['p001', 13300] as const] : [])] },
      'Documents/BHC_006_DO_001_p001.jpg': { 4: [['p001', 13300], ['p002', 13300]] },
    },
    marks: [
      { find: '_front', pick: 'BHC_006_AF_001_front.jpg', parts: ['front'], ticks: ['back', 'verso'] },
      ...(size > 9 ? [{ find: '_p001', pick: 'BHC_006_DO_001_p001.jpg', parts: ['p001'], ticks: [] }] : []),
    ],
    groups,
    sheet: {
      file: 'register.csv',
      headerLine: 2,
      lines: ['BHC_006 register', 'Accession No,Title,Date,Shelf mark,Notes', 'BHC_006_AF_001,Brass lamp,1950,A1,'],
      rows: n(8, 33_000),
      columns: [
        { header: 'Accession No', groupKeys: n(6, 32_900), fileNames: 0, empty: 0 },
        { header: 'Title', groupKeys: 0, fileNames: 0, empty: n(1, 20) },
        { header: 'Date', groupKeys: 0, fileNames: 0, empty: 0 },
        { header: 'Shelf mark', groupKeys: 0, fileNames: 0, empty: 0 },
        { header: 'Notes', groupKeys: 0, fileNames: 0, empty: n(6, 30_000) },
      ],
    },
    mapping: [],
    typeFrom: 'top',
    typeValues: [['Artifacts', n(5, 15000)], ['Photographs', n(3, 11000)], ['_scratch', 1], ...(size > 9 ? [['Documents', 13300] as const] : [])],
    flags: [
      { kind: 'cells-count', slot: null, count: n(1, 4), detail: 'BHC_006_AF_003,"Short row' },
      { kind: 'no-file', slot: null, count: n(1, 30), detail: null },
      { kind: 'exists', slot: null, count: n(1, 12), detail: null },
      { kind: 'date-form', slot: 'dcterms:date', count: n(1, 60), detail: 'circa 1950s?' },
      { kind: 'unknown-type', slot: 'template', count: n(9, 39000), detail: 'Artifacts' },
      { kind: 'not-media', slot: null, count: 1, detail: 'application/CDFV2' },
    ],
    identical: n(0, 12),
    unpairedRows: n(1, 30),
    unpairedFiles: n(3, 500),
    untitled: n(2, 300),
    leave: ['1 file, all pictures', 'no row in your list', '“Artifacts”'],
    proto: size > 9 ? { decisions: 38, clicks: 22 } : { decisions: 29, clicks: 16 },
  };
};

export const scenario = (name: Scenario['name'], size: number): Scenario => (name === 'meera' ? meera(size) : bhc(size));
