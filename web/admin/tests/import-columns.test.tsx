/**
 * "Where each column goes" over a list shaped like BHC006's Artifacts
 * workbook: its first column, `Sl No`, is empty in every row; each row
 * describes one file; Keywords vary inside 42 of 251 items and hold several
 * values at commas in 610 of 690 cells; the year never varies. Each column
 * shows its own first cell, also under a title line; a column's first field
 * goes where its count suggests and is split as its cells suggest, in both
 * modes and when the list read maps it by name. Each answer is read back
 * from the plan the screen saves. Lines are what the sheet view sends: a
 * workbook's rows from line 1, written as CSV lines.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import type { ColumnCells, Grain } from '../src/api/import-sections';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import type { Scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const AF = 'BHC006_GMT_AF.xlsx';
const COLUMNS = ['Sl No', 'File', 'Keywords', 'Year Of Digitization', 'Digitized by'];
const SL_NO = 0;
const FILE = 1;
const KEYWORDS = 2;
const YEAR = 3;
const DIGITIZED = 4;

const GRAIN: Grain = {
  groupsWithManyRows: 251,
  rowsNamingOtherFilesOfAGroup: 302,
  columns: COLUMNS.map((header, index) => ({ index, header, varies: { groups: index === FILE ? 251 : index === KEYWORDS ? 42 : 0, of: 251 } })),
};

const FIRST_ROW = ',BHC006_GMT_AF_001a.jpg,"Coral, Natural History, Fossil",2017,Dilip Patil';

const CELLS: readonly ColumnCells[] = [
  { index: SL_NO, header: 'Sl No', separator: null },
  { index: FILE, header: 'File', separator: null },
  { index: KEYWORDS, header: 'Keywords', separator: { text: ',', cells: 610 } },
  { index: YEAR, header: 'Year Of Digitization', separator: null },
  { index: DIGITIZED, header: 'Digitized by', separator: { text: ',', cells: 40 } },
];

const bhc006: Scenario = {
  ...scenario('bhc', 9),
  lists: [
    {
      file: AF,
      rows: 690,
      paired: 680,
      normalized: 680,
      columns: COLUMNS,
      grain: GRAIN,
      cells: CELLS,
      lines: [COLUMNS.join(','), FIRST_ROW],
    },
  ],
};

const ref = (index: number) => ({ index, header: COLUMNS[index] ?? '' });
const fromColumn = (index: number) => ({ from: 'column', sheet: 'folder-list-1', column: ref(index) });

/** The list in use, each row one file, no column in a field yet. */
const FILE_ROWS: Record<string, unknown> = {
  sheets: [
    {
      id: 'folder-list-1',
      source: 's1',
      file: AF,
      hash: null,
      read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
      rowKind: 'media',
      join: { on: 'fileName', column: ref(FILE), normalize: { caseFold: false, leadingZeros: false, separators: false } },
      itemKey: null,
      columns: COLUMNS.map((_, index) => ({ column: ref(index), options: { split: null, type: 'literal', lang: null, private: false } })),
    },
  ],
  slots: { item: {}, media: {}, itemSet: {} },
};

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};
const sourcesOf = (plan: Record<string, unknown>, kind: 'item' | 'media', slot: string): unknown => {
  const rule = asRecord(asRecord(plan['slots'], [])[kind], [])[slot];
  return rule === undefined ? undefined : asRecord(rule, [])['sources'];
};
const splitOf = (plan: Record<string, unknown>, index: number): unknown => {
  const sheet = asRecord(Array.isArray(plan['sheets']) ? plan['sheets'][0] : null, []);
  const columns = Array.isArray(sheet['columns']) ? sheet['columns'] : [];
  const column = columns.map((one) => asRecord(one, [])).find((one) => asRecord(one['column'], [])['index'] === index);
  return asRecord(column?.['options'], [])['split'];
};

const open = (screenName: string): ReturnType<typeof simpleServer> => {
  const server = simpleServer(bhc006, FILE_ROWS);
  installFetch(server.handler);
  const [mode, name] = screenName.split('/');
  render(<ImportPage param="1" rest={['1', mode ?? 'simple', name ?? 'list']} />);
  return server;
};

