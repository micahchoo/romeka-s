/**
 * The run screen before and while it works (`#/import/<id>/<mode>/add|run`):
 * the counts the check made, the start that sends their digest back, each
 * refusal in plain words with its next step, and a run or an undo in
 * progress with the time left and Stop.
 */
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { POLL_MS } from '../src/pages/import/run/use-live-import';
import { checks, conflict, fixture, pathOf, runRoutes } from './import-run-server';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const at = (id: number, mode = 'simple', screenId = 'add') => (
  <ImportPage param={String(id)} rest={[String(id), mode, screenId]} />
);

const DIGEST = asRecord(fixture('preview-check.json')['check'], [])['digest'];

afterEach(() => {
  vi.useRealTimers();
  window.location.hash = '';
});

const ready = async (): Promise<HTMLElement> => screen.findByRole('button', { name: 'Add 10 items to Omeka' });

describe('before the start', () => {
  it('says what adding puts into Omeka and what it leaves out, in the check’s counts', async () => {
    installFetch(runRoutes(1, { reads: [fixture('import-draft.json')] }));
    render(at(1));
    expect(await ready()).toBeEnabled();
    expect(screen.getByText('Adding puts 10 items and 19 files into Omeka.')).toBeInTheDocument();
    expect(screen.getByText('Left out: 1 item and 3 files.')).toBeInTheDocument();
    expect(screen.getByText('You can undo the whole import later.')).toBeInTheDocument();
  });

  it('starts with the digest the counts came with, then shows the run working', async () => {
    const fetchMock = installFetch(
      runRoutes(1, { reads: [fixture('import-draft.json'), fixture('import-running.json')] }),
    );
    render(at(1));
    fireEvent.click(await ready());
    expect(await screen.findByText('Writing to Omeka...')).toBeInTheDocument();
    const [sent] = requestsFor(fetchMock, `${pathOf(1)}/run`);
    expect(sent === undefined ? null : requestBody(sent)).toEqual({ digest: DIGEST });
  });

  it('shows the counts by record kind and a Run button in Advanced', async () => {
    installFetch(runRoutes(1, { reads: [fixture('import-draft.json')] }));
    render(at(1, 'advanced', 'run'));
    expect(await screen.findByRole('button', { name: 'Run the import' })).toBeEnabled();
    expect(screen.getByRole('table', { name: 'What the run writes, by record kind and action.' })).toBeInTheDocument();
    expect(screen.queryByText('Adding puts 10 items and 19 files into Omeka.')).toBeNull();
  });

  it('holds Add while problems stop the start, and links to them', async () => {
    installFetch(runRoutes(1, { reads: [fixture('import-draft.json')], check: 'preview-check-blocked.json' }));
    render(at(1));
    expect(await screen.findByText(/^1 problem must be fixed before you can add\./)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Look at them' })).toHaveAttribute('href', '#/import/1/simple/check');
    expect(screen.getByRole('button', { name: /to Omeka$/ })).toBeDisabled();
  });
});

describe('a start the server refuses', () => {
  const refused = async (answer: () => Response, mode = 'simple') => {
    const fetchMock = installFetch(runRoutes(1, { reads: [fixture('import-draft.json')], run: [answer] }));
    render(at(1, mode, mode === 'simple' ? 'add' : 'run'));
    const add = await screen.findByRole('button', { name: mode === 'simple' ? 'Add 10 items to Omeka' : 'Run the import' });
    fireEvent.click(add);
    return fetchMock;
  };

  it('changed: says so and counts again', async () => {
    const fetchMock = await refused(() => conflict('conflict-changed.json'));
    expect(await screen.findByText(/changed after they were counted/)).toBeInTheDocument();
    await waitFor(() => {
      expect(checks(requestsFor(fetchMock, `${pathOf(1)}/preview`))).toHaveLength(2);
    });
  });

  it('blocked: names how many problems and links to them', async () => {
    await refused(() => conflict('conflict-blocked.json'), 'advanced');
    expect(await screen.findByText('This import cannot start: 1 problem must be fixed first.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Look at them' })).toHaveAttribute('href', '#/import/1/advanced/problems');
  });

  it('confirm: asks for the count typed, and sends it', async () => {
    const fetchMock = installFetch(
      runRoutes(1, {
        reads: [fixture('import-draft.json'), fixture('import-running.json')],
        run: [() => conflict('conflict-confirm.json'), () => jsonResponse(fixture('run.json'))],
      }),
    );
    render(at(1));
    fireEvent.click(await ready());
    expect(await screen.findByText(/A delete needs its count typed: 1 items, files and item sets/)).toBeInTheDocument();
    const go = screen.getByRole('button', { name: 'Add 10 items to Omeka' });
    expect(go).toBeDisabled();
    fireEvent.change(screen.getByLabelText('Type 1 to confirm'), { target: { value: '1' } });
    fireEvent.click(go);
    expect(await screen.findByText('Writing to Omeka...')).toBeInTheDocument();
    const sent = requestsFor(fetchMock, `${pathOf(1)}/run`)[1];
    expect(sent === undefined ? null : requestBody(sent)).toEqual({ digest: DIGEST, confirm: 1 });
  });

  it('busy with another import: links to it and offers to try again', async () => {
    await refused(() => conflict('conflict-busy.json', { importId: 9 }));
    expect(await screen.findByText(/Another import is being added or undone now/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open that import' })).toHaveAttribute('href', '#/import/9');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('busy with this one: says it is still looking at its files', async () => {
    await refused(() => conflict('conflict-busy.json'));
    expect(await screen.findByText('This import is still looking at its files. Try again in a moment.')).toBeInTheDocument();
  });

  it('nothing: says every item is left out and links to the files', async () => {
    await refused(() => conflict('conflict-nothing.json'));
    expect(await screen.findByText('There is nothing to add: every item is left out.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'See what we found' })).toHaveAttribute('href', '#/import/1/simple/found');
  });

  it('cannotRun: names where it stands and reads the import again', async () => {
    const fetchMock = await refused(() => conflict('conflict-cannot-run.json'));
    expect(await screen.findByText(/This import cannot be added now\. It is: Added\./)).toBeInTheDocument();
    await waitFor(() => {
      expect(requestsFor(fetchMock, pathOf(1)).length).toBeGreaterThan(1);
    });
  });
});

describe('while it works', () => {
  it('shows how far it got, the time left, that it goes on without the page, and the log', async () => {
    installFetch(runRoutes(1, { reads: [fixture('import-running.json')] }));
    render(at(1));
    expect(await screen.findByText('3 of 9 done.')).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).toHaveAttribute('value', '3');
    expect(screen.getByText('Less than a minute left at this speed.')).toBeInTheDocument();
    expect(screen.getByText(/You can leave this page\. The import goes on/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'See the log on the Jobs page' })).toHaveAttribute('href', '#/jobs/1');
  });

  it('asks the job to stop, then says it is stopping until it has', async () => {
    const fetchMock = installFetch(
      runRoutes(1, { reads: [fixture('import-running.json'), fixture('import-stopping.json')] }),
    );
    render(at(1));
    fireEvent.click(await screen.findByRole('button', { name: 'Stop after the one in hand' }));
    expect(await screen.findByText('Stopping after the one in hand...')).toBeInTheDocument();
    expect(requestsFor(fetchMock, `${pathOf(1)}/stop`)).toHaveLength(1);
    expect(screen.queryByRole('button', { name: 'Stop after the one in hand' })).toBeNull();
  });

  it('a stop that finds it not running says so and reads it again', async () => {
    installFetch(
      runRoutes(1, {
        reads: [fixture('import-running.json'), fixture('import-run-failed.json')],
        stop: () => conflict('conflict-not-running.json'),
      }),
    );
    render(at(1));
    fireEvent.click(await screen.findByRole('button', { name: 'Stop after the one in hand' }));
    expect(await screen.findByText('The import stopped before the end.')).toBeInTheDocument();
  });

  it('reads the import again every 2 seconds and shows the end when it comes', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    installFetch(runRoutes(6, { reads: [{ ...fixture('import-running.json'), id: 6 }, fixture('import-done.json')] }));
    render(at(6));
    expect(await screen.findByText('3 of 9 done.')).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_MS);
    });
    expect(await screen.findByText('The import is done.')).toBeInTheDocument();
  });

  it('shows an undo working, and its own Stop', async () => {
    installFetch(runRoutes(6, { reads: [fixture('import-undoing.json')] }));
    render(at(6));
    expect(await screen.findByText('Undoing...')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Stop the undo after the one in hand' })).toBeInTheDocument();
    expect(screen.getByText(/The undo goes on/)).toBeInTheDocument();
  });
});
