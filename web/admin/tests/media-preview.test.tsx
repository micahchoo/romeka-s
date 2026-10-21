/**
 * The derivative state on a media document: `o-module-romeka:preview` reads into
 * the entity, a pending preview says "Making preview" where the thumbnail is
 * missing, and a failed one carries the tool's reason.
 */
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { brandId, parseMedia } from '../src/api/parse';
import type { Media, MediaPreview } from '../src/api/types';
import { MediaImage, MediaPreviewState } from '../src/app/media-image';

const media = (preview: MediaPreview | null, kind: 'image' | 'video' = 'video'): Media => ({
  id: brandId<'MediaId'>(7),
  apiUrl: 'http://localhost/api/media/7',
  types: ['o:Media'],
  isPublic: true,
  owner: null,
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  title: 'A reel',
  created: '2026-10-21T09:00:00+00:00',
  modified: null,
  thumbnailDisplayUrls: {},
  values: new Map(),
  ingester: 'upload',
  renderer: 'file',
  kind: kind === 'image' ? 'local-image' : 'video',
  item: { id: brandId<'ItemId'>(1), apiUrl: 'http://localhost/api/items/1' },
  source: 'reel.mp4',
  mediaType: kind === 'image' ? 'image/jpeg' : 'video/mp4',
  sha256: null,
  size: 1024,
  filename: 'reel.mp4',
  lang: null,
  altText: null,
  originalUrl: 'http://localhost/files/original/7.mp4',
  preview,
  thumbnailUrls: {},
});

describe('a pending or failed preview', () => {
  it('says the preview is being made where the thumbnail would be', () => {
    render(<MediaImage media={media({ state: 'pending', reason: null, progress: null })} url={null} className="preview" />);
    expect(screen.getByText('Making preview')).toBeTruthy();
  });

  it('gives a small box the same words as its title', () => {
    render(
      <MediaImage
        media={media({ state: 'pending', reason: null, progress: 40 })}
        url={null}
        className="preview"
        compact
      />,
    );
    expect(screen.queryByText('Making preview')).toBeNull();
    expect(screen.getByTitle(/Making preview/)).toBeTruthy();
  });

  it('shows the tool reason a failed preview carries', () => {
    render(
      <MediaPreviewState
        media={media({ state: 'failed', reason: 'the pdftoppm command failed: exit status 1', progress: null })}
      />,
    );
    expect(
      screen.getByText('The preview could not be made: the pdftoppm command failed: exit status 1'),
    ).toBeTruthy();
  });

  it('says nothing for a ready preview or a server without the member', () => {
    const { container, rerender } = render(
      <MediaPreviewState media={media({ state: 'ready', reason: null, progress: null })} />,
    );
    expect(container.textContent).toBe('');
    rerender(<MediaPreviewState media={media(null)} />);
    expect(container.textContent).toBe('');
  });
});

describe('the preview member at the API boundary', () => {
  const document = (preview: unknown): unknown => ({
    '@context': '/api-context',
    '@id': 'http://localhost/api/media/7',
    '@type': ['o:Media'],
    'o:id': 7,
    'o:is_public': true,
    'o:owner': null,
    'o:resource_class': null,
    'o:resource_template': null,
    'o:thumbnail': null,
    'o:title': 'A reel',
    'thumbnail_display_urls': { large: null, medium: null, square: null },
    'o:created': { '@value': '2026-10-21T09:00:00+00:00', '@type': 'http://www.w3.org/2001/XMLSchema#dateTime' },
    'o:modified': null,
    'o:ingester': 'upload',
    'o:renderer': 'file',
    'o:item': { '@id': 'http://localhost/api/items/1', 'o:id': 1 },
    'o:source': 'reel.mp4',
    'o:media_type': 'video/mp4',
    'o:kind': 'video',
    'o:sha256': null,
    'o:size': 1024,
    'o:filename': 'reel.mp4',
    'o:lang': null,
    'o:alt_text': null,
    'o:original_url': 'http://localhost/files/original/7.mp4',
    'o-module-romeka:preview': preview,
    'o:thumbnail_urls': { large: null, medium: null, square: null },
    'data': [],
  });

  it('reads the state, the reason and the progress', () => {
    const parsed = parseMedia(
      document({ state: 'failed', reason: 'toolMissing: ffmpeg', progress: 40 }),
    );
    expect(parsed.preview).toEqual({
      state: 'failed',
      reason: 'toolMissing: ffmpeg',
      progress: 40,
    });
    expect(parsed.values.size).toBe(0);
  });

  it('reads the pending member the server sends, which omits reason and progress', () => {
    const parsed = parseMedia(document({ state: 'pending' }));
    expect(parsed.preview).toEqual({ state: 'pending', reason: null, progress: null });
  });

  it('reads an absent member, and a state this build does not know, as null', () => {
    expect(parseMedia(document(undefined)).preview).toBeNull();
    expect(parseMedia(document({ state: 'making-tea' })).preview).toBeNull();
  });
});
