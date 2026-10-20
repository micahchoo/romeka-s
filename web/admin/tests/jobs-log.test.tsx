/**
 * The Jobs detail page with its log (`GET /api/romeka/jobs/{id}/log`): read
 * every 2 seconds while the job runs, and not again once it ends. Romeka's
 * import job classes read as names.
 */
import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { JobsPage } from '../src/pages/jobs/page';
import { JOB_POLL_MS } from '../src/pages/jobs/use-job-poll';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestsFor, route } from './stub';

const LOG = '/api/romeka/jobs/5/log';

const job = (status: string): Record<string, unknown> => {
  const page = loadFixture('jobs-page.json');
  const first: unknown = Array.isArray(page) ? page[0] : null;
  if (typeof first !== 'object' || first === null) {
    throw new Error('expected a job list fixture');
  }
  return {
    ...first,
    '@id': 'http://localhost/api/jobs/5',
    'o:id': 5,
    'o:status': status,
    'o:job_class': 'Romeka\\Job\\ImportArchive',
    'o:args': { importId: 7 },
  };
};

const tick = async (ms: number): Promise<void> => {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
};

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
});

describe('Job detail log', () => {
  it('reads the job and its log every 2 seconds while it runs and stops when it ends', async () => {
    const statuses = ['in_progress', 'in_progress', 'completed'];
    const logs = ['row 1 imported', 'row 1 imported\nrow 2 imported', 'row 1 imported\nrow 2 imported\ndone'];
    let reads = 0;
    const fetchMock = installFetch(
      route({
        '/api/jobs/5': () => {
          const answer = job(statuses[Math.min(reads, statuses.length - 1)] ?? 'completed');
          return jsonResponse(answer);
        },
        [LOG]: () => {
          const text = logs[Math.min(reads, logs.length - 1)] ?? '';
          reads += 1;
          return new Response(text, { headers: { 'Content-Type': 'text/plain' } });
        },
      }),
    );
    render(<JobsPage param="5" />);

    expect(await screen.findByText('row 1 imported')).toBeInTheDocument();
    expect(screen.getByText('Archive import')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open the import' })).toHaveAttribute('href', '#/import/7/run');
    expect(screen.getByText('This page refreshes every 2 seconds while the job runs.')).toBeInTheDocument();

    await tick(JOB_POLL_MS - 100);
    expect(requestsFor(fetchMock, LOG)).toHaveLength(1);
    await tick(100);
    expect(await screen.findByText(/row 2 imported$/)).toBeInTheDocument();
    await tick(JOB_POLL_MS);
    expect(await screen.findByText(/done$/)).toBeInTheDocument();
    expect(screen.queryByText('This page refreshes every 2 seconds while the job runs.')).toBeNull();

    await tick(JOB_POLL_MS * 3);
    expect(requestsFor(fetchMock, LOG)).toHaveLength(3);
    expect(requestsFor(fetchMock, '/api/jobs/5')).toHaveLength(3);
  });

  it('stops reading when the page closes', async () => {
    const fetchMock = installFetch(
      route({
        '/api/jobs/5': () => jsonResponse(job('starting')),
        [LOG]: () => new Response('', { headers: { 'Content-Type': 'text/plain' } }),
      }),
    );
    const view = render(<JobsPage param="5" />);
    expect(await screen.findByText('The job has written no log yet.')).toBeInTheDocument();
    view.unmount();
    await tick(JOB_POLL_MS * 3);
    expect(requestsFor(fetchMock, LOG)).toHaveLength(1);
  });
});
