/**
 * The admin's media helpers over one fixture per corpus kind. A preview or
 * large URL is a URL to an image, or null when the kind has no still image to
 * show. The helpers currently fall back to the original file for every kind,
 * so an audio or PDF row gives an `<img>` pointing at a file the browser
 * cannot decode.
 *
 * `KNOWN_RED` holds the cells the helpers still fail. The test fails when a
 * cell is red and not listed, and when a listed cell passes, so the list only
 * shrinks. `MEDIA_MATRIX_PRINT=1` prints the table.
 */
import { readdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { largeImageUrl, mediaKind, previewImageUrl } from '../src/app/media';
import { parseMedia } from '../src/api/media';
import type { Media, ServerMediaKind } from '../src/api/types';

interface Kind {
  readonly file: string;
  readonly mediaType: string;
}

const KINDS: readonly Kind[] = [
  { file: 'image.jpg', mediaType: 'image/jpeg' },
  { file: 'image.png', mediaType: 'image/png' },
  { file: 'image.gif', mediaType: 'image/gif' },
  { file: 'image.webp', mediaType: 'image/webp' },
  { file: 'image.tif', mediaType: 'image/tiff' },
  { file: 'image-cmyk.tif', mediaType: 'image/tiff' },
  { file: 'image-deep.tif', mediaType: 'image/tiff' },
  { file: 'image-pages.tif', mediaType: 'image/tiff' },
  { file: 'image.psd', mediaType: 'image/vnd.adobe.photoshop' },
  { file: 'image.eps', mediaType: 'application/postscript' },
  { file: 'image.nef', mediaType: 'image/x-nikon-nef' },
  { file: 'image.avif', mediaType: 'image/avif' },
  { file: 'image.heic', mediaType: 'image/heic' },
  { file: 'image.svg', mediaType: 'image/svg+xml' },
  { file: 'hostile.svg', mediaType: 'image/svg+xml' },
  { file: 'audio.mp3', mediaType: 'audio/mpeg' },
  { file: 'audio.m4a', mediaType: 'audio/mp4' },
  { file: 'audio.ogg', mediaType: 'audio/ogg' },
  { file: 'audio.opus', mediaType: 'audio/opus' },
  { file: 'audio.flac', mediaType: 'audio/flac' },
  { file: 'audio.wav', mediaType: 'audio/x-wav' },
  { file: 'wma.wma', mediaType: 'audio/x-ms-wma' },
  { file: 'video.mp4', mediaType: 'video/mp4' },
  { file: 'video.webm', mediaType: 'video/webm' },
  { file: 'video.mov', mediaType: 'video/quicktime' },
  { file: 'video.mkv', mediaType: 'video/x-matroska' },
  { file: 'video.ogv', mediaType: 'video/ogg' },
  { file: 'mjpeg.avi', mediaType: 'video/x-msvideo' },
  { file: 'mpeg2.mpg', mediaType: 'video/mpeg' },
  { file: 'doc.pdf', mediaType: 'application/pdf' },
  {
    file: 'doc.docx',
    mediaType: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  },
  { file: 'model.glb', mediaType: 'model/gltf-binary' },
  { file: 'model.gltf', mediaType: 'model/gltf+json' },
  { file: 'captions.vtt', mediaType: 'text/vtt' },
  { file: 'captions.srt', mediaType: 'application/x-subrip' },
];

/** `(corpus file, helper)` pairs the helpers still fail. */
const KNOWN_RED: ReadonlySet<string> = new Set([
]);

/**
 * The kind `MediaKind::of` gives each corpus file once the server has sniffed
 * it. Every image format now has a derivative or an access master, so the
 * server's word and the admin's own image table agree, HEIC and AVIF included.
 */
const SERVER_KINDS: Readonly<Record<string, ServerMediaKind>> = {
  'image.jpg': 'local-image',
  'image.png': 'local-image',
  'image.gif': 'local-image',
  'image.webp': 'local-image',
  'image.tif': 'local-image',
  'image-cmyk.tif': 'local-image',
  'image-deep.tif': 'local-image',
  'image-pages.tif': 'local-image',
  'image.psd': 'local-image',
  'image.eps': 'local-image',
  'image.nef': 'local-image',
  'image.avif': 'local-image',
  'image.heic': 'local-image',
  'image.svg': 'local-image',
  'hostile.svg': 'local-image',
  'audio.mp3': 'audio',
  'audio.m4a': 'audio',
  'audio.ogg': 'audio',
  'audio.opus': 'audio',
  'audio.flac': 'audio',
  'audio.wav': 'audio',
  'wma.wma': 'audio',
  'video.mp4': 'video',
  'video.webm': 'video',
  'video.mov': 'video',
  'video.mkv': 'video',
  'video.ogv': 'video',
  'mjpeg.avi': 'video',
  'mpeg2.mpg': 'video',
  'doc.pdf': 'pdf',
  'doc.docx': 'document',
  'model.glb': 'model-3d',
  'model.gltf': 'model-3d',
  'captions.vtt': 'captions',
  'captions.srt': 'captions',
};

const HELPERS: readonly (readonly [string, (media: Media) => string | null])[] = [
  ['previewImageUrl', previewImageUrl],
  ['largeImageUrl', largeImageUrl],
];

const IMAGE_EXTENSIONS: ReadonlySet<string> = new Set([
  'avif',
  'gif',
  'heic',
  'jpeg',
  'jpg',
  'png',
  'svg',
  'tif',
  'tiff',
  'webp',
]);

/** The corpus directory beside the checkout's `crates/` and `web/`. */
const corpusDir = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../../fixtures/media-kinds',
);

