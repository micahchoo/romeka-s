/**
 * The item page's view of its files' notes: one read for the whole item, kept
 * in one place so the cards, the first file's summary and the grouped list
 * agree about every count.
 *
 * The reader is a dynamic import, so an item page that never shows a count
 * downloads the WAP client only when it has a file to ask about.
 */
import { useState } from 'react';
import type { Media, MediaId } from '../../../api/types';
import { useAsync } from '../../../app/useAsync';
import type { AsyncState } from '../../../app/useAsync';
import type { ItemNotesRead } from './read';

export interface ItemNotes {
  readonly state: AsyncState<ItemNotesRead>;
  /** The file's note count, or null while it is unknown. */
  readonly count: (id: MediaId) => number | null;
  /** Whether the file's notes may be written, or null while it is unknown. */
  readonly writable: (id: MediaId) => boolean | null;
  readonly retry: () => void;
}

const LOADING: AsyncState<ItemNotesRead> = { status: 'loading' };

/** The media ids a read answers for, so a stale answer is never shown. */
const signatureOf = (media: readonly Media[]): string =>
  media.map((entry) => String(entry.id)).join(',');

export const useItemNotes = (media: readonly Media[]): ItemNotes => {
  const [attempt, setAttempt] = useState(0);
  const signature = signatureOf(media);
  const state = useAsync(async (): Promise<ItemNotesRead> => {
    const { readItemNotes } = await import('./read');
    return readItemNotes(media, signature);
  }, [media, signature, attempt]);
  // The effect that starts a newer read runs after one render, so the answer
  // for the previous list would otherwise show as this one's.
  const current =
    state.status === 'ready' && state.data.signature !== signature ? LOADING : state;

  const fileOf = (id: MediaId) =>
    current.status === 'ready' ? (current.data.files.get(id) ?? null) : null;

  return {
    state: current,
    count: (id) => fileOf(id)?.total ?? null,
    writable: (id) => fileOf(id)?.writable ?? null,
    retry: () => {
      setAttempt((currentAttempt) => currentAttempt + 1);
    },
  };
};
