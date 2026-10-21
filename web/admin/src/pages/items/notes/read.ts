/**
 * The item page's notes read: one WAP container per file, which says how many
 * notes the file has, how a note may address it, whether this reader may write,
 * and the first page of them.
 *
 * A file whose container refuses is left out of the map and named in `unread`,
 * so one unreadable file never hides the rest of the item.
 */
import type { Media, MediaId } from '../../../api/types';
import { notesClient } from '../../media/annotations/client';
import type { Note, Surface } from '../../media/annotations/wire';

/** One file's notes, as the item page's summary and grouped list show them. */
export interface FileNotes {
  readonly media: Media;
  readonly surface: Surface;
  /** The container offers a write, which is the media-update grant. */
  readonly writable: boolean;
  /** Every note of the file; `notes` holds the first page of them. */
  readonly total: number;
  readonly notes: readonly Note[];
}

export interface ItemNotesRead {
  /** The files the read reached, in the order they were asked for. */
  readonly files: ReadonlyMap<MediaId, FileNotes>;
  /** The files whose container did not answer. */
  readonly unread: readonly MediaId[];
  /** The ids this read answered for, so a newer list is never shown as its result. */
  readonly signature: string;
}

export const readItemNotes = async (
  media: readonly Media[],
  signature: string,
): Promise<ItemNotesRead> => {
  const answers = await Promise.all(
    media.map(async (entry) => {
      try {
        const container = await notesClient.container(entry.id, { pages: 'first' });
        return { entry, container };
      } catch {
        return { entry, container: null };
      }
    }),
  );
  const files = new Map<MediaId, FileNotes>();
  const unread: MediaId[] = [];
  for (const { entry, container } of answers) {
    if (container === null) {
      unread.push(entry.id);
      continue;
    }
    files.set(entry.id, {
      media: entry,
      surface: container.surface,
      writable: container.writable,
      total: container.total,
      notes: container.notes,
    });
  }
  return { files, unread, signature };
};