/** One Media as the API sends it, before a thumbnail exists. */
const media = (kind: Kind, serverKind: ServerMediaKind | null = null): Media =>
  parseMedia({
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
    ...(serverKind === null ? {} : { 'o:kind': serverKind }),
    'o:item': { '@id': 'http://localhost/api/items/1', 'o:id': 1 },
    'o:source': kind.file,
    'o:media_type': kind.mediaType,
    'o:sha256': null,
    'o:size': 1,
    'o:filename': `storage-${kind.file}`,
    'o:lang': null,
    'o:alt_text': null,
    'o:original_url': `http://localhost/files/original/${kind.file}`,
    'o:thumbnail_urls': {},
    data: [],
  });

/** A URL that serves an image: a derivative, an asset, or an image original. */
const isImageUrl = (url: string): boolean => {
  if (url.includes('/application/asset/thumbnails/')) {
    return true;
  }
  if (url.includes('/files/') && !url.includes('/files/original/')) {
    return true;
  }
  const name = url.split('?')[0] ?? url;
  const extension = name.split('.').pop()?.toLowerCase() ?? '';
  return IMAGE_EXTENSIONS.has(extension);
};

describe('media kind previews', () => {
  it('the fixture table names every corpus file', () => {
    const files = readdirSync(corpusDir)
      .filter((name) => !name.startsWith('.') && !['make.sh', 'README.md', 'BASELINE.md'].includes(name))
      .sort();
    expect(KINDS.map((kind) => kind.file).sort()).toEqual(files);
  });

  it('the server kind names every row, and the table agrees where the server reads the file', () => {
    expect(Object.keys(SERVER_KINDS).sort()).toEqual(KINDS.map((kind) => kind.file).sort());
    const failures: string[] = [];
    for (const kind of KINDS) {
      const fromServer = mediaKind(media(kind, SERVER_KINDS[kind.file]));
      const fromTable = mediaKind(media(kind));
      const expected = fromTable;
      if (fromServer !== expected) {
        failures.push(`${kind.file}: server ${fromServer}, table ${expected}`);
      }
    }
    expect(failures).toEqual([]);
  });

  it('a preview URL is an image or null', () => {
    const failures: string[] = [];
    const rows: string[] = [];
    for (const kind of KINDS) {
      const fixture = media(kind);
      const cells: string[] = [];
      for (const [name, helper] of HELPERS) {
        const url = helper(fixture);
        const passes = url === null || isImageUrl(url);
        const cell = `${kind.file} ${name}`;
        const listed = KNOWN_RED.has(cell);
        if (passes === listed) {
          failures.push(
            listed ? `${cell} now passes; remove it from KNOWN_RED` : `${cell} is red`,
          );
        }
        cells.push(passes ? 'pass' : '**fail**');
      }
      rows.push(`| ${kind.file} | ${cells.join(' | ')} |`);
    }
    const table = [
      `| kind | ${HELPERS.map(([name]) => name).join(' | ')} |`,
      `| --- | ${HELPERS.map(() => '---').join(' | ')} |`,
      ...rows,
    ].join('\n');
    if (failures.length > 0 || process.env['MEDIA_MATRIX_PRINT'] === '1') {
      console.log(table);
    }
    expect(failures).toEqual([]);
  });
});
