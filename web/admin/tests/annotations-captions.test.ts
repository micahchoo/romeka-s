/**
 * Captions through the notes panel: the language rows, the one dialog for add
 * and replace with its dry run, the two-step remove, and **Use as captions
 * of…** on a caption file.
 *
 * Every case drives the real panel and the real client with stubbed HTTP
 * answers, so the route, the query and the dry run are part of what is
 * asserted. The server is the only cue parser, so no case reads the file.
 */
import { createElement } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { brandId } from '../src/api/parse';
import type { Media } from '../src/api/types';
import { MediaNotes } from '../src/pages/media/annotations/panel';
import { installFetch, jsonResponse, listResponse, noContent, requestsFor } from './stub';

const MEDIA_ID = 9;
const BASE = 'http://localhost';
const CANVAS = `${BASE}/iiif/3/item/2/canvas/9`;
const CONTAINER_PATH = `/annotations/media/${String(MEDIA_ID)}/`;
const TRACKS_PATH = `/annotations/media/${String(MEDIA_ID)}/tracks`;

const media: Media = {
  id: brandId<'MediaId'>(MEDIA_ID),
  apiUrl: `${BASE}/api/media/9`,
  types: ['o:Media'],
  isPublic: true,
  owner: null,
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  title: null,
  created: '2026-10-20T09:00:00+00:00',
  modified: null,
  thumbnailDisplayUrls: {},
  values: new Map(),
  ingester: 'upload',
  renderer: 'file',
  kind: 'video',
  item: { id: brandId<'ItemId'>(2), apiUrl: `${BASE}/api/items/2` },
  source: null,
  mediaType: 'video/mp4',
  sha256: null,
  size: 6328,
  filename: 'video.mp4',
  lang: 'en',
  altText: null,
  originalUrl: `${BASE}/files/original/9.mp4`,
  thumbnailUrls: {},
};

interface TrackFacts {
  readonly lang: string;
  readonly cues: number;
  readonly imported: string | null;
  readonly handEdited: number;
  readonly undo?: { readonly cues: number; readonly imported: string | null } | null;
}

const trackBody = (track: TrackFacts): Record<string, unknown> => ({
  lang: track.lang,
  cues: track.cues,
  imported: track.imported,
  handEdited: track.handEdited,
  undo: track.undo ?? null,
});

const containerOf = (writable = true): Response =>
  jsonResponse(
    {
      '@context': ['http://www.w3.org/ns/anno.jsonld', 'http://www.w3.org/ns/ldp.jsonld'],
      id: `${BASE}${CONTAINER_PATH}`,
      type: ['BasicContainer', 'AnnotationCollection'],
      total: 0,
      first: {
        id: `${BASE}${CONTAINER_PATH}page`,
        type: 'AnnotationPage',
        startIndex: 0,
        items: [],
      },
    },
    {
      headers: {
        'Romeka-Surface': 'time; duration=2530',
        'Romeka-Canvas': CANVAS,
        Allow: writable ? 'GET, HEAD, POST, OPTIONS' : 'GET, HEAD, OPTIONS',
      },
    },
  );

/** A caption file the browser's file input can carry. */
const captionFile = (name: string, size = 48): File =>
  new File([new Uint8Array(size)], name, { type: 'text/vtt' });

/** Choose a file in the dialog's file input. */
const chooseFile = (file: File): void => {
  const input = screen.getByLabelText(/Caption file/);
  fireEvent.change(input, { target: { files: [file] } });
};