const block = async (): Promise<HTMLElement> => screen.findByRole('region', { name: AF }, { timeout: 5000 });
/** The row headed by `column`; Simple writes the column's first value under its name. */
const headerOf = (element: HTMLElement, column: string): HTMLElement =>
  within(element).getByRole('rowheader', { name: (name) => name === column || name.startsWith(`${column} `) });
const rowOf = (element: HTMLElement, column: string): HTMLElement => {
  const row = headerOf(element, column).closest('tr');
  if (row === null) {
    throw new Error(`no row for ${column}`);
  }
  return row;
};
const goesInto = async (element: HTMLElement, column: string): Promise<HTMLElement> =>
  within(element).findByRole('combobox', { name: `Where the column ${column} goes` }, { timeout: 5000 });
const choose = async (select: HTMLElement, value: string): Promise<void> => {
  await waitFor(() => {
    expect(within(select).getByRole('option', { name: /^Description/ })).toBeInTheDocument();
  }, { timeout: 5000 });
  fireEvent.change(select, { target: { value } });
};

afterEach(() => {
  window.location.hash = '';
});

describe('the first value of each column', () => {
  it('is the cell under that column, when the first column of the row is empty', async () => {
    open('simple/list');
    const af = await block();
    await goesInto(af, 'Keywords');
    const first = (column: string): string => headerOf(af, column).textContent.slice(column.length).trim();
    expect(first('Sl No')).toBe('');
    expect(first('Keywords')).toBe('Coral, Natural History, Fossil');
    expect(first('Year Of Digitization')).toBe('2017');
    expect(first('Digitized by')).toBe('Dilip Patil');
  }, 20_000);
});

describe('several values in one cell, in Simple', () => {
  it('cuts Keywords at commas from its first field, as most of its cells hold one, until the person says no', async () => {
    const server = open('simple/list');
    const af = await block();
    await choose(await goesInto(af, 'Keywords'), 'field:dcterms:description');
    const split = await within(rowOf(af, 'Keywords')).findByRole('checkbox', { name: /^split at “,”/ }, { timeout: 3000 });
    expect(split).toBeChecked();
    expect(rowOf(af, 'Keywords')).toHaveTextContent('610/690');
    await waitFor(() => {
      expect(splitOf(last(server.saved), KEYWORDS)).toBe(',');
    }, { timeout: 3000 });
    fireEvent.click(split);
    await waitFor(() => {
      expect(splitOf(last(server.saved), KEYWORDS)).toBeNull();
    }, { timeout: 3000 });
    expect(within(rowOf(af, 'Keywords')).getByRole('checkbox', { name: /^split at “,”/ })).not.toBeChecked();
  }, 20_000);

  it('offers the split but leaves it off for a column where few cells hold a comma', async () => {
    const server = open('simple/list');
    const af = await block();
    expect(within(rowOf(af, 'Digitized by')).queryByRole('checkbox')).toBeNull();
    await choose(await goesInto(af, 'Digitized by'), 'field:dcterms:description');
    const split = await within(rowOf(af, 'Digitized by')).findByRole('checkbox', { name: /^split at “,”/ }, { timeout: 3000 });
    expect(split).not.toBeChecked();
    expect(rowOf(af, 'Digitized by')).toHaveTextContent('40/690');
    await waitFor(() => {
      expect(sourcesOf(last(server.saved), 'item', 'dcterms:description')).toEqual([fromColumn(DIGITIZED)]);
    }, { timeout: 3000 });
    expect(splitOf(last(server.saved), DIGITIZED)).toBeNull();
    fireEvent.click(split);
    await waitFor(() => {
      expect(splitOf(last(server.saved), DIGITIZED)).toBe(',');
    }, { timeout: 3000 });
  }, 20_000);
});

