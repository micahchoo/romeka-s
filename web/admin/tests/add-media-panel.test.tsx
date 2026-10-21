/**
 * The add-media panel through its public surface: pick, paste, the pre-send
 * checks, the row states, Try again, Escape, and the description save. The
 * fetch stub is a small tus server, so the queue runs through the real client.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Item } from '../src/api/types';
import { brandId, isRecord } from '../src/api/parse';
import AddMediaPanel from '../src/pages/items/add-media/panel';
import { forgetAdds } from '../src/pages/items/add-media/store';
import { loadFixture } from './loadFixture';
import type { Mock } from 'vitest';
import { first, jsonResponse, requestBody, requestUrl, requestsFor, route } from './stub';

type Handler = (url: URL, init: RequestInit | undefined) => Response | Promise<Response>;

const serve = (handler: Handler): Mock<typeof fetch> => {
  const fetchMock = vi.fn<typeof fetch>((input, init) => Promise.resolve(handler(requestUrl(input), init)));
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
};

const id = brandId<'ItemId'>(7);

const makeItem = (title = 'Letter to Anna, 1912'): Item => ({
  id,
  apiUrl: '/api/items/7',
  types: ['o:Item'],
  isPublic: true,
  owner: null,
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  title,
  created: '2026-10-21T10:00:00+00:00',
  modified: null,
  thumbnailDisplayUrls: {},
  values: new Map(),
  primaryMedia: null,
  media: [],
  itemSets: [],
  sites: [],
});

const LIMITS = {
  maxFileBytes: 2 * 1024 ** 3,
  kinds: ['local-image', 'audio', 'video', 'pdf', 'document'],
  extensions: ['jpg', 'png', 'mp4', 'pdf'],
  validationOff: false,
};

const fixture = loadFixture('media-951.json');
const createdMedia = (media: Record<string, unknown> = {}): Record<string, unknown> =>
  isRecord(fixture) ? { ...fixture, ...media } : media;

interface TusState {
  readonly uploads: Map<string, { path: string; length: number; offset: number }>;
  next: number;
  /** Answers to the next media creates, in order; a 500 is `null`. */
  mediaAnswers: (Record<string, unknown> | null)[];
  /** While set, a PATCH waits on it. */
  hold: Promise<void> | null;
  readonly patches: number;
}

const tus = (state: TusState, url: URL, init: RequestInit | undefined): Response | Promise<Response> => {
  const method = init?.method ?? 'GET';
  const path = url.pathname;
  const base = '/api/romeka/uploads';
  if (path === '/api/romeka/upload-limits') {
    return jsonResponse(LIMITS);
  }
  if (path === base && method === 'GET') {
    return jsonResponse({ uploads: [] });
  }
  if (path === base && method === 'POST') {
    state.next += 1;
    const upload = `u${String(state.next)}`;
    const header = new Headers(init?.headers).get('Upload-Metadata') ?? '';
    const encoded = header.split('filename ')[1]?.split(',')[0] ?? '';
    const file = Buffer.from(encoded, 'base64').toString('utf8');
    state.uploads.set(upload, { path: file, length: Number(new Headers(init?.headers).get('Upload-Length')), offset: 0 });
    return new Response(null, { status: 201, headers: { Location: `${base}/${upload}` } });
  }
  const upload = path.startsWith(`${base}/`) ? path.slice(base.length + 1) : '';
  const held = state.uploads.get(upload);
  if (held === undefined) {
    return jsonResponse({ errors: { error: 'Not found' } }, { status: 404 });
  }
  if (method === 'HEAD') {
    return new Response(null, { status: 200, headers: { 'Upload-Offset': String(held.offset), 'Upload-Length': String(held.length) } });
  }
  if (method === 'PATCH') {
    const body = init?.body;
    const size = body instanceof Blob ? body.size : 0;
    const store = (): Response => {
      held.offset += size;
      return new Response(null, { status: 204, headers: { 'Upload-Offset': String(held.offset) } });
    };
    return state.hold === null ? store() : state.hold.then(store);
  }
  return new Response(null, { status: 204 });
};

