/**
 * The item page's preview is URL state: `#/items/{id}?preview={mediaId}`. The
 * viewer opens on the file the URL names, Next and Previous rewrite that file
 * in place, and Back closes the viewer because the hash it opened from has no
 * preview in it. The page's other keys (a walk's `in` and `at`, `edit`) stay.
 */
import { mediaIdFromParam } from '../../../api/media';
import type { ItemId, MediaId } from '../../../api/types';

/** The item's page with its query's preview set to `mediaId`, or cleared when null. */
export const previewHref = (itemId: ItemId, query: URLSearchParams, mediaId: MediaId | null): string => {
  const next = new URLSearchParams(query);
  if (mediaId === null) {
    next.delete('preview');
  } else {
    next.set('preview', String(mediaId));
  }
  const search = next.toString();
  return `#/items/${String(itemId)}${search === '' ? '' : `?${search}`}`;
};

/** The file the URL previews, or null when the URL previews none. */
export const previewIdOf = (hash: string): MediaId | null => {
  const cut = hash.indexOf('?');
  if (cut === -1) {
    return null;
  }
  const preview = new URLSearchParams(hash.slice(cut + 1)).get('preview');
  return preview === null ? null : mediaIdFromParam(preview);
};
