/**
 * The list parts a screen places: the groups by facts (with another example,
 * "show all" and one change for the group), the files-per-item chart, the
 * identical files, and the list round trip (columns, the first rows as they
 * download, the upload and what it did, at 9 and at 40,000 items).
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { listsServer, PATH } from './import-lists-server';
import { installFetch, requestBody, requestsFor } from './stub';

vi.mock('../src/pages/import/simple/bodies', async () => {
  const { Groups } = await import('../src/pages/import/lists/groups');
  const { FilesChart } = await import('../src/pages/import/lists/histogram');
  const { IdenticalFiles } = await import('../src/pages/import/lists/identical');
  const { RoundTrip } = await import('../src/pages/import/lists/round-trip');
  const Found = () => (
    <>
      <FilesChart />
      <IdenticalFiles />
      <Groups />
    </>
  );
  return { SIMPLE_BODIES: { found: Found, list: RoundTrip } };
});

const at = (screenName: string) => <ImportPage param="1" rest={['1', 'simple', screenName]} />;
const filterSegment = (filter: Record<string, string>): string => encodeURIComponent(JSON.stringify(filter));

afterEach(() => {
  window.location.hash = '';
});

describe('the groups, the chart and the identical files', () => {
  it('describe each group by its facts, draw another example, and link to its items', async () => {
    const server = listsServer(9);
    const fetchMock = installFetch(server.handler);
    render(at('found'));
    const group = (await screen.findByText(/^2 items have$/)).closest('article');
    if (group === null) {
      throw new Error('no group card');
    }
    expect(group).toHaveTextContent('2 items have 1 file, all pictures. They are in the folder “Photographs”.');
    expect(group).toHaveTextContent('BHC_006_PH_103');
    fireEvent.click(within(group).getByRole('button', { name: 'Another example' }));
    expect(group).toHaveTextContent('BHC_006_PH_104');
    fireEvent.click(within(group).getByRole('button', { name: 'Another example' }));
    await waitFor(() => {
      const shapes = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).filter((body) => JSON.stringify(body['include']) === '["shapes"]');
      expect(shapes.map((body) => body['seed'])).toEqual([0, 1]);
    });
    const facts = { files: '1', folders: ['Photographs'], media: ['image'], pages: null, parts: [], origin: 'pattern', row: false };
    const all = within(group).getByRole('link', { name: 'Show all 2' }).getAttribute('href') ?? '';
    expect(all.startsWith('#/import/1/simple/all-items/')).toBe(true);
    const named: unknown = JSON.parse(decodeURIComponent(all.split('/').pop() ?? ''));
    const key = asRecord(named, [])['group'];
    expect(typeof key === 'string' ? JSON.parse(key) : null).toEqual(facts);
    expect(screen.getByText('Worth a look: under 2 in 100 items, or 7+ files.')).toBeInTheDocument();

    fireEvent.click(within(group).getByRole('button', { name: 'Leave them out' }));
    fireEvent.click(await within(group).findByRole('button', { name: /^Change these/ }));
    const records = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).filter((body) => JSON.stringify(body['include']) === '["records"]');
    expect(records[0]?.['records']).toEqual({ kind: 'item', shape: facts, page: 1, perPage: 500 });
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    }, { timeout: 3000 });
    expect(JSON.stringify(server.saved[0]?.['decisions'])).toMatch(/^\[\{"kind":"many","entries":\[\{"file"/);

    expect(screen.getByRole('link', { name: '6 items with 1 file. Show them.' })).toHaveAttribute(
      'href',
      `#/import/1/simple/all-items/${filterSegment({ files: '1' })}`,
    );
    expect(screen.getByText('2 files are byte for byte the same as another file.')).toBeInTheDocument();
    expect(screen.getByText(/Misc\/café\.jpg, Misc\/loose\.jpg/)).toBeInTheDocument();
  }, 20_000);
});

describe('the words of a group that the pattern did not make', () => {
  it('say why each such item exists: a list row with no file, or a file made an item of its own', async () => {
    installFetch(listsServer(9).handler);
    render(at('found'));
    await screen.findAllByRole('link', { name: /^Show all/ });
    const texts = Array.from(document.querySelectorAll('article.import-group'), (card) => card.textContent);
    const row = texts.filter((text) => text.includes('a row in your list that has no file'));
    const own = texts.filter((text) => text.includes('a file you made an item of its own'));
    expect(row).toHaveLength(1);
    expect(row[0]).toMatch(/^\d+ items? (has|have) no file, a row in your list that has no file\./);
    expect(own).toHaveLength(1);
    expect(texts.filter((text) => text.includes('does not fit') || text.includes('example you marked'))).toEqual([]);
  }, 20_000);
});

describe.each([9, 40_000])('the list round trip at %i items', (size) => {
  it('shows the first rows as they download, uploads a list and says what it did', async () => {
    const server = listsServer(size, { strays: 7 });
    const fetchMock = installFetch(server.handler);
    render(at('list'));
    const first = await screen.findByRole('region', { name: 'The first rows of the list' });
    expect(within(first).getAllByRole('columnheader').map((cell) => cell.textContent)).toEqual(['Identifier', 'Files', 'Title [dcterms:title]', 'Date [dcterms:date]']);
    expect(within(first).getAllByRole('row')).toHaveLength(4);
    const download = screen.getByRole('link', { name: 'Download the list (9 rows, .csv)' });
    expect(download.getAttribute('href')).toContain('/api/romeka/imports/1/template.csv?columns=dcterms%3Atitle%2Cdcterms%3Adate');

    fireEvent.change(screen.getByRole('combobox', { name: 'Add a column...' }), { target: { value: 'sets' } });
    await waitFor(() => {
      expect(requestsFor(fetchMock, `${PATH}/template.csv`).at(-1)?.url.searchParams.get('columns')).toBe('dcterms:title,dcterms:date,sets');
    });
    expect(screen.getByRole('button', { name: 'Remove the column Item sets' })).toBeInTheDocument();

    const input = screen.getByLabelText('Choose the file');
    fireEvent.change(input, { target: { files: [new File(['Identifier,Files\n'], 'filled.csv', { type: 'text/csv' })] } });
    const total = size + 7;
    expect(await screen.findByText(`Read ${total.toLocaleString('en')} rows from filled.csv.`)).toBeInTheDocument();
    const said = screen.getByText(`Read ${total.toLocaleString('en')} rows from filled.csv.`).closest('p');
    expect(said).toHaveTextContent(`${size.toLocaleString('en')} belong to items of this import. 7 do not.`);
    expect(screen.getByText('Fills “Title [dcterms:title]”, “Date [dcterms:date]”; its values win over other lists and file names.')).toBeInTheDocument();
    expect(screen.getByText('Not used: “Notes”. These names have no [field].')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Show the 7 rows that belong to no item'));
    expect(screen.getByText('nobody_1#1')).toBeInTheDocument();
    expect(requestsFor(fetchMock, `${PATH}/sheets`)).toHaveLength(1);
    const asked = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).filter((body) => body['entries'] !== undefined);
    expect(asked.map((body) => body['entries'])).toEqual([{ sheet: 'list-1', paired: false, perPage: 50 }]);
  }, 20_000);
});
