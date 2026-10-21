/**
 * The upload view (`pages/import/upload/upload-files.tsx`) as a keyboard and
 * screen reader user meets it: real buttons for every action, labels that
 * name the file, progress bars, a polite announcement per file, and the
 * refusals in plain words.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { importTarget } from '../src/api/import-uploads';
import { brandId } from '../src/api/parse';
import type { UploadTarget } from '../src/app/upload/target';
import { UploadFiles } from '../src/pages/import/upload/upload-files';

const ID = brandId<'ImportId'>(2);
const BASE = '/api/romeka/imports/2/uploads';
const resolved = (): Promise<void> => Promise.resolve();

interface Options {
  readonly refuseCreate?: Record<string, unknown> & { readonly status: number };
  readonly holdPatch?: Promise<void>;
}

/** A one-upload-at-a-time tus server: enough for what the view shows. */
const serverApi = (options: Options = {}): { readonly api: UploadTarget; readonly methods: string[] } => {
  const methods: string[] = [];
  let offset = 0;
  const fetchFn: typeof fetch = async (input, init) => {
    const method = init?.method ?? 'GET';
    methods.push(method);
    const path = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
    if (method === 'GET' && path === BASE) {
      return new Response(JSON.stringify({ uploads: [], used: 0, limit: { file: 2 * 1024 ** 3, total: 50 * 1024 ** 3 } }));
    }
    if (method === 'POST') {
      if (options.refuseCreate !== undefined) {
        const { status, ...members } = options.refuseCreate;
        return new Response(JSON.stringify({ errors: { error: 'refused' }, ...members }), { status });
      }
      offset = 0;
      return new Response(null, { status: 201, headers: { Location: `${BASE}/u1` } });
    }
    if (method === 'HEAD') {
      return new Response(null, { headers: { 'Upload-Offset': String(offset), 'Upload-Length': '4' } });
    }
    if (method === 'PATCH') {
      if (options.holdPatch !== undefined) {
        await Promise.race([
          options.holdPatch,
          new Promise((_, reject) => init?.signal?.addEventListener('abort', () => { reject(new DOMException('aborted', 'AbortError')); })),
        ]);
      }
      offset += init?.body instanceof Blob ? init.body.size : 0;
      return new Response(null, { status: 204, headers: { 'Upload-Offset': String(offset) } });
    }
    return new Response(null, { status: 204 });
  };
  return { api: importTarget(ID, { baseUrl: '', fetchFn }), methods };
};

const choose = (container: HTMLElement, files: readonly File[]): void => {
  const input = container.querySelector('input[type="file"][multiple]');
  if (input === null) {
    throw new Error('no file input');
  }
  fireEvent.change(input, { target: { files } });
};

describe('the upload view', () => {
  it('offers both pickers as buttons, the folder picker as a folder picker, and sends what was chosen', async () => {
    const { api } = serverApi();
    const changed: string[] = [];
    const { container } = render(<UploadFiles importId={ID} api={api} wait={resolved} onFilesChanged={() => changed.push('changed')} />);
    expect(await screen.findByRole('button', { name: 'Choose files' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Choose a folder' })).toBeInTheDocument();
    const folderInput = container.querySelector('input[type="file"]:not([multiple])');
    expect(folderInput instanceof HTMLInputElement ? folderInput.webkitdirectory : null).toBe(true);
    expect(await screen.findByText('One file can be up to 2.1 GB. All the files of this import together can be up to 53.7 GB. In use now: 0 bytes.')).toBeInTheDocument();

    choose(container, [new File(['abcd'], 'a.tif'), new File(['abcd'], '.DS_Store')]);
    expect(await screen.findByRole('status')).toHaveTextContent('All files are sent.');
    expect(screen.getByRole('progressbar', { name: 'All files' })).toHaveAttribute('value', '4');
    expect(screen.getByText('4 bytes of 4 bytes sent. Files finished: 1 of 1.')).toBeInTheDocument();
    expect(screen.getByText('Left out, because the name starts with a dot: 1.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove a.tif' })).toBeInTheDocument();
    expect(changed).toEqual(['changed']);
  });

  it('pauses and resumes a file with buttons that name it', async () => {
    let release = (): void => undefined;
    const holdPatch = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { api, methods } = serverApi({ holdPatch });
    const { container } = render(<UploadFiles importId={ID} api={api} wait={resolved} />);
    await screen.findByText(/One file can be up to/);
    choose(container, [new File(['abcd'], 'a.tif')]);
    fireEvent.click(await screen.findByRole('button', { name: 'Pause a.tif' }));
    expect(screen.getByText(/^Paused/)).toBeInTheDocument();
    release();
    fireEvent.click(screen.getByRole('button', { name: 'Resume a.tif' }));
    expect(await screen.findByRole('status')).toHaveTextContent('All files are sent.');
    expect(methods).toEqual(['GET', 'POST', 'PATCH', 'HEAD', 'PATCH']);
  });

  it('says in one sentence what to do about a file over the limit, at the file', async () => {
    const { api } = serverApi({ refuseCreate: { status: 413, code: 'fileTooLarge', limit: 1_000_000_000 } });
    const { container } = render(<UploadFiles importId={ID} api={api} wait={resolved} />);
    await screen.findByText(/One file can be up to/);
    choose(container, [new File(['abcd'], 'big.tif')]);
    const text = 'This file is larger than 1 GB, the most one file can be. Split it into smaller files, or ask an administrator to raise the limit.';
    expect(await screen.findByText(text)).toBeInTheDocument();
    await waitFor(() => { expect(screen.getByRole('status')).toHaveTextContent(`big.tif was not sent. ${text}`); });
    expect(screen.queryByRole('button', { name: 'Try big.tif again' })).toBeNull();
  });

  it('replaces the drop zone with the reason once the import has started', async () => {
    const { api } = serverApi({ refuseCreate: { status: 409, code: 'notDraft' } });
    const { container } = render(<UploadFiles importId={ID} api={api} wait={resolved} />);
    await screen.findByText(/One file can be up to/);
    choose(container, [new File(['abcd'], 'a.tif')]);
    expect(await screen.findByRole('alert')).toHaveTextContent('This import has started, so no more files can be added. Make a copy of it to add files.');
    expect(screen.queryByRole('button', { name: 'Choose files' })).toBeNull();
  });
});