describe('a list whose header is on line 2, under a title, with an empty first column', () => {
  const TITLE = 'Register of BHC006';
  const LINES = [TITLE, COLUMNS.join(','), '', FIRST_ROW];
  const titled = (file: string): Scenario => ({
    ...scenario('bhc', 9),
    lists: [{ file, rows: 690, paired: 680, normalized: 680, columns: COLUMNS, grain: GRAIN, cells: CELLS, headerLine: 2, lines: LINES }],
  });
  const onLine2 = (file: string): Record<string, unknown> => {
    const sheets = Array.isArray(FILE_ROWS['sheets']) ? FILE_ROWS['sheets'].map((one) => asRecord(one, [])) : [];
    return { ...FILE_ROWS, sheets: sheets.map((one) => ({ ...one, file, read: { ...asRecord(one['read'], []), headerLine: 2 } })) };
  };

  it.each([AF, 'Artifacts/BHC006_GMT_AF.csv'])('%s: the header-line choice shows line 1, and each column its own first cell', async (file) => {
    installFetch(simpleServer(titled(file), onLine2(file)).handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'list']} />);
    const list = await screen.findByRole('region', { name: file }, { timeout: 5000 });
    await goesInto(list, 'Keywords');
    expect(within(list).getByText(/^Line 1 has 1 cell: “Register of BHC006”\./)).toBeInTheDocument();
    expect(within(list).getByRole('spinbutton', { name: 'Column names are on line' })).toHaveValue(2);
    const first = (column: string): string => headerOf(list, column).textContent.slice(column.length).trim();
    expect(first('Sl No')).toBe('');
    expect(first('Keywords')).toBe('Coral, Natural History, Fossil');
    expect(first('Year Of Digitization')).toBe('2017');
    expect(first('Digitized by')).toBe('Dilip Patil');
  }, 20_000);
});

describe('a list read for the first time, its columns mapped by name', () => {
  const NAMED = ['Sl No', 'File', 'Type', 'Date'];
  const TYPE = 2;
  const DATE = 3;
  const named = (file: string): Scenario => ({
    ...scenario('bhc', 9),
    lists: [
      {
        file,
        rows: 690,
        paired: 680,
        normalized: 680,
        columns: NAMED,
        headerLine: 2,
        cells: NAMED.map((header, index) => ({ index, header, separator: index === TYPE ? { text: ',', cells: 610 } : null })),
        lines: ['Register of BHC006', NAMED.join(','), ',BHC006_GMT_AF_001a.jpg,"Coral, Natural History, Fossil",2017'],
      },
    ],
  });
  const added = async (file: string, mode: string, screenName: string): Promise<Record<string, unknown>> => {
    const server = simpleServer(named(file));
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', mode, screenName]} />);
    if (mode === 'simple') {
      fireEvent.click(await screen.findByRole('option', { name: file }, { timeout: 5000 }));
      fireEvent.click(within(await screen.findByRole('region', { name: file })).getByRole('checkbox', { name: 'use this list' }));
    } else {
      const offer = await screen.findByRole('listitem', { name: file }, { timeout: 5000 });
      fireEvent.click(within(offer).getByRole('button', { name: 'Use this list' }));
    }
    await waitFor(() => {
      expect(Array.isArray(last(server.saved)['sheets']) ? last(server.saved)['sheets'] : []).toHaveLength(1);
    }, { timeout: 5000 });
    return last(server.saved);
  };
  const named1 = (index: number) => ({ from: 'column', sheet: 'folder-list-1', column: { index, header: NAMED[index] ?? '' } });

  it.each([AF, 'Artifacts/BHC006_GMT_AF.csv'])('%s: a column most of whose cells hold a comma is cut at commas, in Simple and Advanced alike', async (file) => {
    const simple = await added(file, 'simple', 'list');
    expect(sourcesOf(simple, 'item', 'dcterms:type')).toEqual([named1(TYPE)]);
    expect(sourcesOf(simple, 'item', 'dcterms:date')).toEqual([named1(DATE)]);
    expect(splitOf(simple, TYPE)).toBe(',');
    expect(splitOf(simple, DATE)).toBeNull();
    expect(asRecord(asRecord(Array.isArray(simple['sheets']) ? simple['sheets'][0] : null, [])['read'], [])['headerLine']).toBe(2);
    cleanup();
    const advanced = await added(file, 'advanced', 'lists');
    expect({ sheets: advanced['sheets'], slots: advanced['slots'] }).toEqual({ sheets: simple['sheets'], slots: simple['slots'] });
  }, 30_000);
});

