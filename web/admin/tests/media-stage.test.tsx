/**
 * The media stage: one browser control per kind, decided by the media the
 * server sent. Every case is the row of the stage table it covers, rendered
 * through the real component, so the element an editor meets is the assertion.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Mock } from 'vitest';
import { brandId } from '../src/api/parse';
import type { Media, MediaAccess, MediaPreview, ServerMediaKind } from '../src/api/types';
import { MediaStage } from '../src/pages/media/stage/stage';
import { recallPosition, rememberPosition } from '../src/pages/media/stage/play-position';
import { installFetch, jsonResponse, route, urlsFor } from './stub';

const ORIGINAL = 'http://localhost/files/original/7.bin';
const LARGE = 'http://localhost/files/large/7.jpg';
/** The media every card test stages: media 7. */
const PLAYER = '/api/romeka/media/7/player';

/** The player route's answer: the provider URL the server names, or none. */
const stubPlayer = (url: string | null): Mock<typeof fetch> =>
  installFetch(route({ [PLAYER]: () => jsonResponse({ url }) }));

interface MediaFields {
  readonly kind?: ServerMediaKind | null;
  readonly renderer?: string;
  readonly ingester?: string;
  readonly type?: string | null;
  readonly source?: string | null;
  readonly title?: string | null;
  readonly filename?: string | null;
  readonly originalUrl?: string | null;
  readonly altText?: string | null;
  readonly thumbnails?: Readonly<Record<string, string>>;
  readonly preview?: MediaPreview | null;
  readonly access?: MediaAccess | null;
}

const media = (fields: MediaFields = {}): Media => ({
  id: brandId<'MediaId'>(7),
  apiUrl: 'http://localhost/api/media/7',
  types: ['o:Media'],
  isPublic: true,
  owner: null,
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  title: fields.title === undefined ? null : fields.title,
  created: '2026-10-20T09:00:00+00:00',
  modified: null,
  thumbnailDisplayUrls: {},
  values: new Map(),
  ingester: fields.ingester ?? 'upload',
  renderer: fields.renderer ?? 'file',
  kind: fields.kind === undefined ? null : fields.kind,
  item: { id: brandId<'ItemId'>(1), apiUrl: 'http://localhost/api/items/1' },
  source: fields.source === undefined ? null : fields.source,
  mediaType: fields.type === undefined ? null : fields.type,
  sha256: null,
  size: 1024,
  filename: fields.filename === undefined ? 'stored.bin' : fields.filename,
  lang: null,
  altText: fields.altText === undefined ? null : fields.altText,
  originalUrl: fields.originalUrl === undefined ? ORIGINAL : fields.originalUrl,
  preview: fields.preview ?? null,
  access: fields.access ?? null,
  thumbnailUrls: fields.thumbnails ?? {},
});

afterEach(() => {
  window.sessionStorage.clear();
});

