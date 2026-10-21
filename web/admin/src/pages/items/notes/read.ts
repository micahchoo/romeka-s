/**
 * The item page's notes read: one counts answer for the whole item, then one
 * WAP container for each file the page draws notes from.
 *
 * The counts say how many notes each file has and whether this caller may write
 * them, which is all a card needs. A container is read only for a file with a
 * note to show, so a file with no notes costs one entry in the counts document
 * and no request. A container that refuses is left out of the map and named in
 * `unread`, so one unreadable file never hides the rest of the item.
 */
import { defaultBaseUrl } from '../../../api/client';
import {
  asBoolean,
  asIdNumber,
  asNumber,
  asRecord,
  brandId,
  field,
} from '../../../api/parse';
import type { ItemId, Media, MediaId } from '../../../api/types';
import { notesClient, refusal } from '../../media/annotations/client';
import type { Note, Surface } from '../../media/annotations/wire';

/** One file as the item's counts read answers it. */
export interface FileCount {
  readonly count: number;
  readonly writable: boolean;
}

/** One file's container, as the first-file summary and the grouped list draw it. */
export interface FileNotes {
  readonly surface: Surface;
  /** The container's first page of notes, which is every note the page draws. */
  readonly notes: readonly Note[];
}

export interface ItemNotesRead {
  /** Every file the counts read answered for. */
  readonly counts: ReadonlyMap<MediaId, FileCount>;
  /** The files whose notes the page shows, with their first page of them. */
  readonly files: ReadonlyMap<MediaId, FileNotes>;
  /** The files whose container did not answer, though their count says it has notes. */
  readonly unread: readonly MediaId[];
  /** The ids this read answered for, so a newer list is never shown as its result. */
  readonly signature: string;
}

/** The counts document: `{media: {"<id>": {count, writable}}}`. */
export const parseCounts = (raw: unknown): ReadonlyMap<MediaId, FileCount> => {
  const document = asRecord(raw, []);
  const media = asRecord(field(document, 'media', []), ['media']);
  const counts = new Map<MediaId, FileCount>();
  for (const [key, value] of Object.entries(media)) {
    const path = ['media', key];
    const entry = asRecord(value, path);
    counts.set(brandId<'MediaId'>(asIdNumber(Number(key), path)), {
      count: asNumber(field(entry, 'count', path), [...path, 'count']),
      writable: asBoolean(field(entry, 'writable', path), [...path, 'writable']),
    });
  }
  return counts;
};

const readCounts = async (item: ItemId): Promise<ReadonlyMap<MediaId, FileCount>> => {
  const response = await fetch(`${defaultBaseUrl()}/annotations/item/${String(item)}/counts`, {
    headers: { Accept: 'application/json' },
    credentials: 'same-origin',
  });
  if (!response.ok) {
    throw await refusal(response);
  }
  const body: unknown = await response.json();
  return parseCounts(body);
};

export const readItemNotes = async (
  item: ItemId,
  media: readonly Media[],
  signature: string,
): Promise<ItemNotesRead> => {
  // The item page's first render reaches here before its media read answers.
  // With no files there is nothing to count, so the visit's one counts read is
  // not spent on the page's own loading state.
  if (media.length === 0) {
    return { counts: new Map(), files: new Map(), unread: [], signature };
  }
  const counts = await readCounts(item);
  const drawn = media.filter((entry) => (counts.get(entry.id)?.count ?? 0) > 0);
  const answers = await Promise.all(
    drawn.map(async (entry) => {
      try {
        return { entry, container: await notesClient.container(entry.id, { pages: 'first' }) };
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
    files.set(entry.id, { surface: container.surface, notes: container.notes });
  }
  return { counts, files, unread, signature };
};
