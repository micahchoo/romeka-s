/**
 * The shared import list (`src/pages/import/paged-list.tsx`) and its filter row
 * (`filters.tsx`): a list of 40,000 rows draws one page of 50, a filter change
 * goes back to page 1, and the filter value is exactly what narrows.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { Paged } from '../src/api/import-sections';
import { Filters } from '../src/pages/import/filters';
import type { FilterField, FilterValue } from '../src/pages/import/filters';
import { PER_PAGE, PagedList } from '../src/pages/import/paged-list';

interface Row {
  readonly id: number;
  readonly name: string;
  readonly folder: string;
}

const ROWS: readonly Row[] = Array.from({ length: 40_000 }, (_, index) => ({
  id: index + 1,
  name: `photo ${String(index + 1).padStart(5, '0')}.jpg`,
  folder: index % 2 === 0 ? 'letters' : 'scans',
}));

type Key = 'text' | 'folder';
const FIELDS: readonly FilterField<Key>[] = [
  { key: 'text', kind: 'text', label: 'Name has' },
  { key: 'folder', kind: 'choice', label: 'Folder', any: 'Any folder', options: [{ value: 'scans', label: 'scans' }] },
];

const serve = (filter: FilterValue<Key>) => (page: number, perPage: number): Promise<Paged<Row>> => {
  const text = filter.text;
  const folder = filter.folder;
  const hits = ROWS.filter((row) => (text === undefined || row.name.includes(text)) && (folder === undefined || row.folder === folder));
  return Promise.resolve({ total: hits.length, page, perPage, items: hits.slice((page - 1) * perPage, page * perPage) });
};

const Files = ({ onFilter }: { readonly onFilter?: (value: FilterValue<Key>) => void }): ReactElement => {
  const [filter, setFilter] = useState<FilterValue<Key>>({});
  return (
    <>
      <Filters
        label="Filter the files"
        fields={FIELDS}
        value={filter}
        onChange={(value) => {
          setFilter(value);
          onFilter?.(value);
        }}
      />
      <PagedList
        label="Every file"
        resource="files"
        load={serve(filter)}
        deps={[filter.text, filter.folder]}
        columns={[{ header: 'Name', cell: (row: Row) => row.name }]}
        rowKey={(row) => String(row.id)}
        empty={<p>No file has that name.</p>}
      />
    </>
  );
};

const bodyRows = (): readonly HTMLElement[] =>
  within(screen.getByRole('region', { name: 'Every file' })).getAllByRole('row').slice(1);

describe('the paged list', () => {
  it('draws one page of 50 from 40,000 rows and pages forward', async () => {
    render(<Files />);
    expect(await screen.findByText(/Page 1 of 800 \(40000 files\)/)).toBeInTheDocument();
    expect(bodyRows()).toHaveLength(PER_PAGE);
    expect(document.querySelectorAll('tbody td')).toHaveLength(50);
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(await screen.findByText(/Page 2 of 800/)).toBeInTheDocument();
    expect(bodyRows()[0]).toHaveTextContent('photo 00051.jpg');
  });

  it('goes back to page 1 when a filter changes, and applies text only on Search', async () => {
    const onFilter = vi.fn();
    render(<Files onFilter={onFilter} />);
    await screen.findByText(/Page 1 of 800/);
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await screen.findByText(/Page 2 of 800/);
    fireEvent.change(screen.getByLabelText('Folder'), { target: { value: 'scans' } });
    expect(await screen.findByText(/Page 1 of 400 \(20000 files\)/)).toBeInTheDocument();
    expect(onFilter).toHaveBeenLastCalledWith({ folder: 'scans' });

    fireEvent.change(screen.getByLabelText('Name has'), { target: { value: '0001' } });
    expect(onFilter).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(onFilter).toHaveBeenLastCalledWith({ folder: 'scans', text: '0001' });
    await screen.findByText('photo 00010.jpg');
    expect(bodyRows().map((row) => row.textContent)).toEqual([
      'photo 00010.jpg',
      'photo 00012.jpg',
      'photo 00014.jpg',
      'photo 00016.jpg',
      'photo 00018.jpg',
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Show all' }));
    expect(onFilter).toHaveBeenLastCalledWith({});
    expect(await screen.findByText(/Page 1 of 800/)).toBeInTheDocument();
    expect(screen.getByLabelText('Name has')).toHaveValue('');
  });

  it('shows the empty text when nothing is left', async () => {
    render(<Files />);
    await screen.findByText(/Page 1 of 800/);
    fireEvent.change(screen.getByLabelText('Name has'), { target: { value: 'nothing like this' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByText('No file has that name.')).toBeInTheDocument();
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
  });
});
