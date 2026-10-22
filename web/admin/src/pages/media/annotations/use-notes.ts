/**
 * The notes panel's state machine: the container read, the draft, the
 * conditional writes and the two refusals (412 with the colleague's version,
 * 404 when the note is gone).
 *
 * The panel renders this state and holds no write logic of its own. A status
 * line and the facts line are messages, so the hook translates them with the
 * panel's namespaces.
 *
 * The server decides three things the hook obeys: the surface (`Romeka-Surface`
 * says whether a note may address the picture, a time or the whole file), the
 * rights (`Allow` says whether POST is offered), and the canvas a create targets
 * (`Romeka-Canvas`).
 *
 * A conditional write needs the revision the editor saw, so opening a note reads
 * it once (`client.read`). A colleague who saves after that read moves the
 * revision past the one this editor holds, and this editor's `If-Match` makes
 * the server refuse with 412 and their version. That read is the difference
 * between a conflict the editor can resolve and one that silently overwrites a
 * colleague.
 */
import { useCallback, useEffect, useState } from 'react';
import type { Media } from '../../../api/types';
import { toError } from '../../../app/useAsync';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import { NoteConflict, NoteGone, notesClient } from './client';
import type { Container, Draft, Note, Selector, Surface } from './wire';
import { spanOf, withSpan } from './wire';
import { defaultRange, rangeProblem } from './time-text';
import type { TimeProblem, TimeRange } from './time-text';
import {
  draftOf,
  editDraftKey,
  forgetDraft,
  knownMotivation,
  newDraftKey,
  recallDraft,
  rememberDraft,
  sameDraft,
  storedOf,
} from './drafts';
import type { StoredDraft } from './drafts';
import { countsOf, placeLabel, placedNotes } from './note-list';

const STATUS_MS = 2000;

const newKey = (): string => globalThis.crypto.randomUUID();

/** One media's notes as the panel keeps them between writes. */
export interface Notes {
  readonly surface: Surface;
  readonly canvas: string;
  readonly writable: boolean;
  readonly notes: readonly Note[];
  readonly total: number;
  /** A container refresh after a write failed, so the counts may be behind. */
  readonly stale: boolean;
}

export type Load =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly error: Error }
  | { readonly status: 'ready'; readonly data: Notes };

export type Editing =
  | { readonly kind: 'none' }
  | { readonly kind: 'new'; readonly draft: Draft; readonly key: string }
  | { readonly kind: 'edit'; readonly note: Note; readonly draft: Draft; readonly restored: boolean }
  | {
      readonly kind: 'conflict';
      readonly note: Note;
      readonly theirs: Note;
      readonly draft: Draft;
      readonly intent: 'save' | 'delete';
    }
  | { readonly kind: 'gone'; readonly draft: Draft }
  /** A reader without the write grant, showing one note's whole text. */
  | { readonly kind: 'reading'; readonly note: Note }
  /** A note whose motivation this panel cannot send back unchanged. */
  | { readonly kind: 'locked'; readonly note: Note };

export interface NotesController {
  readonly load: Load;
  readonly ready: Notes | null;
  readonly editing: Editing;
  readonly busy: boolean;
  readonly formError: Error | null;
  readonly panelError: Error | null;
  readonly status: string | null;
  readonly restored: StoredDraft | null;
  readonly advanced: boolean;
  /** A loading read that has lasted past a second, so the outline may show. */
  readonly slow: boolean;
  readonly focusedIri: string | null;
  readonly focusedHeading: boolean;
  /** The facts line, or null when there is nothing to state. */
  readonly facts: string | null;
  /** The open entry's IRI, or null when no entry is open. */
  readonly selected: string | null;
  /**
   * The range the time editor's fields show: the open time draft's, else the
   * range the next note starts from (the last End used).
   */
  readonly range: TimeRange | null;
  /** Why that range cannot be saved, or null when it can. */
  readonly timeProblem: TimeProblem | null;
  setAdvanced: (value: boolean) => void;
  setRange: (startMs: number, endMs: number) => void;
  beginDraft: (selector: Selector) => void;
  open: (note: Note) => void;
  close: () => void;
  changeDraft: (draft: Draft) => void;
  save: () => void;
  drop: (note: Note) => void;
  saveOver: (theirs: Note, draft: Draft) => void;
  useTheirs: (theirs: Note) => void;
  keepTheirs: (theirs: Note) => void;
  saveAsNew: (draft: Draft) => void;
  /** The banner's Keep: opens the draft the browser kept. */
  keepRestored: () => void;
  discardRestored: () => void;
  /** Drops an edited note's kept draft and restores its stored text. */
  discardChange: (note: Note) => void;
  /** What the delete confirmation calls a note. */
  rowName: (note: Note) => string;
  refresh: () => void;
  retry: () => void;
  focused: () => void;
  /** Moves focus to a note's entry, which scrolls the list to it. */
  focusNote: (iri: string) => void;
  clearFocusedHeading: () => void;
}

