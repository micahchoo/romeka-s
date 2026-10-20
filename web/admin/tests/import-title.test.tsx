/**
 * The import's title names its file sources (`pages/import/words.ts`): a
 * staging source the last look found only lists in is no files source, so a
 * folder draft that received an uploaded list keeps the folder's name alone.
 */
import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const SOURCES = [
  { id: 's1', kind: 'folder', root: 'archive', path: '' },
  { id: 'up', kind: 'staging' },
];

/** The Simple server, whose staging source the last look found `files` and `lists` in. */
const withStaging = (files: number, lists: readonly string[]): void => {
  installFetch(simpleServer(scenario('meera', 9), { sources: SOURCES }, { files, sheetFiles: lists }).handler);
  render(<ImportPage param="1" rest={['1', 'simple', 'files']} />);
};

afterEach(() => {
  window.location.hash = '';
});

describe('the import’s title', () => {
  it('leaves out a staging source that holds only an uploaded list', async () => {
    withStaging(0, ['lists/1.csv']);
    expect(await screen.findByRole('heading', { level: 2, name: 'All of archive' }, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { level: 2, name: /Files from this computer/ })).toBeNull();
  }, 20_000);

  it('names the staging source when it holds files', async () => {
    withStaging(3, ['lists/1.csv']);
    expect(await screen.findByRole('heading', { level: 2, name: 'All of archive, Files from this computer' }, { timeout: 5000 })).toBeInTheDocument();
  }, 20_000);
});
