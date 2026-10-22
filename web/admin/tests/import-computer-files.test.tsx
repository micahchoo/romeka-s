/**
 * Files from this computer on the files screens of both modes
 * (`pages/import/upload/computer-files.tsx`): the first file that arrives
 * adds a staging source to the plan, and each arrival looks at the files
 * again once that plan is saved, so the counts on screen include it.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import { PATH, simpleServer } from './import-simple-server';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';
import type { Handler } from './stub';

const UPLOADS = `${PATH}/uploads`;

/** The Simple server, with a one-upload tus area and a scan route. */
const withUploads = (inner: Handler): { readonly handler: Handler; readonly calls: string[] } => {
  const calls: string[] = [];
  let offset = 0;
  const handler: Handler = (url, init) => {
    const method = init?.method ?? 'GET';
    calls.push(`${method} ${url.pathname}`);
    if (url.pathname === UPLOADS && method === 'GET') {
      return jsonResponse({ uploads: [], used: 0, limit: { file: 2 * 1024 ** 3, total: 50 * 1024 ** 3 } });
    }
    if (url.pathname === UPLOADS && method === 'POST') {
      offset = 0;
      return new Response(null, { status: 201, headers: { Location: `${UPLOADS}/u1` } });
    }
    if (url.pathname === `${UPLOADS}/u1`) {
      if (method === 'PATCH') {
        offset += init?.body instanceof Blob ? init.body.size : 0;
      }
      return new Response(null, { status: method === 'HEAD' ? 200 : 204, headers: { 'Upload-Offset': String(offset), 'Upload-Length': '4' } });
    }
    if (url.pathname === `${PATH}/scan`) {
      return jsonResponse(loadFixture('import/scan.json'));
    }
    return inner(url, init);
  };
  return { handler, calls };
};

const choose = (files: readonly File[]): void => {
  const input = document.querySelector('input[type="file"][multiple]');
  if (input === null) {
    throw new Error('no file input');
  }
  fireEvent.change(input, { target: { files } });
};

const sourcesOf = (plan: Record<string, unknown> | undefined): unknown => (plan === undefined ? undefined : plan['sources']);

afterEach(() => {
  window.location.hash = '';
});

/** Simple's Files screen, and Advanced's Structure at the archive's node, which holds the sources. */
describe.each([
  ['simple', 'files', null],
  ['advanced', 'structure', '{"node":"archive"}'],
] as const)('files from this computer in %s', (mode, screenId, node) => {
  it('adds a staging source with the first file, then looks at the files again and counts again', async () => {
    const server = simpleServer(scenario('meera', 9));
    const { handler, calls } = withUploads(server.handler);
    installFetch(handler);
    window.location.hash = `#/import/1/${mode}/${screenId}${node === null ? '' : `/${node}`}`;
    render(<ImportPage param="1" rest={['1', mode, screenId]} />);
    expect(await screen.findByRole('button', { name: 'Choose files' }, { timeout: 5000 })).toBeInTheDocument();
    expect(calls).not.toContain(`POST ${PATH}/scan`);

    choose([new File(['abcd'], 'a_001.tif')]);
    await waitFor(() => {
      expect(sourcesOf(server.saved.at(-1))).toEqual([
        { id: 's1', kind: 'folder', root: 'archive', path: '' },
        { id: 's2', kind: 'staging' },
      ]);
    }, { timeout: 5000 });
    await waitFor(() => {
      expect(calls.filter((call) => call === `POST ${PATH}/scan`)).toHaveLength(1);
    }, { timeout: 5000 });
    const scanAt = calls.indexOf(`POST ${PATH}/scan`);
    await waitFor(() => {
      expect(calls.slice(scanAt + 1)).toContain(`POST ${PATH}/preview`);
    });
    if (mode === 'simple') {
      expect(await screen.findByText(/Files from this computer$/, { selector: 'b' })).toBeInTheDocument();
    }
  });
});

describe('a second file', () => {
  it('looks again without adding another source, and the counts are read again', async () => {
    const server = simpleServer(scenario('meera', 9));
    const { handler, calls } = withUploads(server.handler);
    installFetch(handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'files']} />);
    await screen.findByRole('button', { name: 'Choose files' }, { timeout: 5000 });
    choose([new File(['abcd'], 'a_001.tif')]);
    await waitFor(() => {
      expect(calls.filter((call) => call === `POST ${PATH}/scan`)).toHaveLength(1);
    }, { timeout: 5000 });
    const saves = server.saved.length;
    choose([new File(['efgh'], 'a_002.tif')]);
    await waitFor(() => {
      expect(calls.filter((call) => call === `POST ${PATH}/scan`)).toHaveLength(2);
    }, { timeout: 5000 });
    const second = calls.lastIndexOf(`POST ${PATH}/scan`);
    await waitFor(() => {
      expect(calls.slice(second + 1)).toContain(`POST ${PATH}/preview`);
    });
    expect(server.saved.length).toBe(saves);
    expect(asRecord(server.saved.at(-1), [])['sources']).toHaveLength(2);
  });
});

