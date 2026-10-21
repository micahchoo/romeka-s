/**
 * The notes client (contract section 5, A-2): the container with its pages,
 * the conditional writes with their typed refusals, and the caption track
 * routes G3, G4 and G5 propose. Every answer is a stub, so the assertions are
 * the requests this client makes and the values it returns.
 */
import { describe, expect, it } from 'vitest';
import { ApiError } from '../src/api/client';
import { brandId } from '../src/api/parse';
import {
  NoteConflict,
  NoteGone,
  NotesClient,
} from '../src/pages/media/annotations/client';
import { fromW3C, notesOf } from '../src/pages/media/annotations/wire';
import type { Draft, Note } from '../src/pages/media/annotations/wire';
import { loadFixture } from './loadFixture';

const MEDIA = brandId<'MediaId'>(4127);
const CANVAS = 'https://archive.example/iiif/3/item/812/canvas/4127';
const NOTE_IRI = 'https://archive.example/annotations/90311';
const SURFACE = 'image; width=4000; height=3000; service="https://archive.example/iiif/image/3/4127"';

const draft: Draft = {
  motivation: 'commenting',
  selector: { kind: 'rect', x: 1200, y: 340, w: 410, h: 388 },
  text: 'Watermark: crowned shield',
  language: 'en',
};

