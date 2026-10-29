/**
 * The Jobs pages in plain words: every job class has a name in the list and
 * the detail, the detail keeps the stored class as secondary text, names the
 * owner as the list does, and folds the arguments.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { JobsPage } from '../src/pages/jobs/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, route } from './stub';

const firstJob = (): Record<string, unknown> => {
  const page = loadFixture('jobs-page.json');
  const first: unknown = Array.isArray(page) ? page[0] : null;
  if (typeof first !== 'object' || first === null) {
    throw new Error('expected a job list fixture');
  }
  return { ...first };
};

const firstUser = (): unknown => {
  const page = loadFixture('users-page.json');
  return Array.isArray(page) ? page[0] : null;
};

const emptyLog = (): Response => new Response('', { headers: { 'Content-Type': 'text/plain' } });

const CLASSES: readonly (readonly [string, string])[] = [
  ['Omeka\\Job\\BatchDelete', 'Batch delete'],
  ['Omeka\\Job\\BatchUpdate', 'Batch edit'],
  ['Omeka\\Job\\IndexFulltextSearch', 'Search index rebuild'],
  ['Omeka\\Job\\UpdateSiteItems', 'Site items update'],
  ['Romeka\\Job\\BulkRun', 'Bulk change'],
  ['Romeka\\Job\\BulkUndo', 'Undo of a bulk change'],
  ['Romeka\\Job\\ImportScan', 'Look at the files of an import'],
  ['Romeka\\Job\\ImportUnzip', 'Unpack an uploaded ZIP'],
  ['Romeka\\Job\\ImportSweep', 'Import sweep'],
  ['Romeka\\Job\\MediaDerivatives', 'Previews and playable copies'],
  ['Romeka\\Job\\FixityCheck', 'Check stored files'],
];

describe('job names', () => {
  it.each(CLASSES)('the detail names %s as "%s" and keeps the class as secondary text', async (jobClass, name) => {
    installFetch(
      route({
        '/api/jobs/1': () => jsonResponse({ ...firstJob(), 'o:job_class': jobClass, 'o:args': {} }),
        '/api/romeka/jobs/1/log': emptyLog,
        '/api/users/3': () => jsonResponse(firstUser()),
      }),
    );
    render(<JobsPage param="1" />);

    expect(await screen.findByRole('heading', { name: name })).toBeInTheDocument();
    expect(screen.getByText(jobClass)).toBeInTheDocument();
  });

  it('the list names every row by its job, never by the stored class', async () => {
    installFetch(
      route({
        '/api/jobs': () =>
          jsonResponse(CLASSES.map(([jobClass], index) => ({ ...firstJob(), '@id': `http://localhost/api/jobs/${String(index + 1)}`, 'o:id': index + 1, 'o:job_class': jobClass }))),
        '/api/users': () => jsonResponse(loadFixture('users-page.json')),
      }),
    );
    render(<JobsPage />);

    const table = await screen.findByRole('table');
    for (const [jobClass, name] of CLASSES) {
      expect(within(table).getByText(name)).toBeInTheDocument();
      expect(within(table).queryByText(jobClass)).toBeNull();
    }
  });
});

describe('job detail', () => {
  it('names the owner as the list does, linked to the user', async () => {
    installFetch(
      route({
        '/api/jobs/1': () => jsonResponse(firstJob()),
        '/api/romeka/jobs/1/log': emptyLog,
        '/api/users/3': () => jsonResponse(firstUser()),
      }),
    );
    render(<JobsPage param="1" />);

    expect(await screen.findByRole('link', { name: 'Ada Lovelace' })).toHaveAttribute('href', '#/users/3');
    expect(screen.queryByText('#3')).toBeNull();
  });

  it('folds the arguments until they are opened', async () => {
    installFetch(
      route({
        '/api/jobs/1': () => jsonResponse(firstJob()),
        '/api/romeka/jobs/1/log': emptyLog,
        '/api/users/3': () => jsonResponse(firstUser()),
      }),
    );
    render(<JobsPage param="1" />);

    const summary = await screen.findByText('Arguments');
    const fold = summary.closest('details');
    expect(fold).not.toBeNull();
    expect(fold?.open).toBe(false);
    expect(within(fold ?? document.body).getByText(/"action": "add"/)).toBeInTheDocument();
  });
});