const fromContainer = (container: Container): Notes => ({
  surface: container.surface,
  canvas: container.canvas,
  writable: container.writable,
  notes: container.notes,
  total: container.total,
  stale: false,
});

/** True once `waiting` has lasted longer than `milliseconds`. */
const useSlow = (waiting: boolean, milliseconds: number): boolean => {
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (!waiting) {
      setSlow(false);
      return;
    }
    const timer = setTimeout(() => {
      setSlow(true);
    }, milliseconds);
    return () => {
      clearTimeout(timer);
    };
  }, [waiting, milliseconds]);
  return slow;
};

export const useNotes = (media: Media): NotesController => {
  const translator = useTranslator(annotations, common);
  const { t } = translator;
  const [load, setLoad] = useState<Load>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const [editing, setEditing] = useState<Editing>({ kind: 'none' });
  const [advanced, setAdvanced] = useState(false);
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<Error | null>(null);
  const [panelError, setPanelError] = useState<Error | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [focusedIri, setFocusedIri] = useState<string | null>(null);
  const [focusedHeading, setFocusedHeading] = useState(false);
  const [restored, setRestored] = useState<StoredDraft | null>(() =>
    recallDraft(newDraftKey(media.id)),
  );
  /** Where the time editor's next range starts before any draft is open. */
  const [nextRange, setNextRange] = useState<TimeRange | null>(null);

  useEffect(() => {
    let live = true;
    setLoad({ status: 'loading' });
    notesClient.container(media.id).then(
      (container) => {
        if (live) {
          setLoad({ status: 'ready', data: fromContainer(container) });
        }
      },
      (error: unknown) => {
        if (live) {
          setLoad({ status: 'error', error: toError(error) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [media.id, attempt]);

  useEffect(() => {
    if (status === null) {
      return;
    }
    const timer = setTimeout(() => {
      setStatus(null);
    }, STATUS_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [status]);

  const ready = load.status === 'ready' ? load.data : null;
  const slow = useSlow(load.status === 'loading', 1000);

  /** The length `Romeka-Surface` states, for the range checks. */
  const durationMs = ready !== null && ready.surface.kind === 'time' ? ready.surface.durationMs : null;
  const openDraft =
    editing.kind === 'new' || editing.kind === 'edit' || editing.kind === 'conflict'
      ? editing.draft
      : null;
  const range: TimeRange | null = (openDraft === null ? null : spanOf(openDraft.selector)) ?? nextRange;
  const timeProblem =
    range === null ? null : rangeProblem(range.startMs, range.endMs, durationMs);

  /**
   * The fields start where the recording does, so an empty recording shows a
   * range rather than two blank boxes; Note from here moves them to the player.
   */
  useEffect(() => {
    if (ready !== null && ready.surface.kind === 'time' && nextRange === null) {
      setNextRange(defaultRange(0, ready.surface.durationMs));
    }
  }, [ready, nextRange]);

  /** A saved time range becomes the next note's start, as dictation does. */
  const keepNext = (selector: Selector): void => {
    const span = spanOf(selector);
    if (span !== null) {
      setNextRange(defaultRange(span.endMs, durationMs));
    }
  };

  /** The container read again after a write: the counts are the server's truth. */
  const refresh = useCallback((): void => {
    notesClient.container(media.id).then(
      (container) => {
        setLoad({ status: 'ready', data: fromContainer(container) });
      },
      () => {
        setLoad((current) =>
          current.status === 'ready'
            ? { status: 'ready', data: { ...current.data, stale: true } }
            : current,
        );
      },
    );
  }, [media.id]);

  const upsertLocal = (note: Note): void => {
    setLoad((current) => {
      if (current.status !== 'ready') {
        return current;
      }
      const known = current.data.notes.some((entry) => entry.iri === note.iri);
      return {
        status: 'ready',
        data: {
          ...current.data,
          notes: known
            ? current.data.notes.map((entry) => (entry.iri === note.iri ? note : entry))
            : [...current.data.notes, note],
          total: known ? current.data.total : current.data.total + 1,
        },
      };
    });
  };

  const dropLocal = (note: Note): void => {
    setLoad((current) => {
      if (current.status !== 'ready') {
        return current;
      }
      const known = current.data.notes.some((entry) => entry.iri === note.iri);
      return {
        status: 'ready',
        data: {
          ...current.data,
          notes: current.data.notes.filter((entry) => entry.iri !== note.iri),
          total: known ? Math.max(0, current.data.total - 1) : current.data.total,
        },
      };
    });
  };

  const focusAfter = (note: Note): void => {
    const rows = placedNotes(ready === null ? [] : ready.notes);
    const index = rows.findIndex((row) => row.note.iri === note.iri);
    const next = rows[index + 1] ?? rows[index - 1];
    if (next === undefined) {
      setFocusedHeading(true);
    } else {
      setFocusedIri(next.note.iri);
    }
  };

  const close = (): void => {
    setEditing({ kind: 'none' });
    setFormError(null);
    // A draft the editor left behind stays on offer until it is discarded.
    setRestored(recallDraft(newDraftKey(media.id)));
  };

  const beginDraft = (selector: Selector): void => {
    if (ready === null || !ready.writable) {
      return;
    }
    const span = spanOf(selector);
    if (span !== null) {
      setNextRange(span);
    }
    const stored = recallDraft(newDraftKey(media.id));
    setEditing({
      kind: 'new',
      key: newKey(),
      draft: {
        motivation:
          stored === null ? 'commenting' : (knownMotivation(stored.motivation) ?? 'commenting'),
        selector,
        text: stored === null ? '' : stored.text,
        language: stored === null ? media.lang : stored.language,
      },
    });
    setRestored(null);
    setFormError(null);
    setStatus(null);
  };

  const open = (note: Note): void => {
    const same =
      (editing.kind === 'edit' ||
        editing.kind === 'reading' ||
        editing.kind === 'locked' ||
        editing.kind === 'conflict') &&
      editing.note.iri === note.iri;
    if (same) {
      close();
      return;
    }
    setFormError(null);
    setPanelError(null);
    if (ready === null) {
      return;
    }
    if (!ready.writable) {
      setEditing({ kind: 'reading', note });
      return;
    }
    notesClient.read(note).then(
      (current) => {
        if (knownMotivation(current.motivation) === null) {
          setEditing({ kind: 'locked', note: current });
          return;
        }
        // An imported cue is a snapshot of a caption file, and the next import
        // replaces it, so its form opens in Advanced where that is stated.
        if (current.fromTrack) {
          setAdvanced(true);
        }
        const stored = recallDraft(editDraftKey(current.iri));
        const base = draftOf(current);
        setEditing({
          kind: 'edit',
          note: current,
          restored: stored !== null,
          draft:
            stored === null
              ? base
              : {
                  ...base,
                  text: stored.text,
                  language: stored.language,
                  motivation: knownMotivation(stored.motivation) ?? base.motivation,
                },
        });
      },
      (error: unknown) => {
        if (error instanceof NoteGone) {
          setStatus(t('notes.alreadyDeleted'));
          dropLocal(note);
          return;
        }
        setPanelError(toError(error));
      },
    );
  };

  const stash = (current: Editing, draft: Draft): void => {
    switch (current.kind) {
      case 'new':
        rememberDraft(newDraftKey(media.id), storedOf(draft));
        break;
      case 'edit':
      case 'conflict':
        rememberDraft(editDraftKey(current.note.iri), storedOf(draft));
        break;
      default:
        break;
    }
  };

  const changeDraft = (draft: Draft): void => {
    stash(editing, draft);
    setEditing((current) => {
      switch (current.kind) {
        case 'new':
          return { ...current, draft };
        case 'edit':
          return { ...current, draft, restored: false };
        case 'conflict':
          return { ...current, draft };
        default:
          return current;
      }
    });
  };

  /**
   * The Start and End fields: they edit the open draft's range, a time note's or
   * a box's, so Save sends the range they show, or the range the next note will
   * start from.
   */
  const setRange = (startMs: number, endMs: number): void => {
    const draft =
      editing.kind === 'new' || editing.kind === 'edit' || editing.kind === 'conflict'
        ? editing.draft
        : null;
    if (draft !== null && spanOf(draft.selector) !== null) {
      changeDraft({ ...draft, selector: withSpan(draft.selector, startMs, endMs) });
      return;
    }
    setNextRange({ startMs, endMs });
  };

  const create = (draft: Draft, key: string): void => {
    if (ready === null) {
      return;
    }
    setBusy(true);
    setFormError(null);
    notesClient.create(media.id, draft, key, ready.canvas).then(
      (note) => {
        setBusy(false);
        forgetDraft(newDraftKey(media.id));
        setRestored(null);
        keepNext(draft.selector);
        setStatus(t('notes.saved'));
        close();
        upsertLocal(note);
        refresh();
      },
      (error: unknown) => {
        setBusy(false);
        setFormError(toError(error));
      },
    );
  };

  const put = (note: Note, draft: Draft): void => {
    setBusy(true);
    setFormError(null);
    notesClient.replace(note, draft).then(
      (saved) => {
        setBusy(false);
        forgetDraft(editDraftKey(note.iri));
        keepNext(draft.selector);
        setStatus(t('notes.saved'));
        close();
        upsertLocal(saved);
        refresh();
      },
      (error: unknown) => {
        setBusy(false);
        if (error instanceof NoteConflict) {
          // The row shows their version now; the draft stays in the form below it.
          upsertLocal(error.current);
          setEditing({ kind: 'conflict', note, theirs: error.current, draft, intent: 'save' });
          return;
        }
        if (error instanceof NoteGone) {
          forgetDraft(editDraftKey(note.iri));
          setEditing({ kind: 'gone', draft });
          return;
        }
        setFormError(toError(error));
      },
    );
  };

  const drop = (note: Note): void => {
    setBusy(true);
    setFormError(null);
    notesClient.remove(note).then(
      () => {
        setBusy(false);
        forgetDraft(editDraftKey(note.iri));
        setStatus(t('notes.deleted'));
        focusAfter(note);
        close();
        dropLocal(note);
        refresh();
      },
      (error: unknown) => {
        setBusy(false);
        if (error instanceof NoteConflict) {
          upsertLocal(error.current);
          setEditing({
            kind: 'conflict',
            note,
            theirs: error.current,
            draft: draftOf(note),
            intent: 'delete',
          });
          return;
        }
        if (error instanceof NoteGone) {
          setStatus(t('notes.alreadyDeleted'));
          focusAfter(note);
          close();
          dropLocal(note);
          return;
        }
        setFormError(toError(error));
      },
    );
  };

  const save = (): void => {
    // A range the server would refuse is already stated at its field; Save can
    // only restate it, never hide it behind a disabled button.
    if (busy || timeProblem !== null) {
      return;
    }
    switch (editing.kind) {
      case 'new':
        create(editing.draft, editing.key);
        break;
      case 'edit':
        if (sameDraft(editing.draft, editing.note)) {
          setStatus(t('notes.form.noChanges'));
          break;
        }
        put(editing.note, editing.draft);
        break;
      default:
        break;
    }
  };

  const keepTheirs = (theirs: Note): void => {
    forgetDraft(editDraftKey(theirs.iri));
    setStatus(t('notes.keptTheirs'));
    close();
    upsertLocal(theirs);
  };

  const selected =
    editing.kind === 'edit' ||
    editing.kind === 'conflict' ||
    editing.kind === 'reading' ||
    editing.kind === 'locked'
      ? editing.note.iri
      : null;

  const counts = ready === null ? null : countsOf(ready.notes);
  const facts =
    ready === null || counts === null || ready.notes.length === 0
      ? null
      : ready.surface.kind === 'image'
        ? t(ready.total === 1 ? 'notes.facts.imageOne' : 'notes.facts.image', {
            total: ready.total,
            regions: counts.region,
            whole: counts.whole,
          })
        : ready.surface.kind === 'time'
          ? t(ready.total === 1 ? 'notes.facts.timeOne' : 'notes.facts.time', {
              total: ready.total,
              times: counts.time,
              whole: counts.whole,
            })
          : t(ready.total === 1 ? 'notes.facts.fileOne' : 'notes.facts.file', {
              total: ready.total,
            });

  /** What the delete confirmation calls a note: its place in the media. */
  const rowName = (note: Note): string => {
    if (ready === null) {
      return '';
    }
    const row = placedNotes(ready.notes).find((entry) => entry.note.iri === note.iri);
    return row === undefined ? '' : placeLabel(row, ready.surface, translator);
  };

  return {
    load,
    ready,
    editing,
    busy,
    formError,
    panelError,
    status,
    restored,
    advanced,
    slow,
    focusedIri,
    focusedHeading,
    facts,
    selected,
    range,
    timeProblem,
    setAdvanced,
    setRange,
    beginDraft,
    open,
    close,
    changeDraft,
    save,
    drop,
    saveOver: put,
    useTheirs: keepTheirs,
    keepTheirs,
    saveAsNew: (draft) => {
      create(draft, newKey());
    },
    keepRestored: () => {
      beginDraft({ kind: 'whole' });
    },
    discardRestored: () => {
      forgetDraft(newDraftKey(media.id));
      setRestored(null);
    },
    discardChange: (note) => {
      forgetDraft(editDraftKey(note.iri));
      setEditing({ kind: 'edit', note, draft: draftOf(note), restored: false });
    },
    rowName,
    refresh,
    retry: () => {
      setAttempt((current) => current + 1);
    },
    focused: () => {
      setFocusedIri(null);
    },
    focusNote: (iri) => {
      setFocusedIri(iri);
    },
    clearFocusedHeading: () => {
      setFocusedHeading(false);
    },
  };
};