/** One media as the API sends it, for the item's media list. */
const mediaDocument = (id: number, filename: string, mediaType: string, kind: string): Record<string, unknown> => ({
  'o:id': id,
  '@id': `${BASE}/api/media/${String(id)}`,
  '@type': ['o:Media'],
  'o:is_public': true,
  'o:owner': null,
  'o:resource_class': null,
  'o:resource_template': null,
  'o:thumbnail': null,
  'o:title': null,
  'o:created': { '@value': '2026-10-20T09:00:00+00:00', '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
  'o:modified': null,
  thumbnail_display_urls: [],
  'o:ingester': 'upload',
  'o:renderer': 'file',
  'o:kind': kind,
  'o:item': { '@id': `${BASE}/api/items/2`, 'o:id': 2 },
  'o:source': null,
  'o:media_type': mediaType,
  'o:sha256': null,
  'o:size': 6328,
  'o:filename': filename,
  'o:lang': 'en',
  'o:alt_text': null,
  'o:original_url': `${BASE}/files/original/${String(id)}.mp4`,
  'o:thumbnail_urls': [],
});

/** A handler with the container answered, so the panel renders its surface. */
const handlerWith = (
  tracks: () => Response,
  writable = true,
): ((url: URL, init: RequestInit | undefined) => Response) => {
  return (url) => (url.pathname === CONTAINER_PATH ? containerOf(writable) : tracks());
};

/** The commit button of the dialog, whatever it is called. */
const commitButton = (): HTMLElement => {
  const dialog = screen.getByRole('dialog');
  const buttons = within(dialog).getAllByRole('button');
  const first = buttons[0];
  if (first === undefined) {
    throw new Error('the dialog rendered no commit button');
  }
  return first;
};

beforeEach(() => {
  window.sessionStorage.clear();
});

afterEach(() => {
  cleanup();
  window.sessionStorage.clear();
});

describe('the captions section', () => {
  it('teaches the empty state and offers one action', async () => {
    installFetch(handlerWith(() => jsonResponse([])));
    const onTracks = vi.fn();

    render(createElement(MediaNotes, { media, onTracks }));
    expect(
      await screen.findByText(
        'No captions yet. Add a caption file (.vtt or .srt) so viewers can read along and search the words.',
      ),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Add captions…' })).toBeDefined();
    await waitFor(() => {
      expect(onTracks).toHaveBeenCalledWith([]);
    });
  });

  it('lists one row per language and offers a reader no control', async () => {
    installFetch(
      handlerWith(() =>
        jsonResponse([
          trackBody({ lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 0 }),
          trackBody({ lang: 'fr', cues: 198, imported: '2026-10-20T09:13:00Z', handEdited: 2 }),
        ]),
      ),
    );

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    const rows = await screen.findAllByRole('listitem');
    const captions = rows.filter((row) => row.textContent.includes('from a caption file'));
    expect(captions).toHaveLength(2);
    expect(captions[0]?.textContent).toContain('English');
    expect(captions[0]?.textContent).toContain('214 cues');
    expect(captions[1]?.textContent).toContain('French');
    expect(captions[1]?.textContent).toContain('2 of these cues were changed here by hand');

    cleanup();
    installFetch(handlerWith(() => jsonResponse([]), false));
    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText(/No captions yet/);
    expect(screen.queryByRole('button', { name: 'Add captions…' })).toBeNull();
  });
});

describe('adding captions', () => {
  it('reads the file once, names the add, and reports the count the server kept', async () => {
    let tracks: readonly TrackFacts[] = [];
    const onTracks = vi.fn();
    const fetchMock = installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf();
      }
      if (url.pathname === TRACKS_PATH && method === 'GET') {
        return jsonResponse(tracks.map(trackBody));
      }
      if (url.pathname === TRACKS_PATH && method === 'POST' && url.searchParams.has('dry_run')) {
        return jsonResponse({ cues: 2, start: 0, end: 1000, replaces: 0, handEdited: 0 });
      }
      if (url.pathname === TRACKS_PATH && method === 'POST') {
        // A first import has no row to remove, so the route answers `false`.
        tracks = [{ lang: 'en', cues: 2, imported: '2026-10-21T08:00:00Z', handEdited: 0 }];
        return jsonResponse({ lang: 'en', cues: 2, replaced: false });
      }
      return jsonResponse({ errors: { error: `${method} ${url.pathname}` } }, { status: 500 });
    });

    render(createElement(MediaNotes, { media, onTracks }));
    fireEvent.click(await screen.findByRole('button', { name: 'Add captions…' }));

    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'en' } });
    chooseFile(captionFile('captions-en.vtt'));

    expect(await screen.findByText('This file has 2 cues, 0:00 to 0:01.')).toBeDefined();
    const commit = commitButton();
    expect(commit.textContent).toBe('Add English captions');
    fireEvent.click(commit);

    await screen.findByText('English captions: 2 cues imported.');
    expect(screen.getByText(/2 cues from a caption file/)).toBeDefined();
    await waitFor(() => {
      expect(onTracks).toHaveBeenCalled();
    });

    const posts = requestsFor(fetchMock, TRACKS_PATH).filter(
      (request) => request.init?.method === 'POST',
    );
    // The dry run first, then the import, both carrying the same bytes.
    expect(posts).toHaveLength(2);
    expect(posts[0]?.url.searchParams.get('dry_run')).toBe('1');
    expect(posts[0]?.url.searchParams.get('lang')).toBe('en');
    expect(posts[1]?.url.searchParams.get('lang')).toBe('en');
    expect(posts[1]?.url.searchParams.has('dry_run')).toBe(false);
  });

  it('refuses a file over 1 MiB before sending it', async () => {
    const fetchMock = installFetch(handlerWith(() => jsonResponse([])));

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    fireEvent.click(await screen.findByRole('button', { name: 'Add captions…' }));
    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'en' } });
    chooseFile(captionFile('huge.vtt', 1024 * 1024 + 1));

    expect(
      await screen.findByText('This file is 1,048,577 bytes. A caption file can be at most 1 MiB.'),
    ).toBeDefined();
    fireEvent.click(commitButton());

    expect(
      requestsFor(fetchMock, TRACKS_PATH).filter((request) => request.init?.method === 'POST'),
    ).toHaveLength(0);
  });
});

