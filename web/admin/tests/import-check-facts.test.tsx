/**
 * What Check and "Your list" count when a list's rows describe files, over
 * the fake server: items without a row are counted once each, however many
 * files they have; a row that names no file of the import is that, with its
 * key as the example and nothing to keep; and "Your list" tells rows that
 * name no file apart from rows whose file this Omeka refuses.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import type { Scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const AF = 'BHC006_GMT_AF.xlsx';

/** The AF catalogue in use, one row per file, joined by file name. */
const FILE_ROWS: Record<string, unknown> = {
  sheets: [
    {
      id: 'folder-list-1',
      source: 's1',
      file: AF,
      hash: null,
      read: { encoding: 'auto', delimiter: 'auto', quote: '"', headerLine: 1, trim: true },
      rowKind: 'media',
      join: { on: 'fileName', column: { index: 0, header: 'File' }, normalize: { caseFold: true, leadingZeros: true, separators: true } },
      itemKey: null,
      columns: [{ column: { index: 0, header: 'File' }, options: { split: null, type: 'literal', lang: null, private: false } }],
    },
  ],
};

const bhc006 = (edits: Partial<Scenario>): Scenario => ({
  ...scenario('bhc', 9),
  lists: [{ file: AF, rows: 690, paired: 680, normalized: 680, columns: ['File'] }],
  flags: [],
  unpairedRows: 0,
  unpairedFiles: 0,
  identical: 0,
  ...edits,
});

const open = (screenName: 'check' | 'list', found: Scenario): void => {
  installFetch(simpleServer(found, FILE_ROWS).handler);
  render(<ImportPage param="1" rest={['1', 'simple', screenName]} />);
};

afterEach(() => {
  window.location.hash = '';
});

describe('Check, when rows describe files', () => {
  it('counts the items that have files but no row, not their files', async () => {
    open('check', bhc006({ unpairedFiles: 21, unpairedItems: 15 }));
    const questions = await screen.findByRole('listbox', { name: 'Questions' }, { timeout: 5000 });
    expect(within(questions).getByRole('option', { name: 'Items with no row' })).toHaveTextContent('15');
    fireEvent.click(within(questions).getByRole('option', { name: 'Items with no row' }));
    expect(await screen.findByRole('region', { name: '15 items have files but no row in your list.' })).toBeInTheDocument();
  }, 20_000);

  it('says a row that names no file of the import is that, with its key, and offers only to leave it out', async () => {
    open('check', bhc006({ flags: [{ kind: 'no-file', slot: null, count: 10, detail: 'BHC006_GMT_PN_049.png', row: 'BHC006_GMT_PN_049.png#1' }] }));
    const card = await screen.findByRole('region', { name: '10 rows of your list name no file of this import.' }, { timeout: 5000 });
    expect(card).toHaveTextContent('For example, BHC006_GMT_PN_049.png');
    const options = within(within(card).getByRole('group', { name: 'Answer for all 10' })).getAllByRole('button');
    expect(options.map((option) => option.textContent)).toEqual(['Leave out✓ suggested']);
    expect(options[0]).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByText(/No item in Omeka/)).toBeNull();
  }, 20_000);
});

describe('Your list, when some rows name a file this Omeka refuses', () => {
  it('says how many rows name no file and how many name a refused file, apart, with the way to Check', async () => {
    open('list', bhc006({ unpairedRows: 10, flags: [{ kind: 'listed-not-media', slot: null, count: 82, detail: 'BHC006_GMT_AF_296a.NEF#1 .nef' }] }));
    const facts = await screen.findByText(/^Lists in use/, { selector: 'span' }, { timeout: 5000 });
    const said = facts.parentElement ?? facts;
    expect(said).toHaveTextContent('Rows paired 598 / 690');
    expect(said).toHaveTextContent('Rows naming no file 10');
    expect(said).toHaveTextContent('Rows naming a refused file 82');
    expect(within(said).getByRole('link', { name: 'Check' })).toHaveAttribute('href', '#/import/1/simple/check');
  }, 20_000);
});
