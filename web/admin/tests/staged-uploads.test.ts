/**
 * The staged upload target (`api/staged-uploads.ts`) through the real tus
 * client: the routes it names, the metadata that ties an upload to its item,
 * the media create body, and the probe and limits parsers.
 */
import { describe, expect, it } from 'vitest';
import { brandId, isRecord, ParseError } from '../src/api/parse';
import { StagedUploads, createMedia, parseProbe, parseUploadLimits, probeLink, uploadLimits } from '../src/api/staged-uploads';
import { ApiError } from '../src/api/client';
import { TusUploads, UploadRefusedError, parseTusListing } from '../src/app/upload/tus';
import { first, installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';
import { loadFixture } from './loadFixture';

const ITEM = brandId<'ItemId'>(7);

const tusWith = (fetchFn: typeof fetch): TusUploads =>
  new TusUploads({ basePath: '/api/romeka/uploads', baseUrl: '', fetchFn, readListing: parseTusListing });

const limitsBody = { maxFileBytes: 2 * 1024 ** 3, kinds: ['audio', 'video'], extensions: ['jpg', 'mp4'], validationOff: false };

const listing = {
  uploads: [
    { id: 'u1', path: 'a.jpg', mediaType: 'image/jpeg', itemId: 7, length: 10, received: 10, created: '2026-10-21T10:00:00+00:00', expires: '2026-10-22T10:00:00+00:00' },
    { id: 'u2', path: 'b.mp4', mediaType: 'video/mp4', itemId: 7, length: 30, received: 10, created: '2026-10-21T10:00:00+00:00', expires: '2026-10-22T10:00:00+00:00' },
  ],
};

const decodeMetadata = (header: string): Record<string, string> =>
  Object.fromEntries(
    header.split(',').map((entry) => {
      const [name, value] = entry.trim().split(' ');
      return [name ?? '', Buffer.from(value ?? '', 'base64').toString('utf8')];
    }),
  );

describe('the staged target', () => {
  it('reads the limits once and states the file size, with no total', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/upload-limits': () => jsonResponse(limitsBody) }));
    const target = new StagedUploads(ITEM, tusWith(fetchMock));

    expect(await target.readLimits()).toEqual({
      kind: 'ready',
      limits: { maxFileBytes: 2 * 1024 ** 3, kinds: ['audio', 'video'], extensions: ['jpg', 'mp4'], validationOff: false },
    });
    expect(await target.limits()).toEqual({ file: 2 * 1024 ** 3, total: null });
    expect(await target.readLimits()).toEqual(await target.readLimits());
    expect(requestsFor(fetchMock, '/api/romeka/upload-limits')).toHaveLength(1);
  });

  it('reads a collection that states neither a total nor a limit, and sums the bytes', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/uploads': () => jsonResponse(listing) }));
    const target = new StagedUploads(ITEM, tusWith(fetchMock));

    expect(await target.list()).toEqual({
      uploads: [
        { id: 'u1', path: 'a.jpg', length: 10, received: 10 },
        { id: 'u2', path: 'b.mp4', length: 30, received: 10 },
      ],
      used: 40,
      limit: null,
    });
    expect(first(requestsFor(fetchMock, '/api/romeka/uploads')).url.searchParams.get('item')).toBe('7');
  });

  it('names the item and the file type when it starts an upload', async () => {
    const fetchMock = installFetch(route({ '/api/romeka/uploads': () => new Response(null, { status: 201, headers: { Location: '/api/romeka/uploads/u9' } }) }));
    const target = new StagedUploads(ITEM, tusWith(fetchMock));

    expect(await target.create('Box/a.jpg', 10, 'image/jpeg')).toBe('u9');
    const post = first(requestsFor(fetchMock, '/api/romeka/uploads'));
    expect(post.init?.method).toBe('POST');
    expect(decodeMetadata(new Headers(post.init?.headers).get('Upload-Metadata') ?? '')).toEqual({
      filename: 'Box/a.jpg',
      item: '7',
      filetype: 'image/jpeg',
    });
  });

  it('answers the problem and the closure the staged routes mean', () => {
    const target = new StagedUploads(ITEM, tusWith(() => Promise.resolve(jsonResponse({}))));
    const big = new UploadRefusedError({ code: 'fileTooLarge', limit: 5 }, 413, { error: ['too big'] }, 'too big', { error: ['too big'] });
    expect(target.problemOf(big)).toEqual({ kind: 'fileTooLarge', limit: 5 });
    expect(target.problemOf(new ApiError(500, null, 'HTTP 500', null))).toEqual({ kind: 'server', message: 'HTTP 500' });
    expect(target.closes({ kind: 'notAllowed' })).toBe(true);
    expect(target.closes({ kind: 'gone' })).toBe(false);
  });
});