describe('replacing captions', () => {
  it('states what the dry run would change, then commits that replace', async () => {
    let tracks: readonly TrackFacts[] = [
      { lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 3 },
    ];
    const fetchMock = installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf();
      }
      if (url.pathname === TRACKS_PATH && method === 'GET') {
        return jsonResponse(tracks.map(trackBody));
      }
      if (url.pathname === TRACKS_PATH && method === 'POST' && url.searchParams.has('dry_run')) {
        return jsonResponse({ cues: 220, start: 0, end: 2_530_000, replaces: 214, handEdited: 3 });
      }
      if (url.pathname === TRACKS_PATH && method === 'POST') {
        tracks = [{ lang: 'en', cues: 220, imported: '2026-10-21T08:00:00Z', handEdited: 0 }];
        return jsonResponse({ lang: 'en', cues: 220, replaced: true });
      }
      return jsonResponse({ errors: { error: `${method} ${url.pathname}` } }, { status: 500 });
    });

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText(/214 cues from a caption file/);
    fireEvent.click(screen.getByRole('button', { name: 'Replace…' }));

    // The row's language is preselected: the dialog is already replacing en.
    expect(screen.getByLabelText('Language')).toHaveValue('en');
    chooseFile(captionFile('english.vtt', 64));

    expect(await screen.findByText('This file has 220 cues, 0:00 to 42:10.')).toBeDefined();
    expect(screen.getByText('It replaces the 214 English cues.')).toBeDefined();
    expect(
      screen.getByText('3 of those were changed here by hand; those changes will be lost.'),
    ).toBeDefined();
    const commit = commitButton();
    expect(commit.textContent).toBe('Replace 214 English cues');
    fireEvent.click(commit);

    await screen.findByText('English captions: 220 cues imported.');
    expect(screen.getByText(/220 cues from a caption file/)).toBeDefined();
    expect(
      requestsFor(fetchMock, TRACKS_PATH).filter((request) => request.init?.method === 'POST'),
    ).toHaveLength(2);
  });

  it('says so when the same file changes nothing', async () => {
    const tracks: readonly TrackFacts[] = [
      { lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 0 },
    ];
    installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf();
      }
      if (url.pathname === TRACKS_PATH && method === 'GET') {
        return jsonResponse(tracks.map(trackBody));
      }
      if (url.pathname === TRACKS_PATH && method === 'POST' && url.searchParams.has('dry_run')) {
        return jsonResponse({ cues: 214, start: 0, end: 2_530_000, replaces: 214, handEdited: 0 });
      }
      return jsonResponse({ lang: 'en', cues: 214, replaced: false });
    });

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText(/214 cues from a caption file/);
    fireEvent.click(screen.getByRole('button', { name: 'Replace…' }));
    chooseFile(captionFile('english.vtt', 64));
    await screen.findByText('This file has 214 cues, 0:00 to 42:10.');
    fireEvent.click(commitButton());

    await screen.findByText('No change. The file matches the current English captions.');
  });
});

