/**
 * The server's media kind (`o:kind`) in the admin. `mediaKind` reads the
 * server's word for every variant it knows, and only an absent or unknown field
 * falls back to the admin's own table. The last case is the one that matters
 * for a server that predates the field.
 */
import { describe, expect, it } from 'vitest';
import { brandId, parseMedia } from '../src/api/parse';
import type { Media, ServerMediaKind } from '../src/api/types';
import { mediaKind } from '../src/app/media';

const media = (kind: ServerMediaKind | null, fields: Partial<Media> = {}): Media => ({
  id: brandId<'MediaId'>(7),
  apiUrl: 'http://localhost/api/media/7',
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
  kind,
  item: null,
  source: null,
  mediaType: null,
  sha256: null,
  size: null,
  filename: null,
  lang: null,
  altText: null,
  originalUrl: null,
  thumbnailUrls: {},
  ...fields,
});

/** Every server variant and the admin's word for it. `other-file` depends on the type. */
const WORDS: readonly (readonly [ServerMediaKind, string])[] = [
  ['local-image', 'image'],
  ['iiif-image', 'image'],
  ['audio', 'audio'],
  ['video', 'video'],
  ['pdf', 'pdf'],
  ['model-3d', 'model'],
  ['captions', 'captions'],
  ['embed', 'embed'],
  ['iiif-presentation', 'embed'],
];

describe('the server media kind', () => {
  it.each(WORDS)('names %s as %s', (kind, word) => {
    expect(mediaKind(media(kind))).toBe(word);
  });

  it('splits the server catch-all by the file type', () => {
    const document = media('other-file', {
      mediaType: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      filename: 'report.docx',
    });
    expect(mediaKind(document)).toBe('document');

    // A generic type and a known document extension is still a document.
    const text = media('other-file', { mediaType: 'text/plain', filename: 'notes.md' });
    expect(mediaKind(text)).toBe('document');

    const unknown = media('other-file', {
      mediaType: 'application/octet-stream',
      filename: 'x.bin',
    });
    expect(mediaKind(unknown)).toBe('other');
  });

  it('reads the server word before the admin table', () => {
    // The table would call a HEIC an image, but the server says it has no
    // kind of its own, so the row must not claim to be a picture.
    const heic = media('other-file', {
      mediaType: 'image/heic',
      filename: 'photo.heic',
    });
    expect(mediaKind(heic)).toBe('other');

    // The table would call an OBJ a model; this server paints only glTF, so
    // its answer wins here too.
    const obj = media('other-file', { mediaType: 'model/obj', filename: 'statue.obj' });
    expect(mediaKind(obj)).toBe('other');
  });

  it('falls back to the admin table when the field is absent', () => {
    const audio = media(null, { mediaType: 'audio/mpeg', filename: 'tone.mp3' });
    expect(mediaKind(audio)).toBe('audio');

    const iiif = media(null, { ingester: 'iiif', renderer: 'iiif', mediaType: 'image/jpeg' });
    expect(mediaKind(iiif)).toBe('image');

    const pdf = media(null, { mediaType: 'application/pdf', filename: 'scan.pdf' });
    expect(mediaKind(pdf)).toBe('pdf');
  });
});

describe('the o:kind field at the API boundary', () => {
  const raw = (member: Record<string, unknown>): Record<string, unknown> => ({
    '@id': 'http://localhost/api/media/1',
    '@type': ['o:Media'],
    'o:id': 1,
    'o:is_public': true,
    'o:owner': null,
    'o:resource_class': null,
    'o:resource_template': null,
    'o:thumbnail': null,
    'o:title': null,
    thumbnail_display_urls: {},
    'o:created': {
      '@value': '2024-01-01T00:00:00+00:00',
      '@type': 'http://www.w3.org/2001/XMLSchema#dateTime',
    },
    'o:modified': null,
    'o:ingester': 'upload',
    'o:renderer': 'file',
    'o:item': { '@id': 'http://localhost/api/items/1', 'o:id': 1 },
    'o:source': 'tone.mp3',
    'o:media_type': 'audio/mpeg',
    'o:sha256': null,
    'o:size': 1,
    'o:filename': 'storage.mp3',
    'o:lang': null,
    'o:alt_text': null,
    'o:original_url': null,
    'o:thumbnail_urls': {},
    data: [],
    ...member,
  });

  it('reads a name this server sends', () => {
    expect(parseMedia(raw({ 'o:kind': 'audio' })).kind).toBe('audio');
  });

  it('keeps an absent field null, which is the fallback path', () => {
    expect(parseMedia(raw({})).kind).toBeNull();
  });

  it('keeps a name this build does not know null instead of failing the document', () => {
    expect(parseMedia(raw({ 'o:kind': 'document' })).kind).toBeNull();
  });
});