describe('a media the stage can show', () => {
  it('shows a raster image as a picture with its alt text', () => {
    render(
      <MediaStage
        media={media({
          type: 'image/jpeg',
          filename: 'photo.jpg',
          altText: 'A gull on a post',
          thumbnails: { large: LARGE },
        })}
      />,
    );

    const image = screen.getByRole('img', { name: 'A gull on a post' });
    expect(image).toHaveAttribute('src', LARGE);
    expect(document.querySelector('.media-stage-image.is-svg')).toBeNull();
    expect(screen.getByText('Image')).toBeInTheDocument();
  });

  it('shows an SVG from its own vector original, not a raster derivative', () => {
    render(
      <MediaStage
        media={media({
          type: 'image/svg+xml',
          filename: 'drawing.svg',
          originalUrl: ORIGINAL,
          thumbnails: { large: LARGE },
        })}
      />,
    );

    const image = screen.getByRole('img');
    expect(image).toHaveAttribute('src', ORIGINAL);
    expect(image).toHaveClass('is-svg');
  });

  it('plays an audio file in a native player that downloads no body first', () => {
    render(<MediaStage media={media({ type: 'audio/mpeg', filename: 'tone.mp3' })} />);

    const player = document.querySelector('audio');
    expect(player).not.toBeNull();
    expect(player).toHaveAttribute('controls');
    expect(player).toHaveAttribute('preload', 'metadata');
    expect(player).toHaveAttribute('src', ORIGINAL);
    expect(document.querySelector('video')).toBeNull();
  });

  it('plays a video file with the large derivative as its poster', () => {
    render(
      <MediaStage
        media={media({
          type: 'video/mp4',
          filename: 'clip.mp4',
          thumbnails: { large: LARGE },
        })}
      />,
    );

    const player = document.querySelector('video');
    expect(player).not.toBeNull();
    expect(player).toHaveAttribute('controls');
    expect(player).toHaveAttribute('preload', 'metadata');
    expect(player).toHaveAttribute('poster', LARGE);
    expect(player).toHaveAttribute('src', ORIGINAL);
  });

  it('plays the access copy of a video no browser opens, and keeps the original as the file', () => {
    const copy = 'http://localhost/files/access/7.mp4';
    render(
      <MediaStage
        media={media({
          type: 'video/x-msvideo',
          filename: 'reel.avi',
          thumbnails: { large: LARGE },
          access: {
            state: 'ready',
            reason: null,
            progress: null,
            url: copy,
            mediaType: 'video/mp4',
          },
        })}
      />,
    );

    expect(document.querySelector('video')).toHaveAttribute('src', copy);
    expect(screen.getByRole('link', { name: 'Download the file' })).toHaveAttribute(
      'href',
      ORIGINAL,
    );
    expect(screen.getByRole('link', { name: /^Original: AVI,/ })).toHaveAttribute(
      'href',
      ORIGINAL,
    );
  });

  it('plays the AAC copy of a WMA once ready, and no player while it is being made', () => {
    const copy = 'http://localhost/files/access/7.m4a';
    const wma = (access: MediaAccess | null): Media =>
      media({ type: 'audio/x-ms-wma', filename: 'talk.wma', access });

    const { unmount } = render(<MediaStage media={wma(null)} />);
    expect(document.querySelector('audio')).toHaveAttribute('src', ORIGINAL);
    unmount();

    const queued = render(
      <MediaStage
        media={wma({ state: 'pending', reason: null, progress: 40, url: null, mediaType: null })}
      />,
    );
    expect(document.querySelector('audio')).toBeNull();
    expect(document.querySelector('.media-stage-file')).not.toBeNull();
    queued.unmount();

    render(
      <MediaStage
        media={wma({ state: 'ready', reason: null, progress: null, url: copy, mediaType: 'audio/mp4' })}
      />,
    );
    expect(document.querySelector('audio')).toHaveAttribute('src', copy);
  });

  it('shows a PDF first page, and the browser viewer only after the press', () => {
    render(
      <MediaStage
        media={media({
          type: 'application/pdf',
          filename: 'doc.pdf',
          thumbnails: { large: LARGE },
        })}
      />,
    );

    expect(screen.getByRole('img')).toHaveAttribute('src', LARGE);
    expect(document.querySelector('object')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Read the PDF here' }));

    const viewer = document.querySelector('object');
    expect(viewer).not.toBeNull();
    expect(viewer).toHaveAttribute('type', 'application/pdf');
    expect(viewer).toHaveAttribute('data', ORIGINAL);
    expect(screen.queryByRole('button', { name: 'Read the PDF here' })).toBeNull();
    // The object's fallback and the line under it both offer the file.
    for (const link of screen.getAllByRole('link', { name: 'Open the PDF' })) {
      expect(link).toHaveAttribute('href', ORIGINAL);
    }
  });

  it('opens a document’s PDF copy in the reader and keeps the document as the download', () => {
    const copy = 'http://localhost/files/access/7.pdf';
    render(
      <MediaStage
        media={media({
          type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
          filename: 'notes.docx',
          thumbnails: { large: LARGE },
          access: {
            state: 'ready',
            reason: null,
            progress: null,
            url: copy,
            mediaType: 'application/pdf',
          },
        })}
      />,
    );

    expect(screen.getByRole('img')).toHaveAttribute('src', LARGE);
    expect(screen.getByRole('link', { name: /^Original: DOCX,/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Read the PDF here' }));

    const viewer = document.querySelector('object');
    expect(viewer).not.toBeNull();
    expect(viewer).toHaveAttribute('type', 'application/pdf');
    expect(viewer).toHaveAttribute('data', copy);
    // The document, never the PDF copy, is what the page downloads.
    const downloads = screen.getAllByRole('link', { name: 'Download the file' });
    expect(downloads.length).toBeGreaterThan(0);
    for (const link of downloads) {
      expect(link).toHaveAttribute('href', ORIGINAL);
    }
    expect(screen.queryByRole('link', { name: 'Open the PDF' })).toBeNull();
  });

  it('shows a caption file as its first 40 lines of text', async () => {
    const lines = Array.from({ length: 60 }, (_value, index) => `line ${String(index + 1)}`);
    installFetch(
      () =>
        new Response(lines.join('\n'), {
          status: 200,
          headers: { 'Content-Type': 'text/vtt' },
        }),
    );

    render(<MediaStage media={media({ type: 'text/vtt', filename: 'captions.vtt' })} />);

    await waitFor(() => {
      expect(document.querySelector('pre')?.textContent).toContain('line 40');
    });
    expect(document.querySelector('pre')?.textContent).not.toContain('line 41');
  });
});

describe('a media the stage cannot show', () => {
  it('names a 3D model as a file to download, never a viewer', () => {
    render(
      <MediaStage media={media({ type: 'model/gltf-binary', filename: 'chair.glb' })} />,
    );

    expect(screen.queryByRole('img')).toBeNull();
    expect(document.querySelector('model-viewer')).toBeNull();
    expect(screen.getByText('chair.glb')).toBeInTheDocument();
    expect(screen.getByText('3D model')).toBeInTheDocument();
    expect(screen.getByText('1,024 bytes')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Download the file' })).toHaveAttribute(
      'href',
      ORIGINAL,
    );
  });

  it('names a document as a file to download', () => {
    render(
      <MediaStage
        media={media({
          type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
          filename: 'notes.docx',
        })}
      />,
    );

    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText('notes.docx')).toBeInTheDocument();
    expect(screen.getByText('Document')).toBeInTheDocument();
    expect(screen.getByText('DOCX')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Download the file' })).toHaveAttribute(
      'href',
      ORIGINAL,
    );
  });

  it('names an unclassified file as a file to download', () => {
    render(<MediaStage media={media({ type: 'application/zip', filename: 'box.zip' })} />);

    expect(screen.getByText('box.zip')).toBeInTheDocument();
    expect(screen.getByText('File')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Download the file' })).toBeInTheDocument();
  });

  it('offers the source when an image has no drawable file', () => {
    render(
      <MediaStage
        media={media({
          kind: 'iiif-image',
          type: 'image/jpeg',
          renderer: 'iiif',
          ingester: 'iiif',
          filename: null,
          source: 'https://iiif.example.org/1/info.json',
          originalUrl: null,
        })}
      />,
    );

    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText('No preview.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open the source' })).toHaveAttribute(
      'href',
      'https://iiif.example.org/1/info.json',
    );
  });

  it('names the host of a YouTube media, and loads nothing before the press', () => {
    const source = 'https://www.youtube.com/watch?v=abc123';
    const fetchMock = stubPlayer('https://www.youtube-nocookie.com/embed/abc123');
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'youtube',
          ingester: 'youtube',
          type: null,
          filename: null,
          originalUrl: null,
          source,
        })}
      />,
    );

    expect(document.querySelector('iframe')).toBeNull();
    expect(screen.getByText(/is a page on www\.youtube\.com/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Load the embed' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open on www.youtube.com' })).toHaveAttribute(
      'href',
      source,
    );
    expect(urlsFor(fetchMock, PLAYER)).toEqual([]);
  });

  it("frames a YouTube media's own player, with the attributes of a provider frame", async () => {
    const source = 'https://www.youtube.com/watch?v=abc123';
    const player = 'https://www.youtube-nocookie.com/embed/abc123?start=30';
    const fetchMock = stubPlayer(player);
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'youtube',
          ingester: 'youtube',
          type: null,
          filename: null,
          originalUrl: null,
          source,
        })}
      />,
    );

    expect(urlsFor(fetchMock, PLAYER)).toEqual([]);
    fireEvent.click(screen.getByRole('button', { name: 'Load the embed' }));

    await waitFor(() => {
      expect(document.querySelector('iframe')).not.toBeNull();
    });
    const frame = document.querySelector('iframe');
    expect(frame).toHaveAttribute('src', player);
    expect(frame).toHaveAttribute(
      'sandbox',
      'allow-scripts allow-same-origin allow-presentation allow-popups',
    );
    expect(frame).toHaveAttribute('allow', 'fullscreen; picture-in-picture');
    expect(frame).toHaveAttribute('referrerpolicy', 'strict-origin-when-cross-origin');
    expect(frame).toHaveAccessibleName('The embed from www.youtube.com');
    expect(screen.getByText('Playing from www.youtube-nocookie.com')).toBeInTheDocument();
    expect(screen.queryByText(/Nothing loads from it until you open it/)).toBeNull();
    expect(urlsFor(fetchMock, PLAYER)).toHaveLength(1);
    expect(screen.getByRole('link', { name: 'Open on www.youtube.com' })).toHaveAttribute(
      'href',
      source,
    );
  });

  it('keeps a YouTube card and its link when the server names no player', async () => {
    const source = 'https://www.youtube.com/watch?v=abc123';
    stubPlayer(null);
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'youtube',
          ingester: 'youtube',
          type: null,
          filename: null,
          originalUrl: null,
          source,
        })}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load the embed' }));

    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Load the embed' })).toBeNull();
    });
    expect(document.querySelector('iframe')).toBeNull();
    expect(screen.getByText(/is a page on www\.youtube\.com/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open on www.youtube.com' })).toHaveAttribute(
      'href',
      source,
    );
  });

  it('frames the server render of an HTML media only after the press', async () => {
    stubPlayer(null);
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'html',
          ingester: 'html',
          type: null,
          filename: null,
          originalUrl: null,
        })}
      />,
    );

    expect(document.querySelector('iframe')).toBeNull();
    expect(
      screen.getByText(/HTML that the server renders. Nothing loads until you open it\./),
    ).toBeInTheDocument();
    expect(screen.queryByRole('link')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Load the embed' }));

    await waitFor(() => {
      expect(document.querySelector('iframe')).not.toBeNull();
    });
    const frame = document.querySelector('iframe');
    expect(frame).toHaveAttribute('src', '/api/romeka/media/7/render');
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts allow-presentation');
    expect(frame?.getAttribute('sandbox')).not.toContain('allow-same-origin');
    expect(frame).toHaveAccessibleName('The rendered HTML');
    expect(screen.queryByRole('button', { name: 'Load the embed' })).toBeNull();
  });

  it('names the host of an oEmbed media, keeps the link, and frames its render on the press', async () => {
    const source = 'https://www.youtube.com/watch?v=abc123';
    stubPlayer(null);
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'oembed',
          ingester: 'oembed',
          type: null,
          filename: null,
          originalUrl: null,
          source,
        })}
      />,
    );

    expect(document.querySelector('iframe')).toBeNull();
    expect(screen.getByText(/is a page on www\.youtube\.com/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open on www.youtube.com' })).toHaveAttribute(
      'href',
      source,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load the embed' }));

    await waitFor(() => {
      expect(document.querySelector('iframe')).not.toBeNull();
    });
    const frame = document.querySelector('iframe');
    expect(frame).toHaveAttribute('src', '/api/romeka/media/7/render');
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts allow-presentation');
    expect(frame).toHaveAccessibleName('The embed from www.youtube.com');
    expect(screen.getByText('The embed from www.youtube.com')).toBeInTheDocument();
    expect(screen.queryByText(/Nothing loads from it until you open it/)).toBeNull();
    expect(screen.getByRole('link', { name: 'Open on www.youtube.com' })).toHaveAttribute(
      'href',
      source,
    );
  });

  it("frames the provider's own player directly when the server names one", async () => {
    const source = 'https://vimeo.com/76979871';
    const player = 'https://player.vimeo.com/video/76979871?h=abc';
    const fetchMock = stubPlayer(player);
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'oembed',
          ingester: 'oembed',
          type: null,
          filename: null,
          originalUrl: null,
          source,
        })}
      />,
    );

    // The facade holds: the card contacts its own server only on the press.
    expect(urlsFor(fetchMock, PLAYER)).toEqual([]);

    fireEvent.click(screen.getByRole('button', { name: 'Load the embed' }));

    await waitFor(() => {
      expect(document.querySelector('iframe')).not.toBeNull();
    });
    const frame = document.querySelector('iframe');
    expect(frame).toHaveAttribute('src', player);
    expect(frame).toHaveAttribute(
      'sandbox',
      'allow-scripts allow-same-origin allow-presentation allow-popups',
    );
    expect(frame).toHaveAttribute('allow', 'fullscreen; picture-in-picture');
    expect(frame).toHaveAttribute('referrerpolicy', 'strict-origin-when-cross-origin');
    expect(frame).toHaveAccessibleName('The embed from vimeo.com');
    expect(urlsFor(fetchMock, PLAYER)).toHaveLength(1);
    expect(screen.getByRole('link', { name: 'Open on vimeo.com' })).toHaveAttribute(
      'href',
      source,
    );
  });

  it('keeps the strict frame when the answer is not an https player', async () => {
    stubPlayer('http://player.vimeo.com/video/76979871');
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'oembed',
          ingester: 'oembed',
          type: null,
          filename: null,
          originalUrl: null,
          source: 'https://vimeo.com/76979871',
        })}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load the embed' }));

    await waitFor(() => {
      expect(document.querySelector('iframe')).not.toBeNull();
    });
    const frame = document.querySelector('iframe');
    expect(frame).toHaveAttribute('src', '/api/romeka/media/7/render');
    expect(frame).toHaveAttribute('sandbox', 'allow-scripts allow-presentation');
  });

  it('says an outside page with no address has no stored file, instead of an empty box', () => {
    render(
      <MediaStage
        media={media({
          kind: 'embed',
          renderer: 'youtube',
          ingester: 'youtube',
          type: null,
          filename: null,
          originalUrl: null,
        })}
      />,
    );

    expect(screen.getByText('This media has no stored file.')).toBeInTheDocument();
    expect(screen.queryByRole('img')).toBeNull();
    expect(document.querySelector('iframe')).toBeNull();
  });

  it('opens a IIIF manifest in the server viewer, and names its host', () => {
    const source = 'https://iiif.example.org/museum/manifest';
    render(
      <MediaStage
        media={media({
          kind: 'iiif-presentation',
          renderer: 'iiif_presentation',
          ingester: 'iiif_presentation',
          type: null,
          filename: null,
          originalUrl: null,
          source,
        })}
      />,
    );

    expect(screen.getByText(/manifest from iiif\.example\.org/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open in the IIIF viewer' })).toHaveAttribute(
      'href',
      `/iiif-viewer?url=${encodeURIComponent(source)}`,
    );
    expect(screen.getByRole('link', { name: 'Open the manifest' })).toHaveAttribute('href', source);
  });
});

