/**
 * The media kind and what follows from it: the image a preview may show, the
 * media an image-only picker offers, and the placeholder a list or a card shows
 * when there is no picture. Every case is built as the API boundary hands a row
 * over, and the pages are driven through the real client and parsers.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import type { Media, ServerMediaKind } from '../src/api/types';
import {
  fileExtension,
  imagePreviewUrl,
  kindOfFile,
  largeImageUrl,
  mediaKind,
  previewImageUrl,
  previewLinkUrl,
} from '../src/app/media';
import { MediaImage } from '../src/app/media-image';
import { MediaPage } from '../src/pages/media/page';
import { loadFixture } from './loadFixture';
import { installFetch, listResponse, route } from './stub';

interface FileFields {
  readonly renderer?: string;
  readonly ingester?: string;
  readonly type?: string | null;
  readonly filename?: string | null;
  readonly source?: string | null;
  readonly originalUrl?: string | null;
  readonly thumbnails?: Readonly<Record<string, string>>;
  readonly kind?: ServerMediaKind | null;
}

const ORIGINAL = 'http://localhost/files/original/7.jpg';
const THUMBNAIL = 'http://localhost/files/square/7.jpg';

const media = (file: FileFields = {}): Media => ({
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
  ingester: file.ingester ?? 'upload',
  renderer: file.renderer ?? 'file',
  kind: file.kind === undefined ? null : file.kind,
  item: { id: brandId<'ItemId'>(1), apiUrl: 'http://localhost/api/items/1' },
  source: file.source === undefined ? null : file.source,
  mediaType: file.type === undefined ? null : file.type,
  sha256: null,
  size: 1024,
  filename: file.filename === undefined ? null : file.filename,
  lang: null,
  altText: null,
  originalUrl: file.originalUrl === undefined ? ORIGINAL : file.originalUrl,
  preview: null,
  access: null,
  thumbnailUrls: file.thumbnails ?? {},
});

describe('the kind of a media', () => {
  it('reads a file from its media type', () => {
    const cases: readonly (readonly [FileFields, string])[] = [
      [{ type: 'image/jpeg', filename: 'photo.jpg' }, 'image'],
      [{ type: 'image/tiff', filename: 'scan.tif' }, 'image'],
      [{ type: 'audio/mpeg', filename: 'tone.mp3' }, 'audio'],
      [{ type: 'video/mp4', filename: 'clip.mp4' }, 'video'],
      [{ type: 'application/pdf', filename: 'report.pdf' }, 'pdf'],
      [{ type: 'application/msword', filename: 'notes.doc' }, 'document'],
      [
        {
          type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
          filename: 'notes.docx',
        },
        'document',
      ],
      [{ type: 'model/gltf-binary', filename: 'chair.glb' }, 'model'],
      [{ type: 'text/vtt', filename: 'captions.vtt' }, 'captions'],
      [{ type: 'application/zip', filename: 'bundle.zip' }, 'other'],
    ];
    const wrong = cases
      .map(([file, kind]) => [mediaKind(media(file)), kind, file.type ?? 'no type'] as const)
      .filter(([found, kind]) => found !== kind);
    expect(wrong).toEqual([]);
  });

  it('reads a generic type from the file extension', () => {
    expect(mediaKind(media({ type: 'application/octet-stream', filename: 'tone.flac' }))).toBe(
      'audio',
    );
    expect(mediaKind(media({ type: 'application/json', filename: 'chair.gltf' }))).toBe('model');
    expect(mediaKind(media({ type: 'text/plain', filename: 'captions.vtt' }))).toBe('captions');
    expect(mediaKind(media({ type: 'text/plain', filename: 'captions.srt' }))).toBe('captions');
    expect(mediaKind(media({ type: null, source: 'https://example.org/a/report.pdf' }))).toBe(
      'pdf',
    );
  });

  it('reads an outside player and a manifest from the renderer or the ingester', () => {
    expect(mediaKind(media({ renderer: 'youtube', ingester: 'youtube', type: null }))).toBe('embed');
    expect(mediaKind(media({ renderer: 'oembed', ingester: 'oembed', type: null }))).toBe('embed');
    expect(mediaKind(media({ renderer: 'html', ingester: 'html', type: null }))).toBe('embed');
    expect(
      mediaKind(media({ renderer: 'iiif_presentation', ingester: 'iiif_presentation', type: null })),
    ).toBe('manifest');
    // A YouTube poster frame is a JPEG the player shows, not an image of the item.
    expect(mediaKind(media({ renderer: 'youtube', type: 'image/jpeg' }))).toBe('embed');
    expect(mediaKind(media({ renderer: 'iiif', type: 'image/jpeg' }))).toBe('image');
  });

  it('leaves a row nothing names as other', () => {
    expect(mediaKind(media({ type: 'application/octet-stream', filename: 'blob.bin' }))).toBe(
      'other',
    );
    expect(mediaKind(media({ type: null }))).toBe('other');
  });

  it('ignores case in the type', () => {
    expect(mediaKind(media({ type: 'AUDIO/MPEG', filename: 'tone.MP3' }))).toBe('audio');
  });
});

describe('a file the curator is about to send', () => {
  it('names its kind from the extension, whatever case it uses', () => {
    expect(kindOfFile('march.MOV', '')).toBe('video');
    expect(kindOfFile('scan.PDF', '')).toBe('pdf');
    expect(kindOfFile('captions.VTT', '')).toBe('captions');
    expect(kindOfFile('chair.GLB', '')).toBe('model');
    expect(kindOfFile('photo.JPG', '')).toBe('image');
    expect(kindOfFile('notes.DOCX', '')).toBe('document');
  });

  it('reads the type when the name has no extension', () => {
    expect(kindOfFile('captions', 'text/vtt')).toBe('captions');
    expect(kindOfFile('tone', 'audio/mpeg; charset=binary')).toBe('audio');
    expect(kindOfFile('thing', 'application/octet-stream')).toBe('other');
    expect(kindOfFile('no-extension', '')).toBe('other');
  });
});

describe('a IIIF manifest', () => {
  it('reads the server kind and names it in its own word', () => {
    const manifest = media({
      kind: 'iiif-presentation',
      renderer: 'iiif_presentation',
      ingester: 'iiif_presentation',
      type: null,
    });
    expect(mediaKind(manifest)).toBe('manifest');

    render(<MediaImage media={manifest} url={null} className="media-card-image" />);
    expect(screen.getByText('IIIF manifest')).toBeInTheDocument();
  });
});

describe('the file extension a placeholder names', () => {
  it('comes from the stored file, upper case', () => {
    expect(fileExtension(media({ filename: 'a1b2.mp3', source: 'tone.mp3' }))).toBe('MP3');
    expect(fileExtension(media({ filename: 'stored.docx' }))).toBe('DOCX');
  });

  it('comes from the source when the row stores no file name', () => {
    expect(fileExtension(media({ source: 'https://example.org/a/report.pdf?download=1' }))).toBe(
      'PDF',
    );
  });

  it('is absent for a media an outside service renders', () => {
    expect(
      fileExtension(media({ renderer: 'iiif', source: 'https://example.org/iiif/1/info.json' })),
    ).toBeNull();
    expect(fileExtension(media({ renderer: 'html', ingester: 'html' }))).toBeNull();
  });

  it('is absent when the name has none', () => {
    expect(fileExtension(media({ filename: 'no-extension' }))).toBeNull();
  });
});

describe('the image a preview shows', () => {
  it('falls back to the original of an image the browser draws', () => {
    for (const type of [
      'image/jpeg',
      'image/png',
      'image/gif',
      'image/webp',
      'image/avif',
      'image/svg+xml',
    ]) {
      expect(previewImageUrl(media({ type })), type).toBe(ORIGINAL);
      expect(largeImageUrl(media({ type })), type).toBe(ORIGINAL);
    }
  });

  it('prefers a thumbnail whichever kind the media is', () => {
    const thumbnails = {
      square: THUMBNAIL,
      large: 'http://localhost/files/large/7.jpg',
    };
    expect(previewImageUrl(media({ type: 'audio/mpeg', thumbnails }))).toBe(THUMBNAIL);
    expect(largeImageUrl(media({ type: 'audio/mpeg', thumbnails }))).toBe(
      'http://localhost/files/large/7.jpg',
    );
  });

  it('shows no image for a file the browser cannot draw', () => {
    for (const file of [
      { type: 'audio/mpeg', filename: 'tone.mp3' },
      { type: 'video/mp4', filename: 'clip.mp4' },
      { type: 'application/pdf', filename: 'report.pdf' },
      { type: 'image/tiff', filename: 'scan.tif' },
      { type: 'image/heic', filename: 'photo.heic' },
    ]) {
      expect(previewImageUrl(media(file)), file.type).toBeNull();
      expect(largeImageUrl(media(file)), file.type).toBeNull();
    }
  });

  it('still links to the original of a file it cannot preview', () => {
    const audio = media({
      type: 'audio/mpeg',
      filename: 'tone.mp3',
      originalUrl: 'http://localhost/files/original/tone.mp3',
    });
    expect(previewLinkUrl(audio)).toBe('http://localhost/files/original/tone.mp3');
  });

  it('offers an image picker only the media whose kind is an image', () => {
    expect(imagePreviewUrl(media({ type: 'image/jpeg' }))).toBe(ORIGINAL);
    expect(imagePreviewUrl(media({ type: 'image/tiff', filename: 'scan.tif' }))).toBeNull();
    expect(imagePreviewUrl(media({ type: 'audio/mpeg' }))).toBeNull();
    expect(
      imagePreviewUrl(media({ type: 'video/mp4', thumbnails: { square: THUMBNAIL } })),
    ).toBeNull();
  });
});

describe('a media with no picture', () => {
  it('names the kind and the file where an image would be', () => {
    render(
      <MediaImage
        media={media({ type: 'audio/mpeg', filename: 'tone.mp3' })}
        url={null}
        className="media-card-image"
      />,
    );

    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText('Audio')).toBeInTheDocument();
    expect(screen.getByText('MP3')).toBeInTheDocument();
    expect(document.querySelector('[data-kind="audio"]')).not.toBeNull();
  });

  it('keeps its words for a screen reader in a box too small to show them', () => {
    render(
      <MediaImage
        media={media({ type: 'application/pdf', filename: 'report.pdf' })}
        url={null}
        className="media-preview"
        compact
      />,
    );

    expect(screen.getByText('PDF')).toBeInTheDocument();
    expect(document.querySelector('[data-kind="pdf"]')).toHaveAttribute('title', 'PDF');
  });

  it('shows the image with its name when there is one', () => {
    render(
      <MediaImage
        media={{ ...media({ type: 'image/jpeg', source: 'photo.jpg' }), title: 'A photo' }}
        url={ORIGINAL}
        className="media-card-image"
      />,
    );

    expect(screen.getByRole('img', { name: 'A photo' })).toHaveAttribute('src', ORIGINAL);
  });
});

describe('a picture of a media that is not an image', () => {
  it('keeps the picture and marks the kind on it', () => {
    render(
      <MediaImage
        media={media({ type: 'video/mp4', filename: 'clip.mp4' })}
        url={THUMBNAIL}
        className="media-card-image"
      />,
    );

    expect(screen.getByRole('img')).toHaveAttribute('src', THUMBNAIL);
    expect(document.querySelector('[data-kind="video"]')).not.toBeNull();
    expect(screen.getByText('Video')).toBeInTheDocument();
  });

  it('leaves a picture of an image unmarked', () => {
    render(
      <MediaImage
        media={media({ type: 'image/jpeg', filename: 'photo.jpg' })}
        url={THUMBNAIL}
        className="media-card-image"
      />,
    );

    expect(screen.getByRole('img')).toHaveAttribute('src', THUMBNAIL);
    expect(document.querySelector('.media-image-badge')).toBeNull();
    expect(screen.queryByText('Image')).toBeNull();
  });
});

describe('the media list', () => {
  const kindsHandler = route({
    '/api/media': () => listResponse(loadFixture('media-page-kinds.json'), 3),
  });

  it('shows an image for the image row and a kind placeholder for the rest', async () => {
    installFetch(kindsHandler);
    render(<MediaPage param={null} />);

    const photo = await screen.findByRole('row', { name: /photo\.jpg/ });
    expect(within(photo).getByRole('img')).toHaveAttribute(
      'src',
      'http://localhost/files/original/photo.jpg',
    );
    expect(within(photo).getAllByRole('cell')[1]).toHaveTextContent('Image');

    const audio = screen.getByRole('row', { name: /tone\.mp3/ });
    expect(within(audio).queryByRole('img')).toBeNull();
    expect(within(audio).getAllByRole('cell')[0]).toHaveTextContent('Audio');
    expect(within(audio).getAllByRole('cell')[1]).toHaveTextContent('Audio');
    expect(within(audio).getByText('MP3')).toBeInTheDocument();

    const pdf = screen.getByRole('row', { name: /report\.pdf/ });
    expect(within(pdf).queryByRole('img')).toBeNull();
    expect(within(pdf).getAllByRole('cell')[0]).toHaveTextContent('PDF');
    expect(within(pdf).getAllByRole('cell')[1]).toHaveTextContent('PDF');

    expect(screen.getAllByRole('img')).toHaveLength(1);
  });
});
