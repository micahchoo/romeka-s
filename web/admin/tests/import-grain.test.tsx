/**
 * What one row of a list describes, over a list shaped like BHC006's
 * Artifacts catalogue: one row per view, 367 rows naming another view of an
 * item a row already names, Keywords that differ between the views of 43 of
 * 274 items and a year of digitization that never does. Simple asks the
 * question with that count; Advanced shows the same answer as settings, and
 * an edit in either shows in the other. Each answer is read back from the
 * plan the screen saves.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import type { Grain } from '../src/api/import-sections';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import type { Scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const AF = 'BHC006_GMT_AF.xlsx';
const COLUMNS = ['File', 'Keywords', 'Year Of Digitization'];

const GRAIN: Grain = {
  groupsWithManyRows: 274,
  rowsNamingOtherFilesOfAGroup: 367,
  columns: [
    { index: 0, header: 'File', varies: { groups: 274, of: 274 } },
    { index: 1, header: 'Keywords', varies: { groups: 43, of: 274 } },
    { index: 2, header: 'Year Of Digitization', varies: { groups: 0, of: 274 } },
  ],
};

const DUPLICATES = { kind: 'duplicate-row', slot: null, count: 367, detail: 'BHC006_GMT_AF_010b.jpg' };

const bhc006: Scenario = {
  ...scenario('bhc', 9),
  lists: [{ file: AF, rows: 690, paired: 680, normalized: 680, columns: COLUMNS, grain: GRAIN }],
  flags: [DUPLICATES],
};

const ref = (index: number) => ({ index, header: COLUMNS[index] ?? '' });
const fromColumn = (index: number) => ({ from: 'column', sheet: 'folder-list-1', column: ref(index) });

/** The list in use as one item a row, Keywords in Subject and the year in Date: the plan before the question. */
const ONE_ITEM_A_ROW: Record<string, unknown> = {
  sheets: [
    {
      id: 'folder-list-1',
      source: 's1',
      file: AF,
      hash: null,
      read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
      rowKind: 'item',
      join: { on: 'fileName', column: ref(0), normalize: { caseFold: false, leadingZeros: false, separators: false } },
      itemKey: null,
      columns: COLUMNS.map((_, index) => ({ column: ref(index), options: { split: ',', type: 'literal', lang: null, private: false } })),
    },
  ],
  slots: {
    item: {
      'dcterms:subject': { sources: [fromColumn(1)], combine: 'all' },
      'dcterms:date': { sources: [fromColumn(2)], combine: 'first' },
    },
    media: {},
    itemSet: {},
  },
};

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};
const sheet = (plan: Record<string, unknown>): Record<string, unknown> => asRecord(Array.isArray(plan['sheets']) ? plan['sheets'][0] : null, []);
const rules = (plan: Record<string, unknown>, kind: 'item' | 'media'): Record<string, unknown> => asRecord(asRecord(plan['slots'], [])[kind], []);
const sourcesOf = (plan: Record<string, unknown>, kind: 'item' | 'media', slot: string): unknown => {
  const rule = rules(plan, kind)[slot];
  return rule === undefined ? undefined : asRecord(rule, [])['sources'];
};

const open = (screenName: string, plan: Record<string, unknown>): ReturnType<typeof simpleServer> => {
  const server = simpleServer(bhc006, plan);
  installFetch(server.handler);
  const [mode, name] = screenName.split('/');
  render(<ImportPage param="1" rest={['1', mode ?? 'simple', name ?? 'list']} />);
  return server;
};

const block = async (): Promise<HTMLElement> => screen.findByRole('region', { name: AF }, { timeout: 5000 });
const levelGroup = (element: HTMLElement, column: string): HTMLElement => within(element).getByRole('group', { name: `Where the values of ${column} go` });

afterEach(() => {
  window.location.hash = '';
});

