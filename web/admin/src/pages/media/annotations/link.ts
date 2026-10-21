/**
 * The URLs that reach a media's notes from another page.
 *
 * The media page is the one place a note is read, written and drawn, so the
 * item page links to it with the panel's job in the query: `notes=add` opens a
 * whole-file draft and `note=<id>` opens that note. The panel applies the job
 * through its own `open` and `beginDraft`, so the row click and the link do the
 * same thing.
 */
import type { MediaId } from '../../../api/types';

/** What a link asks the media page's notes panel to do. */
export type NotesFocus =
  | { readonly kind: 'add' }
  | { readonly kind: 'note'; readonly noteId: string };

/** The media page's address, and the two ways the notes panel is asked to open. */
export const mediaHref = (mediaId: MediaId): string => `#/media/${String(mediaId)}`;

export const annotateHref = (mediaId: MediaId): string =>
  `${mediaHref(mediaId)}?notes=add`;

export const noteHref = (mediaId: MediaId, noteId: string): string =>
  `${mediaHref(mediaId)}?note=${encodeURIComponent(noteId)}`;

/** The query of a hash URL, as a map: `#/media/3?notes=add` and `#/media/3?note=7`. */
const queryOf = (hash: string): URLSearchParams => {
  const cut = hash.indexOf('?');
  return new URLSearchParams(cut === -1 ? '' : hash.slice(cut + 1));
};

/**
 * The panel's job from the URL. An unknown `note` is no job: the page still
 * opens, and the panel is what a reader gets without the query.
 */
export const notesFocusOf = (hash: string): NotesFocus | null => {
  const query = queryOf(hash);
  if (query.get('notes') === 'add') {
    return { kind: 'add' };
  }
  const note = query.get('note');
  if (note === null || !/^\d+$/.test(note)) {
    return null;
  }
  return { kind: 'note', noteId: note };
};
