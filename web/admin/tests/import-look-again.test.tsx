/**
 * "Look at the files again" on a large folder (`pages/import/look-again.ts`):
 * the route answers a scan job, and the screen reads the job until it ends,
 * then reads the import and the counts again.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import { PATH, simpleServer } from './import-simple-server';
import { installFetch, jsonResponse } from './stub';

const JOB = '/api/jobs/7';

afterEach(() => {
  window.location.hash = '';
});

describe('a look that runs as a job', () => {
  it('says it takes a while, reads the job until it ends, then reads the import and the counts again', async () => {
    const meera = scenario('meera', 9);
    const server = simpleServer(meera);
    const calls: string[] = [];
    let jobReads = 0;
    installFetch((url, init) => {
      calls.push(`${init?.method ?? 'GET'} ${url.pathname}`);
      if (url.pathname === `${PATH}/scan`) {
        return jsonResponse({ jobId: 7 }, { status: 202 });
      }
      if (url.pathname === JOB) {
        jobReads += 1;
        return jsonResponse({ 'o:id': 7, 'o:status': jobReads < 2 ? 'in_progress' : 'completed' });
      }
      return server.handler(url, init);
    });
    render(<ImportPage param="1" rest={['1', 'simple', 'files']} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Look at the files again' }, { timeout: 5000 }));

    expect(await screen.findByText(/^We are looking again\./)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Looking...' })).toBeDisabled();
    expect(await screen.findByText(`We looked again: ${String(meera.files)} files.`, {}, { timeout: 10_000 })).toBeInTheDocument();

    expect(jobReads).toBe(2);
    const ended = calls.lastIndexOf(`GET ${JOB}`);
    expect(calls.slice(ended).filter((call) => call === `GET ${PATH}`)).toHaveLength(1);
    expect(calls.slice(ended).filter((call) => call === `POST ${PATH}/preview`).length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Look at the files again' })).toBeEnabled();
  }, 20_000);
});
