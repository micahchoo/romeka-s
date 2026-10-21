/**
 * The access copy on the media page and the media list: what a reader sees
 * while a copy is being made, after it failed, when the install lacks its tool,
 * and when it is ready. Every case renders the component the page renders, so
 * the words and the Retry button are the assertion.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { brandId } from '../src/api/parse';
import type { Media, MediaAccess, ServerMediaKind, User } from '../src/api/types';
import { mayEditMedia } from '../src/app/media';
import { MediaAccessCue, MediaAccessLine } from '../src/pages/media/access';
import { MediaStage } from '../src/pages/media/stage/stage';

const ORIGINAL = 'http://localhost/files/original/7.avi';

const media = (fields: {
  readonly kind?: ServerMediaKind;
  readonly access?: MediaAccess;
  readonly type?: string;
  readonly filename?: string;
  readonly size?: number | null;
  readonly owner?: User['id'] | null;
  readonly originalUrl?: string | null;
}): Media => ({
  id: brandId<'MediaId'>(7),
  apiUrl: 'http://localhost/api/media/7',
  types: ['o:Media'],
  isPublic: true,
  owner:
    fields.owner === undefined
      ? null
      : fields.owner === null
        ? null
        : { id: fields.owner, apiUrl: `http://localhost/api/users/${String(fields.owner)}` },
  resourceClass: null,
  resourceTemplate: null,
  thumbnail: null,
  title: null,
  created: '2026-10-21T09:00:00+00:00',
  modified: null,
  thumbnailDisplayUrls: {},
  values: new Map(),
  ingester: 'upload',
  renderer: 'file',
  kind: fields.kind ?? null,
  item: { id: brandId<'ItemId'>(1), apiUrl: 'http://localhost/api/items/1' },
  source: null,
  mediaType: fields.type ?? 'video/x-msvideo',
  sha256: null,
  size: fields.size === undefined ? 489_684_992 : fields.size,
  filename: fields.filename ?? 'reel.avi',
  lang: null,
  altText: null,
  originalUrl: fields.originalUrl === undefined ? ORIGINAL : fields.originalUrl,
  preview: null,
  access: fields.access ?? null,
  thumbnailUrls: {},
});

const access = (fields: Partial<MediaAccess>): MediaAccess => ({
  state: 'pending',
  reason: null,
  progress: null,
  url: null,
  mediaType: null,
  ...fields,
});

const user = (role: string, id = 3): User =>
  ({
    id: brandId<'UserId'>(id),
    apiUrl: `http://localhost/api/users/${String(id)}`,
    types: ['o:User'],
    name: 'Sam',
    email: 'sam@example.org',
    created: '2026-10-21T09:00:00+00:00',
    role,
    isActive: true,
    localeSetting: null,
    lessons: { hidden: false, seen: [] },
    adminLocale: null,
  });

afterEach(() => {
  vi.restoreAllMocks();
});

describe('a copy being made', () => {
  it('names the playable copy and its progress', () => {
    render(
      <MediaAccessLine
        media={media({ kind: 'video', access: access({ state: 'pending', progress: 40 }) })}
        canRetry={false}
      />,
    );

    expect(screen.getByRole('status')).toHaveTextContent('Making a playable copy, 40%');
  });

  it('names a full-size image and a PDF copy by what they are', () => {
    const { unmount } = render(
      <MediaAccessLine
        media={media({ kind: 'local-image', type: 'image/tiff', access: access({ state: 'pending' }) })}
        canRetry={false}
      />,
    );
    expect(screen.getByRole('status')).toHaveTextContent('Making a full-size image');
    unmount();

    render(
      <MediaAccessLine
        media={media({ kind: 'document', type: 'application/msword', access: access({ state: 'pending' }) })}
        canRetry={false}
      />,
    );
    expect(screen.getByRole('status')).toHaveTextContent('Making a PDF to read');
  });

  it('shows a compact cue in a media list', () => {
    const { unmount } = render(
      <MediaAccessCue
        media={media({ kind: 'video', access: access({ state: 'pending' }) })}
      />,
    );
    expect(screen.getByText('Making a playable copy')).toBeInTheDocument();
    unmount();

    render(
      <MediaAccessCue media={media({ kind: 'video', access: access({ state: 'ready' }) })} />,
    );
    expect(screen.queryByText('Making a playable copy')).toBeNull();
  });
});

describe('a copy that failed', () => {
  it('states the reason and offers Retry to a reader who may edit the media', () => {
    const retry = vi.fn();
    render(
      <MediaAccessLine
        media={media({
          kind: 'video',
          access: access({ state: 'failed', reason: 'the ffmpeg command failed: exit status 1' }),
        })}
        canRetry
        onRetry={retry}
      />,
    );

    expect(screen.getByRole('status')).toHaveTextContent(
      'The copy could not be made: the ffmpeg command failed: exit status 1',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(retry).toHaveBeenCalledTimes(1);
  });

  it('offers no Retry when the reader may not edit the media', () => {
    render(
      <MediaAccessLine
        media={media({ kind: 'video', access: access({ state: 'failed', reason: 'ffmpeg failed' }) })}
        canRetry={false}
      />,
    );
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
  });
});

describe('a copy this install cannot make', () => {
  it('says which tool is missing and offers no Retry', () => {
    render(
      <MediaAccessLine
        media={media({
          kind: 'local-image',
          type: 'image/tiff',
          access: access({ state: 'none', reason: 'needs vips, not installed' }),
        })}
        canRetry
        onRetry={vi.fn()}
      />,
    );

    expect(screen.getByText('No copy was made: needs vips, not installed')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
  });
});

describe('a settled copy', () => {
  it('says nothing when the copy is ready or the original is what the reader gets', () => {
    const { unmount } = render(
      <MediaAccessLine
        media={media({
          kind: 'video',
          access: access({ state: 'ready', url: 'http://localhost/files/access/7.mp4' }),
        })}
        canRetry
      />,
    );
    expect(document.querySelector('.media-access-state')).toBeNull();
    unmount();

    render(
      <MediaAccessLine
        media={media({
          kind: 'local-image',
          type: 'image/tiff',
          access: access({ state: 'none', reason: 'the image server reads the original' }),
        })}
        canRetry
      />,
    );
    expect(document.querySelector('.media-access-state')).toBeNull();
  });

  it('shows no player while the copy is being made, and the original to take', () => {
    render(
      <MediaStage
        media={media({ kind: 'video', access: access({ state: 'pending', progress: 10 }) })}
      />,
    );

    expect(document.querySelector('video')).toBeNull();
    expect(document.querySelector('.media-stage-file')).not.toBeNull();
    expect(screen.getByRole('link', { name: 'Download the file' })).toHaveAttribute(
      'href',
      ORIGINAL,
    );
  });

  it('shows no player when this install lacks the tool that makes the copy', () => {
    render(
      <MediaStage
        media={media({
          kind: 'video',
          access: access({ state: 'none', reason: 'needs vips, not installed' }),
        })}
      />,
    );

    expect(document.querySelector('video')).toBeNull();
    expect(document.querySelector('.media-stage-file')).not.toBeNull();
  });

  it('states the original beside the stage, with its format and size', () => {
    render(
      <MediaStage
        media={media({
          kind: 'video',
          access: access({
            state: 'ready',
            url: 'http://localhost/files/access/7.mp4',
            mediaType: 'video/mp4',
          }),
        })}
      />,
    );

    const link = screen.getByRole('link', { name: /^Original: AVI, / });
    expect(link).toHaveAttribute('href', ORIGINAL);
    expect(link).toHaveTextContent('Original: AVI, 489,684,992 bytes');
  });

  it('states no original when the original is what the browser shows', () => {
    render(
      <MediaStage
        media={media({
          kind: 'video',
          type: 'video/mp4',
          filename: 'clip.mp4',
          access: access({ state: 'none', reason: 'a browser shows the original' }),
        })}
      />,
    );
    expect(screen.queryByText(/^Original:/)).toBeNull();
  });
});

describe('who may retry', () => {
  const owned = media({ kind: 'video', owner: brandId<'UserId'>(3) });

  it('lets an editor change any media and an author only their own', () => {
    expect(mayEditMedia(owned, user('editor', 9))).toBe(true);
    expect(mayEditMedia(owned, user('global_admin', 9))).toBe(true);
    expect(mayEditMedia(owned, user('reviewer', 9))).toBe(true);
    expect(mayEditMedia(owned, user('author', 3))).toBe(true);
    expect(mayEditMedia(owned, user('author', 9))).toBe(false);
    expect(mayEditMedia(owned, null)).toBe(false);
  });
});
