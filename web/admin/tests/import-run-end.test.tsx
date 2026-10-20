/**
 * The run screen after the work: a run that ended (done, done with failures,
 * stopped) with its counts, failures, left-out file and retry; undo with the
 * list of what it keeps and the choice to undo those anyway; and the report of
 * an undo that ended (undone, partly undone, stopped).
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ImportPage } from '../src/pages/import/page';
import { checks, conflict, fixture, pathOf, reported, runRoutes } from './import-run-server';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const at = (id: number) => <ImportPage param={String(id)} rest={[String(id), 'simple', 'add']} />;

afterEach(() => {
  vi.unstubAllGlobals();
  window.location.hash = '';
});

describe('a run that ended', () => {
  it('done: the count, the left-out file, undo later, and the log; no retry', async () => {
    installFetch(runRoutes(6, { reads: [fixture('import-done.json')] }));
    render(at(6));
    expect(await screen.findByText('The import is done.')).toBeInTheDocument();
    expect(screen.getByText('9 of 9 done.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^Download the list of what was left out/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo this import...' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Try again/ })).toBeNull();
    expect(screen.getByRole('link', { name: 'See the log on the Jobs page' })).toHaveAttribute('href', '#/jobs/2');
  });

  it('done with failures: each failure with its reason, and a retry that sends a fresh digest', async () => {
    const fetchMock = installFetch(
      runRoutes(6, { reads: [fixture('import-done-failures.json'), fixture('import-running.json')] }),
    );
    render(at(6));
    expect(await screen.findByText('1 failed:')).toBeInTheDocument();
    const failures = await screen.findByRole('region', { name: 'failed' });
    expect(within(failures).getByText('group:BHC_006_AF_003')).toBeInTheDocument();
    expect(within(failures).getByText(/could not be read: permission denied/)).toBeInTheDocument();
    const retry = await screen.findByRole('button', { name: 'Try again: the 1 not done yet' });
    await waitFor(() => {
      expect(retry).toBeEnabled();
    });
    fireEvent.click(retry);
    expect(await screen.findByText('Writing to Omeka...')).toBeInTheDocument();
    expect(checks(requestsFor(fetchMock, `${pathOf(6)}/preview`))).toHaveLength(1);
    expect(requestsFor(fetchMock, `${pathOf(6)}/run`)).toHaveLength(1);
  });

  it('run_failed: stopped before the end because someone asked, with the failure and a retry', async () => {
    installFetch(runRoutes(1, { reads: [fixture('import-run-failed.json')] }));
    render(at(1));
    expect(await screen.findByText('The import stopped before the end.')).toBeInTheDocument();
    expect(screen.getByText('8 of 9 done.')).toBeInTheDocument();
    expect(screen.getByText('It stopped because someone asked it to.')).toBeInTheDocument();
    expect(await screen.findByText('The file could not be read.')).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Try again: the 1 not done yet' })).toBeInTheDocument();
  });

  it('downloads the stored left-out file', async () => {
    const created = vi.fn(() => 'blob:left');
    vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: created, revokeObjectURL: vi.fn() }));
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);
    const fetchMock = installFetch(runRoutes(6, { reads: [fixture('import-done.json')] }));
    render(at(6));
    fireEvent.click(await screen.findByRole('button', { name: /^Download the list of what was left out/ }));
    await waitFor(() => {
      expect(click).toHaveBeenCalledTimes(1);
    });
    expect(requestsFor(fetchMock, `${pathOf(6)}/left.csv`)).toHaveLength(1);
    expect(created).toHaveBeenCalledTimes(1);
    click.mockRestore();
  });
});

describe('undo', () => {
  const opened = async (id: number, read: string) => {
    const fetchMock = installFetch(runRoutes(id, { reads: [fixture(read), fixture('import-undoing.json')] }));
    render(at(id));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo this import...' }));
    return fetchMock;
  };

  it('says what it removes, and lists what it keeps with what changed each', async () => {
    await opened(6, 'import-done.json');
    expect(await screen.findByText('Undo removes 8 items, 13 files, and 2 item sets that this import added.')).toBeInTheDocument();
    expect(screen.getByText(/^4 are kept: since the import, something in Omeka changed them or uses them\./)).toBeInTheDocument();
    const kept = await screen.findByRole('region', { name: 'kept' });
    expect(within(kept).getByRole('link', { name: 'Item 4' })).toHaveAttribute('href', '#/items/4');
    expect(within(kept).getByText('This was edited after the import wrote it.')).toBeInTheDocument();
    expect(within(kept).getByRole('link', { name: 'Item set 1' })).toHaveAttribute('href', '#/item-sets/1');
  });

  it('undoes one kept item anyway, and says what that loses before the press', async () => {
    const fetchMock = await opened(6, 'import-done.json');
    fireEvent.click(await screen.findByRole('checkbox', { name: 'Undo it anyway: Item 4' }));
    expect(screen.getByText('Undoing 1 anyway also undoes what was done to it in Omeka after the import.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Undo the import' }));
    expect(await screen.findByText('Undoing...')).toBeInTheDocument();
    const [sent] = requestsFor(fetchMock, `${pathOf(6)}/undo`);
    expect(sent === undefined ? null : requestBody(sent)).toEqual({ force: [{ kind: 'item', id: 4 }] });
  });

  it('undoes all kept anyway', async () => {
    const fetchMock = await opened(6, 'import-done.json');
    fireEvent.click(await screen.findByRole('checkbox', { name: 'Undo all 4 anyway' }));
    expect(screen.getByText('Undoing 4 anyway also undoes what was done to them in Omeka after the import.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Undo the import' }));
    await screen.findByText('Undoing...');
    const [sent] = requestsFor(fetchMock, `${pathOf(6)}/undo`);
    expect(sent === undefined ? null : requestBody(sent)).toEqual({ force: 'all' });
  });

  it('undoes with nothing forced when nothing is ticked', async () => {
    const fetchMock = await opened(6, 'import-done.json');
    fireEvent.click(await screen.findByRole('button', { name: 'Undo the import' }));
    await screen.findByText('Undoing...');
    const [sent] = requestsFor(fetchMock, `${pathOf(6)}/undo`);
    expect(sent === undefined ? null : requestBody(sent)).toEqual({});
  });

  it('says that undoing an item type anyway also takes its values from items outside the import', async () => {
    const preview = fixture('undo-preview-kept.json');
    const type = { kind: 'resourceTemplate', id: 3, reasons: [{ reason: 'inUse', key: 'server.importUndoInUse', detail: 'Items, media, item sets or item types outside this import use this.' }] };
    installFetch(
      runRoutes(6, {
        reads: [fixture('import-done.json')],
        undoPreview: () => jsonResponse({ ...preview, kept: [type] }),
      }),
    );
    render(at(6));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo this import...' }));
    fireEvent.click(await screen.findByRole('checkbox', { name: 'Undo it anyway: Item type 3' }));
    expect(screen.getByText(/also loses its values on items outside this import/)).toBeInTheDocument();
  });

  it('a preview the server refuses (cannotUndo) says so in plain words', async () => {
    installFetch(
      runRoutes(6, { reads: [fixture('import-done.json')], undoPreview: () => conflict('conflict-cannot-undo.json') }),
    );
    render(at(6));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo this import...' }));
    expect(await screen.findByText(/This import cannot be undone now\. It is: Not added yet\./)).toBeInTheDocument();
  });

  it('an undo refused as busy names the other import', async () => {
    installFetch(
      runRoutes(6, { reads: [fixture('import-done.json')], undo: () => conflict('conflict-busy.json', { importId: 2 }) }),
    );
    render(at(6));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo this import...' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Undo the import' }));
    expect(await screen.findByRole('link', { name: 'Open that import' })).toHaveAttribute('href', '#/import/2');
  });
});

describe('an undo that ended', () => {
  it('partly undone: what it removed, how many it kept, and the kept list to undo the rest', async () => {
    installFetch(runRoutes(1, { reads: [reported('undo-report-kept.json')] }));
    render(at(1));
    expect(await screen.findByText('Partly undone.')).toBeInTheDocument();
    expect(screen.getByText(/Removed 8 items, 13 files, and 2 item sets\./)).toBeInTheDocument();
    expect(screen.getByText('4 were kept: since the import, something in Omeka changed them or uses them.')).toBeInTheDocument();
    expect(await screen.findByRole('checkbox', { name: 'Undo it anyway: Item 4' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Cancel' })).toBeNull();
  });

  it('undone: what it undid anyway, and no undo left to offer', async () => {
    installFetch(runRoutes(1, { reads: [reported('undo-report-forced.json')] }));
    render(at(1));
    expect(await screen.findByText('Undone.')).toBeInTheDocument();
    expect(screen.getByText('1 was undone anyway, although something in Omeka changed it or used it after the import.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Undo the rest' })).toBeNull();
  });

  it('undone with some gone already: says how many', async () => {
    installFetch(runRoutes(1, { reads: [reported('undo-report-skipped.json')] }));
    render(at(1));
    expect(await screen.findByText('3 were not in Omeka any more.')).toBeInTheDocument();
  });

  it('undo stopped on request: how far it got, and undo the rest', async () => {
    installFetch(runRoutes(1, { reads: [reported('undo-report-stopped.json')] }));
    render(at(1));
    expect(await screen.findByText('The undo stopped before the end.')).toBeInTheDocument();
    expect(screen.getByText('It stopped because someone asked it to, after 3 of 27.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo the rest' })).toBeInTheDocument();
  });

  it('undo stopped by an error: the reason the server gave', async () => {
    installFetch(runRoutes(1, { reads: [reported('undo-report-error.json')] }));
    render(at(1));
    expect(await screen.findByText('It stopped after 9 of 27: The undo stopped before the end: the worker died')).toBeInTheDocument();
  });
});