const json = (status: number, body: unknown, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/ld+json', ...headers } });

const noContent = (status: number): Response => new Response(null, { status });

interface Recorded {
  readonly url: string;
  readonly init: RequestInit;
}

type Answer = () => Response;

const recorder = (
  answers: readonly Answer[],
): { readonly calls: readonly Recorded[]; readonly client: NotesClient } => {
  const calls: Recorded[] = [];
  const fetchFn: typeof fetch = (input, init) => {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
    calls.push({ url, init: init ?? {} });
    const answer = answers[calls.length - 1];
    if (answer === undefined) {
      return Promise.reject(new Error(`unexpected request: ${url}`));
    }
    return Promise.resolve(answer());
  };
  return { calls, client: new NotesClient({ baseUrl: '', fetchFn }) };
};

const headersOf = (call: Recorded | undefined): Headers => new Headers(call?.init.headers);

const sentOf = (call: Recorded | undefined): unknown => {
  const body = call?.init.body;
  if (typeof body !== 'string') {
    throw new Error('expected the request body to be JSON text');
  }
  const value: unknown = JSON.parse(body);
  return value;
};

const rejection = async (promise: Promise<unknown>): Promise<unknown> => {
  try {
    await promise;
  } catch (error) {
    return error;
  }
  throw new Error('expected the call to fail');
};

const pageDocument = loadFixture('annotation-page.json');
const containerDocument = loadFixture('annotation-container.json');

const storedNote = (etag: string): Note => {
  const note = notesOf(pageDocument, new Set())[0];
  if (note === undefined) {
    throw new Error('the page fixture holds one note');
  }
  return { ...note, etag };
};

const annotationAt = (id: string, value: string): Record<string, unknown> => ({
  id,
  type: 'Annotation',
  motivation: 'commenting',
  body: { type: 'TextualBody', value, format: 'text/plain' },
  target: {
    type: 'SpecificResource',
    source: { id: CANVAS, type: 'Canvas' },
    selector: {
      type: 'FragmentSelector',
      conformsTo: 'http://www.w3.org/TR/media-frags/',
      value: 'xywh=pixel:1200,340,410,388',
    },
  },
});

describe('container', () => {
  it('reads the surface, the rights, the total, the canvas and the notes', async () => {
    const { client, calls } = recorder([
      () =>
        json(200, containerDocument, {
          'Romeka-Surface': SURFACE,
          Allow: 'GET, HEAD, POST, OPTIONS',
          'Romeka-Track-Ids': '90311',
          'Romeka-Canvas': CANVAS,
        }),
    ]);
    const container = await client.container(MEDIA);
    expect(calls.map((call) => call.url)).toEqual(['/annotations/media/4127/']);
    expect(container.surface).toEqual({
      kind: 'image',
      width: 4000,
      height: 3000,
      service: 'https://archive.example/iiif/image/3/4127',
    });
    expect(container.writable).toBe(true);
    expect(container.total).toBe(12);
    expect(container.canvas).toBe(CANVAS);
    expect(container.notes).toHaveLength(1);
    expect(container.notes[0]?.iri).toBe(NOTE_IRI);
    expect(container.notes[0]?.fromTrack).toBe(true);
  });

  it('is read-only when Allow has no POST, and learns the canvas from a note', async () => {
    const { client } = recorder([
      () => json(200, containerDocument, { 'Romeka-Surface': 'whole', Allow: 'GET, HEAD, OPTIONS' }),
    ]);
    const container = await client.container(MEDIA);
    expect(container.writable).toBe(false);
    expect(container.surface).toEqual({ kind: 'whole' });
    expect(container.canvas).toBe(CANVAS);
  });

  it('follows next and collects the track ids of every page', async () => {
    const next = 'https://archive.example/annotations/media/4127/page?after=90311';
    const first = {
      id: 'https://archive.example/annotations/media/4127/page',
      type: 'AnnotationPage',
      items: [annotationAt(NOTE_IRI, 'one')],
      next,
    };
    const second = {
      id: next,
      type: 'AnnotationPage',
      items: [annotationAt('https://archive.example/annotations/90312', 'two')],
    };
    const { client, calls } = recorder([
      () => json(200, { total: 2, first }, { 'Romeka-Surface': 'whole' }),
      () => json(200, second, { 'Romeka-Track-Ids': '90312' }),
    ]);
    const container = await client.container(MEDIA);
    expect(calls.map((call) => call.url)).toEqual(['/annotations/media/4127/', next]);
    expect(container.notes.map((note) => note.text)).toEqual(['one', 'two']);
    expect(container.notes.map((note) => note.fromTrack)).toEqual([false, true]);
  });
});

describe('create', () => {
  it('sends the Idempotency-Key, the draft on the canvas, and reads the ETag', async () => {
    const { client, calls } = recorder([
      () => json(201, annotationAt(NOTE_IRI, 'Watermark: crowned shield'), { ETag: '"r1"' }),
    ]);
    const note = await client.create(MEDIA, draft, '6f1c2a9b', CANVAS);
    expect(calls.map((call) => call.url)).toEqual(['/annotations/media/4127/']);
    const call = calls[0];
    expect(call?.init.method).toBe('POST');
    expect(headersOf(call).get('Idempotency-Key')).toBe('6f1c2a9b');
    expect(headersOf(call).get('Content-Type')).toContain('anno.jsonld');
    expect(sentOf(call)).toMatchObject({
      motivation: 'commenting',
      body: { type: 'TextualBody', value: 'Watermark: crowned shield' },
      target: {
        type: 'SpecificResource',
        source: { id: CANVAS },
        selector: { value: 'xywh=pixel:1200,340,410,388' },
      },
    });
    expect(note.iri).toBe(NOTE_IRI);
    expect(note.etag).toBe('"r1"');
    expect(note.selector).toEqual(draft.selector);
  });

  it('reuses the key when the same draft is retried, and a 200 replay is the same note', async () => {
    const { client, calls } = recorder([
      () => json(201, annotationAt(NOTE_IRI, 'Watermark'), { ETag: '"r1"' }),
      () => json(200, annotationAt(NOTE_IRI, 'Watermark'), { ETag: '"r1"' }),
    ]);
    const first = await client.create(MEDIA, draft, '6f1c2a9b', CANVAS);
    const second = await client.create(MEDIA, draft, '6f1c2a9b', CANVAS);
    expect(second.iri).toBe(first.iri);
    expect(calls.map((call) => headersOf(call).get('Idempotency-Key'))).toEqual([
      '6f1c2a9b',
      '6f1c2a9b',
    ]);
  });

  it('answers NoteGone on a 404', async () => {
    const { client } = recorder([() => json(404, { errors: { error: 'Not found.' } })]);
    expect(await rejection(client.create(MEDIA, draft, '6f1c2a9b', CANVAS))).toBeInstanceOf(NoteGone);
  });
});

describe('replace', () => {
  it('sends If-Match with the note ETag and the server document with the edits applied', async () => {
    const stored = { ...annotationAt(NOTE_IRI, 'Watermark'), 'dcterms:source': 'Briquet 1442' };
    const { client, calls } = recorder([() => json(200, stored, { ETag: '"r2"' })]);
    const note = await client.replace(storedNote('"r1"'), draft);
    const call = calls[0];
    expect(call?.url).toBe(NOTE_IRI);
    expect(call?.init.method).toBe('PUT');
    expect(headersOf(call).get('If-Match')).toBe('"r1"');
    expect(sentOf(call)).toMatchObject({
      'dcterms:source': 'Briquet 1442',
      body: { type: 'TextualBody', value: 'Watermark: crowned shield', language: 'en' },
      target: { selector: { value: 'xywh=pixel:1200,340,410,388' } },
    });
    expect(note.etag).toBe('"r2"');
  });

  it('reads the current note first when the note came from a page and carries no ETag', async () => {
    const current = { ...annotationAt(NOTE_IRI, 'Watermark'), 'dcterms:source': 'Briquet 1500' };
    const { client, calls } = recorder([
      () => json(200, current, { ETag: '"r4"' }),
      () => json(200, current, { ETag: '"r5"' }),
    ]);
    await client.replace(storedNote(''), draft);
    expect(calls.map((call) => call.init.method ?? 'GET')).toEqual(['GET', 'PUT']);
    expect(headersOf(calls[1]).get('If-Match')).toBe('"r4"');
    expect(sentOf(calls[1])).toMatchObject({ 'dcterms:source': 'Briquet 1500' });
  });

  it('throws NoteConflict with the server version and its ETag on 412', async () => {
    const current = { ...annotationAt(NOTE_IRI, 'Their words'), 'dcterms:source': 'Briquet 1600' };
    const { client } = recorder([() => json(412, current, { ETag: '"r3"' })]);
    const error = await rejection(client.replace(storedNote('"r1"'), draft));
    expect(error).toBeInstanceOf(NoteConflict);
    if (error instanceof NoteConflict) {
      expect(error.current.etag).toBe('"r3"');
      expect(error.current.text).toBe('Their words');
      expect(error.current.iri).toBe(NOTE_IRI);
    }
  });

  it('throws NoteGone on 404, also when the note has no ETag yet', async () => {
    const { client } = recorder([() => json(404, { errors: { error: 'Not found.' } })]);
    expect(await rejection(client.replace(storedNote(''), draft))).toBeInstanceOf(NoteGone);
  });
});

describe('remove', () => {
  it('sends DELETE with If-Match', async () => {
    const { client, calls } = recorder([() => noContent(204)]);
    await client.remove(storedNote('"r2"'));
    expect(calls[0]?.url).toBe(NOTE_IRI);
    expect(calls[0]?.init.method).toBe('DELETE');
    expect(headersOf(calls[0]).get('If-Match')).toBe('"r2"');
  });

  it('answers NoteGone when the note is already deleted', async () => {
    const { client } = recorder([() => json(404, { errors: { error: 'Not found.' } })]);
    expect(await rejection(client.remove(storedNote('"r2"')))).toBeInstanceOf(NoteGone);
  });
});

describe('caption tracks', () => {
  it('lists the languages with their cue counts (G3)', async () => {
    const { client, calls } = recorder([
      () =>
        json(200, [
          { lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 3 },
          { lang: 'fr', cues: 12, imported: null, handEdited: 0 },
        ]),
    ]);
    expect(await client.tracks(MEDIA)).toEqual([
      { lang: 'en', cues: 214, imported: '2026-10-20T09:12:44Z', handEdited: 3 },
      { lang: 'fr', cues: 12, imported: null, handEdited: 0 },
    ]);
    expect(calls.map((call) => call.url)).toEqual(['/annotations/media/4127/tracks']);
  });

  it('removes one language (G4)', async () => {
    const { client, calls } = recorder([() => noContent(204)]);
    await client.removeTrack(MEDIA, 'en');
    expect(calls[0]?.url).toBe('/annotations/media/4127/tracks?lang=en');
    expect(calls[0]?.init.method).toBe('DELETE');
  });

  it('previews an import with dry_run=1, sending the file bytes (G5)', async () => {
    const file = new File(['WEBVTT\n\n'], 'captions.vtt', { type: '' });
    const { client, calls } = recorder([
      () => json(200, { cues: 220, start: 0, end: 2530000, replaces: 214, handEdited: 3 }),
    ]);
    const preview = await client.previewTrack(MEDIA, file, 'en');
    const call = calls[0];
    expect(call?.url).toContain('/annotations/media/4127/tracks?');
    expect(call?.url).toContain('dry_run=1');
    expect(call?.url).toContain('lang=en');
    expect(call?.init.method).toBe('POST');
    expect(headersOf(call).get('Content-Type')).toBe('text/vtt');
    expect(call?.init.body).toBe(file);
    expect(preview).toEqual({ cues: 220, startMs: 0, endMs: 2530000, replaces: 214, handEdited: 3 });
  });

  it('imports an SRT with its own content type and reads the summary', async () => {
    const file = new File(['1\n00:01:02,000 --> 00:01:10,000\nText\n'], 'captions.srt', { type: '' });
    const { client, calls } = recorder([() => json(200, { lang: 'fr', cues: 12, replaced: false })]);
    expect(await client.importTrack(MEDIA, file, 'fr')).toEqual({ lang: 'fr', cues: 12, replaced: false });
    expect(calls[0]?.url).toBe('/annotations/media/4127/tracks?lang=fr');
    expect(headersOf(calls[0]).get('Content-Type')).toBe('application/x-subrip');
  });

  it('keeps the server sentence when a caption file is refused', async () => {
    const file = new File(['x'], 'captions.vtt');
    const { client } = recorder([() => json(400, { errors: { error: 'This media has no timeline.' } })]);
    const error = await rejection(client.previewTrack(MEDIA, file, 'en'));
    expect(error).toBeInstanceOf(ApiError);
    if (error instanceof ApiError) {
      expect(error.status).toBe(400);
      expect(error.message).toBe('This media has no timeline.');
    }
  });
});

it('reads a note document without the page wrapper', () => {
  const note = fromW3C(annotationAt(NOTE_IRI, 'One line'), new Set());
  expect(note.text).toBe('One line');
  expect(note.etag).toBe('');
});
