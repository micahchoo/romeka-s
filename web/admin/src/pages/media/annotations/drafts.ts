/**
 * The panel's drafts: a stored note as a draft, a draft as the record kept in
 * `sessionStorage`, and that storage itself.
 *
 * One record per note IRI, and one per media for a note that is not saved yet.
 * The record holds the text, its language and the motivation; the selector is
 * not stored because this panel only starts whole-file notes. The region editor
 * (A-4) and the time editor (A-5) extend this record once their drawn selector
 * must survive a reload.
 *
 * `sessionStorage` throws in a browser that blocks storage, and a stale record
 * is untrusted input, so every read validates its shape and answers `null` on
 * anything it cannot read. The storage itself is touched only here.
 */
import { isRecord } from '../../../api/parse';
import type { Draft, Motivation, Note } from './wire';

/** What a draft keeps between renders and across a reload. */
export interface StoredDraft {
  readonly text: string;
  readonly language: string | null;
  readonly motivation: string;
}

const PREFIX = 'romeka.annotations.';

/** The motivations this panel can write (the W3C set it offers). */
const MOTIVATIONS: readonly Motivation[] = ['commenting', 'tagging', 'describing', 'supplementing'];

/** A note's motivation when the panel knows it, else null. */
export const knownMotivation = (value: string): Motivation | null =>
  MOTIVATIONS.find((motivation) => motivation === value) ?? null;

/** The draft that starts from a stored note. */
export const draftOf = (note: Note): Draft => ({
  motivation: knownMotivation(note.motivation) ?? 'commenting',
  selector: note.selector,
  text: note.text,
  language: note.language,
});

/** What a draft keeps between renders and across a reload. */
export const storedOf = (draft: Draft): StoredDraft => ({
  text: draft.text,
  language: draft.language,
  motivation: draft.motivation,
});

/** Whether the editor changed anything a `PUT` would carry. */
export const sameDraft = (draft: Draft, note: Note): boolean =>
  draft.text === note.text &&
  draft.language === note.language &&
  draft.motivation === note.motivation &&
  JSON.stringify(draft.selector) === JSON.stringify(note.selector);

/** The key of the note not saved yet on one media. */
export const newDraftKey = (mediaId: number): string => `${PREFIX}new.${String(mediaId)}`;

/** The key of the unsaved change to one stored note. */
export const editDraftKey = (iri: string): string => `${PREFIX}note.${iri}`;

const storage = (): Storage | null => {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
};

export const rememberDraft = (key: string, draft: StoredDraft): void => {
  try {
    storage()?.setItem(key, JSON.stringify(draft));
  } catch {
    // A full or blocked store costs the draft, not the note being written.
  }
};

export const recallDraft = (key: string): StoredDraft | null => {
  try {
    const text = storage()?.getItem(key) ?? null;
    if (text === null) {
      return null;
    }
    const value: unknown = JSON.parse(text);
    return storedDraftOf(value);
  } catch {
    return null;
  }
};

export const forgetDraft = (key: string): void => {
  try {
    storage()?.removeItem(key);
  } catch {
    // Nothing to remove from a store that never accepted the draft.
  }
};

const storedDraftOf = (value: unknown): StoredDraft | null => {
  if (!isRecord(value)) {
    return null;
  }
  const { text, language, motivation } = value;
  if (typeof text !== 'string' || typeof motivation !== 'string') {
    return null;
  }
  return {
    text,
    language: typeof language === 'string' ? language : null,
    motivation,
  };
};