/** Simple: answers "one file" and returns the plan it saved. */
const answeredOneFile = async (): Promise<Record<string, unknown>> => {
  const server = open('simple/list', ONE_ITEM_A_ROW);
  const af = await block();
  const question = await within(af).findByRole('group', { name: 'Each row describes' }, { timeout: 5000 });
  expect(question).toHaveTextContent('367 rows name another file of an item that a row already names; the rows look like they describe one file each.');
  const oneFile = within(question).getByRole('radio', { name: /^one file/ });
  expect(oneFile.closest('label')).toHaveTextContent('suggested');
  expect(within(question).getByRole('radio', { name: 'one item' })).toBeChecked();
  expect(within(question).getByRole('radio', { name: 'one item' }).closest('label')).not.toHaveTextContent('suggested');
  fireEvent.click(oneFile);
  await waitFor(() => {
    expect(sheet(last(server.saved))['rowKind']).toBe('media');
  }, { timeout: 3000 });
  return last(server.saved);
};

describe('each row describes one file, in Simple', () => {
  it('asks with the count, suggests one file, and saves media rows that join by file name', async () => {
    const plan = await answeredOneFile();
    expect(asRecord(sheet(plan)['join'], [])['on']).toBe('fileName');
  }, 20_000);

  it('puts Keywords, which vary in 43 of 274 items, on each file, and the year, which never varies, on the item', async () => {
    const plan = await answeredOneFile();
    expect(sourcesOf(plan, 'media', 'dcterms:subject')).toEqual([fromColumn(1)]);
    expect(sourcesOf(plan, 'item', 'dcterms:subject')).toBeUndefined();
    expect(sourcesOf(plan, 'item', 'dcterms:date')).toEqual([fromColumn(2)]);
    const af = await block();
    const keywords = await waitFor(() => levelGroup(af, 'Keywords'), { timeout: 3000 });
    expect(within(keywords).getByRole('radio', { name: /^Each file/ })).toBeChecked();
    expect(within(keywords).getByRole('radio', { name: /^Each file/ }).closest('label')).toHaveTextContent('suggested');
    expect(keywords.closest('td')).toHaveTextContent('Varies in 43 of 274 items.');
    const year = levelGroup(af, 'Year Of Digitization');
    expect(within(year).getByRole('radio', { name: /^The item/ })).toBeChecked();
    expect(year.closest('td')).toHaveTextContent('Varies in 0 of 274 items.');
  }, 20_000);

  it('moves Keywords to the item when the person says so', async () => {
    const plan = await answeredOneFile();
    cleanup();
    const server = open('simple/list', plan);
    const keywords = await waitFor(async () => levelGroup(await block(), 'Keywords'), { timeout: 5000 });
    fireEvent.click(within(keywords).getByRole('radio', { name: /^The item/ }));
    await waitFor(() => {
      expect(sourcesOf(last(server.saved), 'item', 'dcterms:subject')).toEqual([fromColumn(1)]);
    }, { timeout: 3000 });
    expect(sourcesOf(last(server.saved), 'media', 'dcterms:subject')).toBeUndefined();
  }, 20_000);
});

describe('the same plan in Advanced', () => {
  it('shows media rows and each column’s place, and its edit shows in Simple', async () => {
    const plan = await answeredOneFile();
    cleanup();
    const advanced = open('advanced/lists', plan);
    const af = await block();
    expect(await within(af).findByRole('combobox', { name: 'Each row describes' }, { timeout: 5000 })).toHaveValue('media');
    const keywords = await within(af).findByRole('combobox', { name: 'Where the values of Keywords go' }, { timeout: 5000 });
    expect(keywords).toHaveValue('file');
    expect(keywords.closest('td')).toHaveTextContent('Varies in 43 of 274 items.');
    const year = within(af).getByRole('combobox', { name: 'Where the values of Year Of Digitization go' });
    expect(year).toHaveValue('item');
    fireEvent.change(year, { target: { value: 'file' } });
    await waitFor(() => {
      expect(sourcesOf(last(advanced.saved), 'media', 'dcterms:date')).toEqual([fromColumn(2)]);
    }, { timeout: 3000 });
    const edited = last(advanced.saved);
    cleanup();

    open('simple/list', edited);
    const yearInSimple = await waitFor(async () => levelGroup(await block(), 'Year Of Digitization'), { timeout: 5000 });
    expect(within(yearInSimple).getByRole('radio', { name: /^Each file/ })).toBeChecked();
  }, 30_000);
});