describe('the media create', () => {
  const fixture = loadFixture('media-951.json');
  const created = isRecord(fixture) ? { ...fixture, 'o:id': 12 } : {};

  it('names the staged upload and the item, and sends the alt text', async () => {
    const fetchMock = installFetch(route({ '/api/media': () => jsonResponse(created, { status: 201 }) }));
    const media = await createMedia(ITEM, { kind: 'upload', upload: 'u9' }, { isPublic: true, altText: 'A hall at dusk' });
    expect(media.id).toBe(12);
    expect(requestBody(first(requestsFor(fetchMock, '/api/media')))).toEqual({
      'o:item': { 'o:id': 7 },
      'o:is_public': true,
      'o:alt_text': 'A hall at dusk',
      'o:ingester': 'upload',
      'o-module-romeka:upload': 'u9',
    });
  });

  it('sends a link with the chosen reader and HTML as pasted', async () => {
    const fetchMock = installFetch(route({ '/api/media': () => jsonResponse(created) }));
    await createMedia(ITEM, { kind: 'link', ingester: 'youtube', url: 'https://youtu.be/x' }, { isPublic: false, altText: null });
    await createMedia(ITEM, { kind: 'html', html: '<p>Hello</p>' }, { isPublic: true, altText: null });
    const bodies = requestsFor(fetchMock, '/api/media').map(requestBody);
    expect(bodies[0]).toEqual({ 'o:item': { 'o:id': 7 }, 'o:is_public': false, 'o:ingester': 'youtube', 'o:source': 'https://youtu.be/x' });
    expect(bodies[1]).toEqual({ 'o:item': { 'o:id': 7 }, 'o:is_public': true, 'o:ingester': 'html', html: '<p>Hello</p>' });
  });
});

describe('the limits and the probe', () => {
  it('reads the limits route', async () => {
    installFetch(route({ '/api/romeka/upload-limits': () => jsonResponse(limitsBody) }));
    expect((await uploadLimits()).maxFileBytes).toBe(2 * 1024 ** 3);
  });

  it('reads what a link turned out to be', async () => {
    const answer = {
      readers: ['youtube', 'oembed', 'url'],
      suggested: 'youtube',
      mediaType: null,
      size: null,
      title: 'Harvest, 1954',
      image: null,
      canvases: null,
    };
    const fetchMock = installFetch(route({ '/api/romeka/media/probe': () => jsonResponse(answer) }));
    const probe = await probeLink('https://youtu.be/x');
    expect(probe).toEqual(answer);
    expect(requestBody(first(requestsFor(fetchMock, '/api/romeka/media/probe')))).toEqual({ url: 'https://youtu.be/x' });
  });

  it('reads an image probe and refuses a reader this build does not know', () => {
    expect(
      parseProbe({
        readers: ['url'],
        suggested: 'url',
        mediaType: 'image/png',
        size: 42,
        title: null,
        image: { w: 120, h: 80 },
        canvases: null,
      }),
    ).toEqual({ readers: ['url'], suggested: 'url', mediaType: 'image/png', size: 42, title: null, image: { w: 120, h: 80 }, canvases: null });
    expect(() => parseProbe({ readers: ['asciidoctor'], suggested: 'asciidoctor', mediaType: null, size: null, title: null, image: null, canvases: null })).toThrow(ParseError);
    expect(() => parseUploadLimits({ ...limitsBody, extra: true })).toThrow(ParseError);
  });
});