describe('undoing a replace', () => {
  const undoPath = `${TRACKS_PATH}/undo`;

  it('says what comes back and brings it back with one click', async () => {
    let tracks: readonly TrackFacts[] = [
      {
        lang: 'en',
        cues: 220,
        imported: '2026-10-21T08:00:00Z',
        handEdited: 0,
        undo: { cues: 214, imported: '2026-10-20T09:12:44Z' },
      },
    ];
    const fetchMock = installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf();
      }
      if (url.pathname === TRACKS_PATH && method === 'GET') {
        return jsonResponse(tracks.map(trackBody));
      }
      if (url.pathname === undoPath && method === 'POST') {
        tracks = [{ lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 0 }];
        return jsonResponse({ lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z' });
      }
      return jsonResponse({ errors: { error: `${method} ${url.pathname}` } }, { status: 500 });
    });

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText(/220 cues from a caption file/);
    expect(
      screen.getByText(/Bring back the 214 English cues from Oct 20, 2026/),
    ).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Undo replace' }));

    await screen.findByText('English captions: 214 cues brought back.');
    expect(screen.getByText(/214 cues from a caption file/)).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Undo replace' })).toBeNull();

    const posts = requestsFor(fetchMock, undoPath);
    expect(posts).toHaveLength(1);
    expect(posts[0]?.init?.method).toBe('POST');
    expect(posts[0]?.url.searchParams.get('lang')).toBe('en');
  });

  it('shows the server’s sentence when there is nothing left to bring back', async () => {
    const tracks: readonly TrackFacts[] = [
      {
        lang: 'en',
        cues: 220,
        imported: '2026-10-21T08:00:00Z',
        handEdited: 0,
        undo: { cues: 214, imported: '2026-10-20T09:12:44Z' },
      },
    ];
    installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf();
      }
      if (url.pathname === TRACKS_PATH && method === 'GET') {
        return jsonResponse(tracks.map(trackBody));
      }
      return jsonResponse(
        {
          errors: {
            error:
              'There is no caption edition to bring back. An undo works once, until the next import of these captions.',
          },
        },
        { status: 409 },
      );
    });

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText(/220 cues from a caption file/);
    fireEvent.click(screen.getByRole('button', { name: 'Undo replace' }));

    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('There is no caption edition to bring back.');
    expect(alert.textContent).toContain('(HTTP 409)');
  });
});

describe('removing a language', () => {
  it('asks with the count, and removes in two steps', async () => {
    let tracks: readonly TrackFacts[] = [
      { lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 0 },
    ];
    const fetchMock = installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === CONTAINER_PATH) {
        return containerOf();
      }
      if (url.pathname === TRACKS_PATH && method === 'GET') {
        return jsonResponse(tracks.map(trackBody));
      }
      if (url.pathname === TRACKS_PATH && method === 'DELETE') {
        tracks = [];
        return noContent();
      }
      return jsonResponse({}, { status: 500 });
    });

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText(/214 cues from a caption file/);
    fireEvent.click(screen.getByRole('button', { name: 'Remove…' }));

    expect(
      screen.getByText(
        'Remove the 214 English cues? Visitors will no longer see English captions. Your own notes stay.',
      ),
    ).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Remove English captions' }));

    await screen.findByText('English captions removed.');
    expect(screen.getByText(/No captions yet/)).toBeDefined();
    const deletes = requestsFor(fetchMock, TRACKS_PATH).filter(
      (request) => request.init?.method === 'DELETE',
    );
    expect(deletes).toHaveLength(1);
    expect(deletes[0]?.url.searchParams.get('lang')).toBe('en');
  });
});