describe('Check: two rows for one item', () => {
  it('offers first, and suggests, that each row describes one file, and that answer saves media rows', async () => {
    const server = open('simple/check', ONE_ITEM_A_ROW);
    const card = await screen.findByRole('article', { name: /Two rows for one item/ }, { timeout: 5000 });
    const options = within(card).getAllByRole('radio');
    expect(options.map((option) => option.closest('label')?.textContent)).toEqual([
      'Each row describes one filesuggested',
      'Keep them as they are',
      'Leave them out',
    ]);
    expect(options[0]).toBeChecked();
    fireEvent.click(within(card).getByRole('button', { name: 'Use the suggested answer' }));
    await waitFor(() => {
      expect(sheet(last(server.saved))['rowKind']).toBe('media');
    }, { timeout: 3000 });
    expect(sourcesOf(last(server.saved), 'media', 'dcterms:subject')).toEqual([fromColumn(1)]);
  }, 20_000);
});

describe('Add: questions nobody answered in Check', () => {
  /** BHC006 with items of three views and no title: both suggestions edit the plan. */
  const untitled: Scenario = {
    ...bhc006,
    groups: [{ files: '3', folder: 'Artifacts', media: 'image', row: true, count: 274, from: 0 }],
    flags: [DUPLICATES],
    untitled: 274,
  };
  const openAdd = (plan: Record<string, unknown>): ReturnType<typeof simpleServer> => {
    const server = simpleServer(untitled, plan);
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'add']} />);
    return server;
  };
  const titleOf = (plan: Record<string, unknown>): unknown => sourcesOf(plan, 'item', 'dcterms:title');

  it('writes each row as one file and the item code as the title, and says both with a way back to Check', async () => {
    const server = openAdd(ONE_ITEM_A_ROW);
    const rows = await screen.findByText(`Each row of “${AF}” describes one file.`, { exact: false }, { timeout: 5000 });
    expect(within(rows).getByRole('link', { name: 'Change it' })).toHaveAttribute('href', '#/import/1/simple/check');
    const title = screen.getByText(/get their item code as the title/);
    expect(within(title).getByRole('link', { name: 'Change it' })).toHaveAttribute('href', '#/import/1/simple/check');
    await waitFor(() => {
      expect(sheet(last(server.saved))['rowKind']).toBe('media');
    }, { timeout: 3000 });
    const plan = last(server.saved);
    expect(asRecord(sheet(plan)['join'], [])['on']).toBe('fileName');
    expect(titleOf(plan)).toEqual([{ from: 'group', name: 'identifier' }]);
    expect(asRecord(plan['simple'], [])['answered']).toEqual(['untitled', 'duplicate-row']);
    const saves = server.saved.length;
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(server.saved).toHaveLength(saves);
  }, 20_000);

  it('keeps one item a row for a person who answered "leave them out" on two rows for one item', async () => {
    const server = openAdd({ ...ONE_ITEM_A_ROW, flags: { 'duplicate-row': 'leave' }, simple: { answered: ['duplicate-row'] } });
    await waitFor(() => {
      expect(titleOf(last(server.saved))).toEqual([{ from: 'group', name: 'identifier' }]);
    }, { timeout: 5000 });
    expect(server.saved.map((plan) => sheet(plan)['rowKind'])).not.toContain('media');
    expect(screen.queryByText(/describes one file/)).toBeNull();
  }, 20_000);
});