describe('several values in one cell, in Advanced', () => {
  it('cuts Keywords at commas from its first field, as Simple does', async () => {
    const server = open('advanced/lists');
    const af = await block();
    await choose(await within(af).findByRole('combobox', { name: 'Field for Keywords' }, { timeout: 5000 }), 'dcterms:description');
    await waitFor(() => {
      expect(splitOf(last(server.saved), KEYWORDS)).toBe(',');
    }, { timeout: 3000 });
    expect(within(af).getByRole('textbox', { name: 'Split Keywords at' })).toHaveValue(',');
    await choose(within(af).getByRole('combobox', { name: 'Field for Digitized by' }), 'dcterms:type');
    await waitFor(() => {
      expect(sourcesOf(last(server.saved), 'item', 'dcterms:type')).toEqual([fromColumn(DIGITIZED)]);
    }, { timeout: 3000 });
    expect(splitOf(last(server.saved), DIGITIZED)).toBeNull();
  }, 20_000);
});

describe('a column’s first field, on a list whose rows describe files: the field decides, not the count', () => {
  const placed = async (screenName: string, column: string, field: string): Promise<{ plan: Record<string, unknown>; af: HTMLElement }> => {
    const server = open(screenName);
    const af = await block();
    const select = screenName.startsWith('advanced')
      ? await within(af).findByRole('combobox', { name: `Field for ${column}` }, { timeout: 5000 })
      : await goesInto(af, column);
    await choose(select, screenName.startsWith('advanced') ? field : `field:${field}`);
    await waitFor(() => {
      expect(sourcesOf(last(server.saved), 'item', field) ?? sourcesOf(last(server.saved), 'media', field)).toBeDefined();
    }, { timeout: 3000 });
    return { plan: last(server.saved), af };
  };

  it.each(['simple/list', 'advanced/lists'])('%s: Keywords go to the item, with the count still shown', async (screenName) => {
    const { plan, af } = await placed(screenName, 'Keywords', 'dcterms:subject');
    expect(sourcesOf(plan, 'item', 'dcterms:subject')).toEqual([fromColumn(KEYWORDS)]);
    expect(sourcesOf(plan, 'media', 'dcterms:subject')).toBeUndefined();
    expect(rowOf(af, 'Keywords')).toHaveTextContent(screenName.startsWith('advanced') ? 'Varies in 42 of 251 items.' : 'differs in 42/251');
  }, 20_000);

  it.each(['simple/list', 'advanced/lists'])('%s: a column mapped to Identifier goes on each file', async (screenName) => {
    const { plan } = await placed(screenName, 'Digitized by', 'dcterms:identifier');
    expect(sourcesOf(plan, 'media', 'dcterms:identifier')).toEqual([fromColumn(DIGITIZED)]);
    expect(sourcesOf(plan, 'item', 'dcterms:identifier')).toBeUndefined();
  }, 20_000);
});

describe('what the list says about its encoding', () => {
  it.each([AF, 'Artifacts/BHC006_GMT_AF.ods'])('%s: a workbook has no text encoding, so nothing is said', async (file) => {
    const scene: Scenario = { ...bhc006, lists: [{ ...(bhc006.lists?.[0] ?? { file, rows: 0, paired: 0, normalized: 0 }), file }] };
    const sheets = Array.isArray(FILE_ROWS['sheets']) ? FILE_ROWS['sheets'].map((one) => asRecord(one, [])) : [];
    installFetch(simpleServer(scene, { ...FILE_ROWS, sheets: sheets.map((one) => ({ ...one, file })) }).handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'list']} />);
    const list = await screen.findByRole('region', { name: file }, { timeout: 5000 });
    await goesInto(list, 'Keywords');
    expect(list).not.toHaveTextContent('UTF-8');
  }, 20_000);

  it('a CSV still says it is UTF-8 text', async () => {
    const file = 'Artifacts/BHC006_GMT_AF.csv';
    const scene: Scenario = { ...bhc006, lists: [{ ...(bhc006.lists?.[0] ?? { file, rows: 0, paired: 0, normalized: 0 }), file }] };
    const sheets = Array.isArray(FILE_ROWS['sheets']) ? FILE_ROWS['sheets'].map((one) => asRecord(one, [])) : [];
    installFetch(simpleServer(scene, { ...FILE_ROWS, sheets: sheets.map((one) => ({ ...one, file })) }).handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'list']} />);
    const list = await screen.findByRole('region', { name: file }, { timeout: 5000 });
    await goesInto(list, 'Keywords');
    expect(list).toHaveTextContent('It is UTF-8 text.');
  }, 20_000);
});
