/**
 * The review lists of one import, Every item and Every file, at 9 and at
 * 40,000 items: one page of at most 50 rows, a filter the server is asked
 * for, and "change all N at once" saving one `many` decision that names every
 * entry the filter selects, read page by page.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { countOf, hasFile, listsServer, pad, PATH } from './import-lists-server';
import { installFetch, requestBody, requestsFor } from './stub';

const at = (...rest: readonly string[]) => <ImportPage param="1" rest={['1', ...rest]} />;

const bodyRows = (name: string): readonly HTMLElement[] =>
  within(screen.getByRole('region', { name })).getAllByRole('row').slice(1);

const filterSegment = (filter: Record<string, string>): string => encodeURIComponent(JSON.stringify(filter));

const lastDecisions = (saved: readonly Record<string, unknown>[]): unknown => saved[saved.length - 1]?.['decisions'];

/** The one decision the last save holds, which must be a `many`. */
const onlyMany = (saved: readonly Record<string, unknown>[]): { readonly change: unknown; readonly entries: readonly unknown[] } => {
  const decisions = lastDecisions(saved);
  expect(Array.isArray(decisions) ? decisions.length : 0).toBe(1);
  const many = asRecord(Array.isArray(decisions) ? decisions[0] : null, []);
  expect(many['kind']).toBe('many');
  const entries = many['entries'];
  return { change: many['change'], entries: Array.isArray(entries) ? entries : [] };
};

afterEach(() => {
  window.location.hash = '';
});

describe.each([9, 40_000])('Every item at %i items', (size) => {
  it('draws one page, sends a filter, and changes what the filter selects as one many decision', async () => {
    const server = listsServer(size);
    const fetchMock = installFetch(server.handler);
    render(at('simple', 'all-items'));
    expect(await screen.findByText(size === 1 ? '1 item' : `${size.toLocaleString('en')} items`)).toBeInTheDocument();
    expect(bodyRows('Every item')).toHaveLength(Math.min(size, 50));
    expect(bodyRows('Every item')[0]).toHaveTextContent(`item_${pad(1)}Letter 1`);
    expect(screen.queryByRole('group', { name: /Change all/ })).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Files'), { target: { value: '1' } });
    const withFile = countOf(size, hasFile);
    expect(await screen.findByText(`${withFile.toLocaleString('en')} items`)).toBeInTheDocument();
    const asked = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).filter((body) => JSON.stringify(body['include']) === '["records"]');
    expect(asked[asked.length - 1]?.['records']).toEqual({ kind: 'item', files: '1', page: 1, perPage: 50 });

    const bar = screen.getByRole('group', { name: `Change all ${withFile.toLocaleString('en')} at once:` });
    fireEvent.click(within(bar).getByRole('button', { name: 'Leave them out' }));
    const names = await screen.findByText(/^This changes/, {}, { timeout: 10_000 });
    expect(names).toHaveTextContent(`This changes ${withFile.toLocaleString('en')}: item_00001, item_00002, item_00004, item_00005, item_00006`);
    fireEvent.click(screen.getByRole('button', { name: `Change these ${withFile.toLocaleString('en')}` }));
    expect(await screen.findByText(/^Done: Leave them out/)).toBeInTheDocument();

    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    }, { timeout: 3000 });
    const many = onlyMany(server.saved);
    expect(many.change).toEqual({ kind: 'leave' });
    expect(many.entries).toHaveLength(withFile);
    expect(many.entries[0]).toEqual({ file: { source: 's1', path: `Letters/item_${pad(1)}.jpg` } });
    expect(screen.getByText(/^You made 1 change\. 1 of them change many\./)).toBeInTheDocument();
    expect(document.querySelectorAll('body *').length).toBeLessThan(1500);
  }, 30_000);
});

describe.each([9, 40_000])('Every file at %i items', (size) => {
  it('draws one page, filters to files not in an item, and moves them all at once', async () => {
    const server = listsServer(size);
    const fetchMock = installFetch(server.handler);
    render(at('advanced', 'all-files'));
    const files = countOf(size, hasFile);
    expect(await screen.findByText(`${files.toLocaleString('en')} files`)).toBeInTheDocument();
    const first = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).find((body) => body['entries'] !== undefined);
    expect(first?.['entries']).toEqual({ kind: 'file', page: 1, perPage: 50 });
    expect(bodyRows('Every file')).toHaveLength(Math.min(files, 50));

    fireEvent.change(screen.getByLabelText('In an item'), { target: { value: 'no' } });
    const out = countOf(size, (n) => hasFile(n) && n % 7 === 0);
    expect(await screen.findByText(out === 1 ? '1 file' : `${out.toLocaleString('en')} files`)).toBeInTheDocument();
    expect(bodyRows('Every file')[0]).toHaveTextContent('You left it out.');

    fireEvent.click(screen.getByRole('button', { name: 'Make each one an item of its own' }));
    fireEvent.click(await screen.findByRole('button', { name: `Change these ${out.toLocaleString('en')}` }, { timeout: 10_000 }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    }, { timeout: 3000 });
    const many = onlyMany(server.saved);
    expect(many.change).toEqual({ kind: 'move', to: 'own' });
    expect(many.entries).toHaveLength(out);
    expect(many.entries[0]).toEqual({ file: { source: 's1', path: `Letters/item_${pad(7)}.jpg` } });
  }, 30_000);

  it('changes one file with one move decision', async () => {
    const server = listsServer(size);
    installFetch(server.handler);
    render(at('simple', 'all-files'));
    fireEvent.change(await screen.findByLabelText(`Change item_${pad(1)}.jpg`), { target: { value: 'leave' } });
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    }, { timeout: 3000 });
    expect(lastDecisions(server.saved)).toEqual([
      { kind: 'move', file: { file: { source: 's1', path: `Letters/item_${pad(1)}.jpg` } }, to: 'leave' },
    ]);
  });
});

describe('an address with a filter', () => {
  it('opens the items list with the filter an address names', async () => {
    const server = listsServer(40_000);
    const fetchMock = installFetch(server.handler);
    window.location.hash = `#/import/1/simple/all-items/${filterSegment({ files: '0' })}`;
    render(<ImportPage param="1" rest={['1', 'simple', 'all-items', filterSegment({ files: '0' })]} />);
    expect(await screen.findByRole('combobox', { name: 'Files' })).toHaveValue('0');
    await waitFor(() => {
      const asked = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).filter((body) => JSON.stringify(body['include']) === '["records"]');
      expect(asked[0]?.['records']).toEqual({ kind: 'item', files: '0', page: 1, perPage: 50 });
    });
  }, 20_000);
});
