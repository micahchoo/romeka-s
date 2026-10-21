/**
 * The item page's preview is URL state: `#/items/{id}?preview={mediaId}`. The
 * viewer opens on the file the URL names, Next and Previous rewrite that file
 * in place, and Back closes the viewer because the hash it opened from has no
 * preview in it.
 */
import { mediaIdFromParam } from '../../../api/parse';
import type { ItemId, MediaId } from '../../../api/types';

export const itemHref = (itemId: ItemId): string => `#/items/${String(itemId)}`;

export const previewHref = (itemId: ItemId, mediaId: MediaId): string =>
  `${itemHref(itemId)}?preview=${String(mediaId)}`;

/** The file the URL previews, or null when the URL previews none. */
export const previewIdOf = (hash: string): MediaId | null => {
  const cut = hash.indexOf('?');
  if (cut === -1) {
    return null;
  }
  const preview = new URLSearchParams(hash.slice(cut + 1)).get('preview');
  return preview === null ? null : mediaIdFromParam(preview);
};
