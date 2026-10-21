/**
 * Pair rows and files by hand, at 9 and at 40,000 items: ten of each side at
 * a time whatever the total, a search the server is asked for, and a pick of
 * a row then a file saving one `pair` decision, which Undo takes back.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ImportPage } from '../src/pages/import/page';
import { countOf, hasFile, listsServer, pad, PATH } from './import-lists-server';
import { installFetch, requestBody, requestsFor } from './stub';

afterEach(() => {
  window.location.hash = '';
});

describe.each([9, 40_000])('Pair by hand at %i items', (size) => {
  it('shows ten a side, searches, and saves one pair decision', async () => {
    const server = listsServer(size);
    const fetchMock = installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'pair']} />);
    const rows = countOf(size, (n) => !hasFile(n));
    const files = countOf(size, (n) => n % 10 === 1);
    const rowsHeading = rows === 1 ? '1. Choose a row: 1 row names no file' : `1. Choose a row: ${rows.toLocaleString('en')} rows name no file`;
    expect(await screen.findByRole('heading', { name: rowsHeading })).toBeInTheDocument();
    expect(screen.getAllByRole('radio')).toHaveLength(Math.min(rows, 10));
    expect(screen.getAllByRole('button', { name: 'Choose a row first' })).toHaveLength(Math.min(files, 10));
    await waitFor(() => {
      expect(screen.queryByRole('heading', { name: /rows have no file: pair them in a spreadsheet/ }) !== null).toBe(rows > 30);
    });

    fireEvent.change(screen.getByRole('searchbox', { name: 'Search the files' }), { target: { value: `item_${pad(1)}` } });
    fireEvent.click(within(screen.getByRole('search', { name: 'Search the files' })).getByRole('button', { name: 'Search' }));
    await waitFor(() => {
      const last = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).pop();
      expect(last?.['unpaired']).toEqual({ rows: { page: 1, perPage: 10 }, files: { page: 1, perPage: 10, text: `item_${pad(1)}` } });
    });

    fireEvent.click(screen.getByRole('radio', { name: /item_00003#1/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Pair with the row item_00003#1' }));
    expect(await screen.findByText('The row item_00003#1 with item_00001.jpg')).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: /item_00003#1/ })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    }, { timeout: 3000 });
    expect(server.saved[0]?.['decisions']).toEqual([
      { kind: 'pair', row: { sheet: 'register', key: 'item_00003#1' }, file: { source: 's1', path: 'Letters/item_00001.jpg' } },
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(2);
    }, { timeout: 3000 });
    expect(server.saved[1]?.['decisions']).toEqual([]);
  }, 20_000);
});

describe('the pairs list at 40,000 items', () => {
  it('goes down as a link, comes back as pairs, says what made none and undoes the pairs', async () => {
    const server = listsServer(40_000);
    const fetchMock = installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'pair']} />);
    const rows = countOf(40_000, (n) => !hasFile(n));
    const card = (await screen.findByRole('heading', { name: `${rows.toLocaleString('en')} rows have no file: pair them in a spreadsheet` })).closest('section');
    if (card === null) {
      throw new Error('the pairs list has no card');
    }
    expect(within(card).getByText(/such as item_00001\.jpg/)).toBeInTheDocument();
    const download = within(card).getByRole('link', { name: `Download the ${rows.toLocaleString('en')} rows (.csv)` });
    expect(download.getAttribute('href')).toBe('/api/romeka/imports/1/pairs.csv');

    fireEvent.change(within(card).getByLabelText('Choose the file'), { target: { files: [new File(['Row [sheet:key],Files\n'], 'pairs.csv', { type: 'text/csv' })] } });
    expect(await within(card).findByText('Paired 1 row.')).toBeInTheDocument();
    expect(requestsFor(fetchMock, `${PATH}/pairs`)).toHaveLength(1);
    expect(within(card).getByText('2 cells made no pair.')).toBeInTheDocument();
    fireEvent.click(within(card).getByText('Show the 2 cells that made no pair'));
    expect(within(card).getByText(/register:NOWHERE#1: “nothing\.jpg”/)).toBeInTheDocument();
    expect(within(card).getByText('No row of this import has this address.')).toBeInTheDocument();
    expect(within(card).getByText('In no item: its name does not fit your example.')).toBeInTheDocument();

    fireEvent.click(within(card).getByRole('button', { name: 'Undo these pairs' }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    }, { timeout: 3000 });
    expect(server.saved[0]?.['decisions']).toEqual([]);
    expect(within(card).queryByText('Paired 1 row.')).not.toBeInTheDocument();
  }, 30_000);
});