describe('every kind that is not an image', () => {
  it('shows no <img> whose source is a non-image file', () => {
    const kinds: readonly MediaFields[] = [
      { type: 'audio/mpeg', filename: 'tone.mp3' },
      { type: 'video/mp4', filename: 'clip.mp4' },
      { type: 'application/pdf', filename: 'doc.pdf' },
      { type: 'model/gltf-binary', filename: 'chair.glb' },
      { type: 'text/vtt', filename: 'captions.vtt' },
      { type: 'application/msword', filename: 'notes.doc' },
      { type: 'application/zip', filename: 'box.zip' },
    ];
    for (const fields of kinds) {
      const { unmount } = render(<MediaStage media={media(fields)} />);
      const pictures = Array.from(document.querySelectorAll('img'));
      const wrong = pictures.flatMap((picture) => {
        const src = picture.getAttribute('src');
        return src === ORIGINAL ? [src] : [];
      });
      expect(wrong, fields.type ?? 'no type').toEqual([]);
      unmount();
    }
  });
});

describe('the position a player stopped at', () => {
  it('comes back for the same media in this tab, to a tenth of a second', () => {
    rememberPosition(7, 12.34);
    expect(recallPosition(7)).toBe(12.3);
    expect(recallPosition(8)).toBeNull();
  });

  it('is absent when the stored value is not a time', () => {
    window.sessionStorage.setItem('romeka.media.position.7', 'soon');
    expect(recallPosition(7)).toBeNull();
    window.sessionStorage.setItem('romeka.media.position.7', '-3');
    expect(recallPosition(7)).toBeNull();
  });
});
