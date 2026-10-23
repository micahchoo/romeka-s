/**
 * What Check and a folder's Lists section count when a list's rows describe
 * files, over the fake server: items without a row are counted once each,
 * however many files they have; a row that names no file of the import is
 * that, with its key as the example and nothing to keep; and rows that name
 * no file are told apart from rows whose file this Omeka refuses.
 */
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
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

const open = (place: 'simple/check' | 'advanced/lists', found: Scenario): void => {
  installFetch(simpleServer(found, FILE_ROWS).handler);
  render(<ImportPage param="1" rest={['1', ...place.split('/')]} />);
};

afterEach(() => {
  window.location.hash = '';
});

describe('Check, when rows describe files', () => {
  it('counts the items that have files but no row, not their files', async () => {
    open('simple/check', bhc006({ unpairedFiles: 21, unpairedItems: 15 }));
    const questions = await screen.findByRole('listbox', { name: 'Questions' }, { timeout: 5000 });
    expect(within(questions).getByRole('option', { name: 'Items with no row' })).toHaveTextContent('15');
    fireEvent.click(within(questions).getByRole('option', { name: 'Items with no row' }));
    expect(await screen.findByRole('region', { name: '15 items have files but no row in your list.' })).toBeInTheDocument();
  }, 20_000);

  it('says a row that names no file of the import is that, with its key, and offers only to leave it out', async () => {
    open('simple/check', bhc006({ flags: [{ kind: 'no-file', slot: null, count: 10, detail: 'BHC006_GMT_PN_049.png', row: 'BHC006_GMT_PN_049.png#1' }] }));
    const card = await screen.findByRole('region', { name: '10 rows of your list name no file of this import.' }, { timeout: 5000 });
    expect(card).toHaveTextContent('For example, BHC006_GMT_PN_049.png');
    const options = within(within(card).getByRole('group', { name: 'Answer for all 10' })).getAllByRole('button');
    expect(options.map((option) => option.textContent)).toEqual(['Leave out✓ suggested']);
    expect(options[0]).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByText(/No item in Omeka/)).toBeNull();
  }, 20_000);
});

describe('a folder’s list, when some rows name a file this Omeka refuses', () => {
  it('Advanced’s Lists section counts the rows that name no file, and Check counts the rows whose file is refused, apart', async () => {
    const found = bhc006({ unpairedRows: 10, flags: [{ kind: 'listed-not-media', slot: null, count: 82, detail: 'BHC006_GMT_AF_296a.NEF#1 .nef' }] });
    open('advanced/lists', found);
    const unpaired = await screen.findByText(/^10 rows have no file\./, {}, { timeout: 5000 });
    expect(unpaired).toHaveTextContent('10 rows have no file. Every item with files has a row.');
    expect(within(unpaired).getByRole('link', { name: 'Pair them by hand' })).toBeInTheDocument();
    cleanup();

    open('simple/check', found);
    const questions = await screen.findByRole('listbox', { name: 'Questions' }, { timeout: 5000 });
    const refused = within(questions).getByRole('option', { name: 'A file your list names that Omeka does not take' });
    expect(refused).toHaveTextContent('82');
    fireEvent.click(refused);
    const fact = await screen.findByRole('region', { name: '82 files in your list are of a type Omeka refuses.' });
    expect(fact).toHaveTextContent('BHC006_GMT_AF_296a.NEF');
    expect(within(fact).getByRole('link', { name: /allowed file types/ })).toHaveAttribute('href', '#/settings');
  }, 20_000);
});