const server = (): { readonly state: TusState; readonly handler: Handler } => {
  const state: TusState = { uploads: new Map(), next: 0, mediaAnswers: [], hold: null, patches: 0 };
  const media = route({});
  return {
    state,
    handler: (url, init) => {
      if (url.pathname === '/api/romeka/upload-limits' || url.pathname.startsWith('/api/romeka/uploads')) {
        return tus(state, url, init);
      }
      if (url.pathname === '/api/romeka/media/probe') {
        return jsonResponse({ readers: ['youtube', 'oembed', 'url'], suggested: 'youtube', mediaType: null, size: null, title: 'Harvest, 1954', image: null, canvases: null });
      }
      if (url.pathname === '/api/media' && init?.method === 'POST') {
        const answer = state.mediaAnswers.length === 0 ? createdMedia() : state.mediaAnswers.shift();
        return answer === null || answer === undefined
          ? jsonResponse({ errors: { error: 'The server broke.' } }, { status: 500 })
          : jsonResponse(answer, { status: 201 });
      }
      if (url.pathname === '/api/media/12' && init?.method === 'PUT') {
        const body = requestBody({ url, init });
        return jsonResponse(createdMedia({ 'o:alt_text': body['o:alt_text'] }));
      }
      return media(url, init);
    },
  };
};

const fileOf = (name: string, size = 10, type = ''): File => new File([new Uint8Array(size)], name, { type });

const item = makeItem();

afterEach(() => {
  forgetAdds(id);
  vi.unstubAllGlobals();
});

describe('the pre-send checks', () => {
  it('refuses a file over the size limit before any request carries it', async () => {
    const { handler } = server();
    const fetchMock = serve(handler);
    const { container } = render(<AddMediaPanel item={item} onAdded={() => undefined} onClose={() => undefined} />);

    await screen.findByText(/Up to 2\.1 GB a file/u);
    const big = fileOf('lecture.mkv', 10, 'video/x-matroska');
    Object.defineProperty(big, 'size', { value: 3 * 1024 ** 3 });
    fireEvent.change(first(Array.from(container.querySelectorAll('input[type=file]'))), { target: { files: [big] } });

    expect(await screen.findByText(/lecture\.mkv is 3\.2 GB\. This archive takes up to 2\.1 GB a file\./u)).toBeInTheDocument();
    expect(requestsFor(fetchMock, '/api/romeka/uploads').some((request) => request.init?.method === 'POST')).toBe(false);
  });

  it('hands a long list to Import and sends nothing', async () => {
    const { handler } = server();
    const fetchMock = serve(handler);
    const { container } = render(<AddMediaPanel item={item} onAdded={() => undefined} onClose={() => undefined} />);
    await screen.findByText(/Up to 2\.1 GB a file/u);

    const files = Array.from({ length: 21 }, (_, index) => fileOf(`${String(index)}.jpg`));
    fireEvent.change(first(Array.from(container.querySelectorAll('input[type=file]'))), { target: { files } });

    expect(await screen.findByText(/21 files go to Import/u)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open Import' })).toHaveAttribute('href', '#/import');
    expect(requestsFor(fetchMock, '/api/romeka/uploads').some((request) => request.init?.method === 'POST')).toBe(false);
  });

  it('names what a pasted YouTube link is and adds it from the named reader', async () => {
    const { handler } = server();
    const fetchMock = serve(handler);
    render(<AddMediaPanel item={item} onAdded={() => undefined} onClose={() => undefined} />);
    const link = screen.getByLabelText('or paste a link');

    fireEvent.paste(link, { clipboardData: { getData: () => 'https://youtu.be/dQw4w9WgXcQ' } });

    expect(await screen.findByText('A YouTube video: Harvest, 1954')).toBeInTheDocument();
    const add = screen.getByRole('button', { name: 'Add YouTube video' });
    fireEvent.click(add);

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/media')).toHaveLength(1);
    });
    expect(requestBody(first(requestsFor(fetchMock, '/api/media')))).toEqual({
      'o:item': { 'o:id': 7 },
      'o:is_public': true,
      'o:ingester': 'youtube',
      'o:source': 'https://youtu.be/dQw4w9WgXcQ',
    });
  });

  it('adds pasted HTML from Advanced and says the scripts are removed', async () => {
    const { handler } = server();
    const fetchMock = serve(handler);
    render(<AddMediaPanel item={item} onAdded={() => undefined} onClose={() => undefined} />);

    fireEvent.click(screen.getByRole('button', { name: 'Advanced' }));
    expect(screen.getByText('Scripts and event attributes are removed when it is saved.')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Paste HTML'), { target: { value: '<p>Hello</p><script>alert(1)</script>' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add HTML' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/media')).toHaveLength(1);
    });
    expect(requestBody(first(requestsFor(fetchMock, '/api/media')))).toEqual({
      'o:item': { 'o:id': 7 },
      'o:is_public': true,
      'o:ingester': 'html',
      html: '<p>Hello</p><script>alert(1)</script>',
    });
  });
});

