/**
 * Derived values for the Media pages. Every function is pure, so the pages only
 * decide layout and never repeat the fallback rules.
 */
import type { Media, ThumbnailType } from '../api/types';

/** Order a browse cell prefers. Upstream shows the square thumbnail. */
const CELL_ORDER: readonly ThumbnailType[] = ['square', 'medium', 'large'];

/** Order the detail page prefers for a full-width image. */
const LARGE_ORDER: readonly ThumbnailType[] = ['large', 'medium', 'square'];

const nonEmpty = (text: string | null): string | null =>
  text === null || text === '' ? null : text;

/**
 * `MediaRepresentation::displayTitle` falls back to the media source. This
 * collection has no media titles, so the source or the filename is what the
 * row is called. `null` when the row has none, so the caller supplies the
 * catalogue's placeholder.
 */
export const mediaName = (media: Media): string | null =>
  nonEmpty(media.title) ?? nonEmpty(media.source) ?? nonEmpty(media.filename);

const thumbnailUrl = (media: Media, order: readonly ThumbnailType[]): string | null => {
  for (const type of order) {
    const url = nonEmpty(media.thumbnailUrls[type] ?? null);
    if (url !== null) {
      return url;
    }
  }
  return null;
};

/**
 * The image a browse cell shows: a thumbnail when the port produced one, the
 * original otherwise. `o:thumbnail_urls` is empty on every row of this
 * collection, so the original is the common case.
 */
export const previewImageUrl = (media: Media): string | null =>
  thumbnailUrl(media, CELL_ORDER) ?? nonEmpty(media.originalUrl);

/** The image the detail page shows at full width. */
export const largeImageUrl = (media: Media): string | null =>
  thumbnailUrl(media, LARGE_ORDER) ?? nonEmpty(media.originalUrl);

/** The stored original file, when the row still has one. */
export const originalFileUrl = (media: Media): string | null => nonEmpty(media.originalUrl);

/** The file a media preview links to: the original, else the best thumbnail. */
export const previewLinkUrl = (media: Media): string | null =>
  nonEmpty(media.originalUrl) ?? thumbnailUrl(media, CELL_ORDER);