describe('an imported cue', () => {
  it('opens in Advanced and says the next import replaces it', async () => {
    const trackNote = {
      id: `${BASE}/annotations/90311`,
      type: 'Annotation',
      motivation: 'supplementing',
      modified: '2026-10-20T09:12:44Z',
      body: { type: 'TextualBody', value: 'A red square', format: 'text/plain', language: 'en' },
      target: {
        type: 'SpecificResource',
        source: { id: CANVAS, type: 'Canvas' },
        selector: {
          type: 'FragmentSelector',
          conformsTo: 'http://www.w3.org/TR/media-frags/',
          value: 't=0,0.4',
        },
      },
    };
    installFetch((url) => {
      if (url.pathname === CONTAINER_PATH) {
        return jsonResponse(
          {
            '@context': ['http://www.w3.org/ns/anno.jsonld', 'http://www.w3.org/ns/ldp.jsonld'],
            id: `${BASE}${CONTAINER_PATH}`,
            type: ['BasicContainer', 'AnnotationCollection'],
            total: 1,
            first: {
              id: `${BASE}${CONTAINER_PATH}page`,
              type: 'AnnotationPage',
              startIndex: 0,
              items: [trackNote],
            },
          },
          {
            headers: {
              'Romeka-Surface': 'time; duration=2530',
              'Romeka-Canvas': CANVAS,
              'Romeka-Track-Ids': '90311',
              Allow: 'GET, HEAD, POST, OPTIONS',
            },
          },
        );
      }
      return jsonResponse(trackNote, { headers: { ETag: '"r1"' } });
    });

    render(createElement(MediaNotes, { media, onTracks: () => undefined }));
    await screen.findByText('1 note. 1 at a time, 0 on the whole file.');
    const row = screen
      .getAllByRole('button', { name: /A red square/ })
      .find((button) => button.classList.contains('note-open'));
    if (row === undefined) {
      throw new Error('the list rendered no entry for the cue');
    }
    fireEvent.click(row);

    const form = (await screen.findByRole('button', { name: 'Save' })).closest('form');
    if (form === null) {
      throw new Error('the note form did not open');
    }
    expect(form).toHaveTextContent(
      'From a caption file. The next import of these captions replaces this note.',
    );
    // Advanced is open, so the motivation the row carries is visible and editable.
    expect(within(form).getByLabelText('What the note does')).toBeDefined();
  });
});

describe('a caption file on the item', () => {
  it('is offered to the item’s recording and posted there', async () => {
    const captionMedia: Media = {
      ...media,
      id: brandId<'MediaId'>(11),
      apiUrl: `${BASE}/api/media/11`,
      kind: 'captions',
      mediaType: 'text/vtt',
      filename: 'captions.vtt',
      originalUrl: `${BASE}/files/original/11.vtt`,
      lang: 'en',
    };
    const target: Media = { ...media, id: brandId<'MediaId'>(12), apiUrl: `${BASE}/api/media/12` };
    const posted: string[] = [];
    installFetch((url, init) => {
      const method = init?.method ?? 'GET';
      if (url.pathname === '/api/media') {
        return listResponse([mediaDocument(12, 'video.mp4', 'video/mp4', 'video')], 1);
      }
      if (url.pathname === `/api/media/${String(captionMedia.id)}`) {
        return jsonResponse({});
      }
      if (url.pathname === '/files/original/11.vtt') {
        return new Response('WEBVTT\n\n00:00.000 --> 00:00.400\nA red square.', {
          headers: { 'Content-Type': 'text/vtt' },
        });
      }
      if (url.pathname === `/annotations/media/${String(target.id)}/tracks`) {
        if (method === 'POST') {
          posted.push(url.search);
          return jsonResponse({ lang: 'en', cues: 1, replaced: true });
        }
        return jsonResponse([]);
      }
      if (url.pathname === `/annotations/media/${String(captionMedia.id)}/`) {
        return jsonResponse(
          {
            '@context': ['http://www.w3.org/ns/anno.jsonld', 'http://www.w3.org/ns/ldp.jsonld'],
            id: `${BASE}/annotations/media/11/`,
            type: ['BasicContainer', 'AnnotationCollection'],
            total: 0,
            first: { id: `${BASE}/annotations/media/11/page`, type: 'AnnotationPage', items: [] },
          },
          {
            headers: {
              'Romeka-Surface': 'whole',
              'Romeka-Canvas': `${BASE}/iiif/3/item/2/canvas/11`,
              Allow: 'GET, HEAD, POST, OPTIONS',
            },
          },
        );
      }
      return jsonResponse({ errors: { error: `${method} ${url.pathname}` } }, { status: 500 });
    });

    render(createElement(MediaNotes, { media: captionMedia, onTracks: () => undefined }));
    fireEvent.click(await screen.findByRole('button', { name: /video\.mp4/ }));

    const dialog = await screen.findByRole('dialog');
    expect(dialog.textContent).toContain('captions.vtt');
    expect(within(dialog).getByLabelText('Language')).toHaveValue('en');
    fireEvent.click(commitButton());

    await screen.findByText('English captions: 1 cue imported.');
    const imports = posted.filter((search) => !search.includes('dry_run'));
    expect(imports).toHaveLength(1);
    expect(imports[0]).toContain('lang=en');
  });
});