describe('the rows', () => {
  it('keeps the file and offers Try again after a failed create, then adds it', async () => {
    const { state, handler } = server();
    state.mediaAnswers = [null, createdMedia({ 'o:id': 12 })];
    serve(handler);
    const onAdded = vi.fn();
    const { container } = render(<AddMediaPanel item={item} onAdded={onAdded} onClose={() => undefined} />);
    await screen.findByText(/Up to 2\.1 GB a file/u);

    fireEvent.change(first(Array.from(container.querySelectorAll('input[type=file]'))), { target: { files: [fileOf('hall.jpg', 10, 'image/jpeg')] } });

    expect(await screen.findByRole('alert')).toHaveTextContent('The server broke.');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('Added')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open media #12' })).toHaveAttribute('href', '#/media/12');
    expect(onAdded).toHaveBeenCalledTimes(1);
    expect(state.uploads.size).toBe(1);
  });

  it('saves the description on blur', async () => {
    const { state, handler } = server();
    state.mediaAnswers = [createdMedia({ 'o:id': 12 })];
    const fetchMock = serve(handler);
    const { container } = render(<AddMediaPanel item={item} onAdded={() => undefined} onClose={() => undefined} />);
    await screen.findByText(/Up to 2\.1 GB a file/u);

    fireEvent.change(first(Array.from(container.querySelectorAll('input[type=file]'))), { target: { files: [fileOf('hall.jpg', 10, 'image/jpeg')] } });
    const describe = await screen.findByLabelText('Describe the image');
    fireEvent.change(describe, { target: { value: 'A hall at dusk' } });
    fireEvent.blur(describe);

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/media/12')).toHaveLength(1);
    });
    expect(requestBody(first(requestsFor(fetchMock, '/api/media/12')))['o:alt_text']).toBe('A hall at dusk');
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
  });

  it('does not close on Escape while a file is sending, and closes when it is done', async () => {
    const { state, handler } = server();
    let release = (): void => undefined;
    state.hold = new Promise<void>((resolve) => {
      release = resolve;
    });
    serve(handler);
    const onClose = vi.fn();
    const { container } = render(<AddMediaPanel item={item} onAdded={() => undefined} onClose={onClose} />);
    await screen.findByText(/Up to 2\.1 GB a file/u);

    fireEvent.change(first(Array.from(container.querySelectorAll('input[type=file]'))), { target: { files: [fileOf('hall.jpg', 10, 'image/jpeg')] } });
    await screen.findByRole('button', { name: 'Pause' });
    fireEvent.keyDown(container.querySelector('.add-media') ?? container, { key: 'Escape' });
    expect(onClose).not.toHaveBeenCalled();

    release();
    await screen.findByText('Added');
    fireEvent.keyDown(container.querySelector('.add-media') ?? container, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
