/**
 * The notes panel on a media page: the facts line, the list, the form that
 * writes a note, and the conflict view when a colleague saved first.
 *
 * The state machine (the read, the drafts, the conditional writes) is
 * `use-notes.ts`; this file renders it and owns no request of its own.
 *
 * The region editor and the time editor draw on the stage itself, in the
 * places it hands the page (`StageSlots`): the tools above the media, the
 * shapes over it, the timeline under it. They read `surface` and `notes`, call
 * `open` with the entry they select, and start a draft with a drawn selector
 * through `beginDraft`. With no stage they render at `.media-notes-surface`.
 */
import type { ReactElement, ReactNode } from 'react';
import { Suspense, lazy, useEffect, useId, useRef, useState } from 'react';
import type { Media } from '../../../api/types';
import type { StageSlots } from '../stage/slots';
import { ErrorNotice } from '../../../app/errors';
import { mediaKind } from '../../../app/media';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import type { TrackSummary } from './client';
import type { NotesFocus } from './link';
import type { Editing } from './use-notes';
import type { Note, Selector } from './wire';
import { noteIdOf, spanOf } from './wire';
import { NoteList, placedNotes } from './note-list';
import { seek } from './player';
import { useNotes } from './use-notes';
import './annotations.css';

/**
 * A conflict is rare and only exists after a 412, and a form is only needed once
 * the editor writes, so both are their own chunks: a reader of the notes, or an
 * editor who never changes one, downloads neither (PERF.md, Decision 22).
 */
const ConflictView = lazy(() =>
  import('./conflict').then((module) => ({ default: module.ConflictView })),
);
const NoteForm = lazy(() => import('./note-form').then((module) => ({ default: module.NoteForm })));

/** A third chunk: only a picture has a region editor, and only it fetches this. */
const RegionEditor = lazy(() =>
  import('./region-editor').then((module) => ({ default: module.RegionEditor })),
);

/** The time editor is its own chunk, as a player's page is not an image's. */
const TimeEditor = lazy(() =>
  import('./time-editor').then((module) => ({ default: module.TimeEditor })),
);

/**
 * Captions are another chunk: most media have none, and the dialog, the dry run
 * and the remove confirmation are not needed to read a note.
 */
const Captions = lazy(() =>
  import('./captions').then((module) => ({ default: module.Captions })),
);
const UseAsCaptions = lazy(() =>
  import('./captions').then((module) => ({ default: module.UseAsCaptions })),
);

/** The open draft's shape, so the overlay draws it where the editor put it. */
const shapeOf = (
  editing: Editing,
): { readonly iri: string | null; readonly selector: Selector } | null => {
  switch (editing.kind) {
    case 'new':
      return { iri: null, selector: editing.draft.selector };
    case 'edit':
    case 'conflict':
      return { iri: editing.note.iri, selector: editing.draft.selector };
    default:
      return null;
  }
};

export const MediaNotes = ({
  media,
  player,
  focus,
  onTracks,
  stage = null,
  onClaim,
}: {
  readonly media: Media;
  /** The stage's player element, when the media has one. */
  readonly player?: HTMLMediaElement | null;
  /** The job the URL asked for, applied once per URL. */
  readonly focus?: NotesFocus | null;
  /** The caption languages, for the stage's `<track>` elements. */
  readonly onTracks?: (tracks: readonly TrackSummary[]) => void;
  /** The places on the stage to draw in; null renders the editors here. */
  readonly stage?: StageSlots | null;
  /** The region editor draws the picture, so the stage leaves its own out. */
  readonly onClaim?: (claimed: boolean) => void;
}): ReactElement => {
  const { t } = useTranslator(annotations, common);
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const notes = useNotes(media);
  const { load, ready, editing } = notes;
  const { focusedHeading, clearFocusedHeading } = notes;
  /** The URL job already applied, so a re-render does not open the note twice. */
  const applied = useRef<string | null>(null);
  /** A time note the URL opened, waiting for the stage's player to exist. */
  const [urlSeek, setUrlSeek] = useState<number | null>(null);

  useEffect(() => {
    if (focusedHeading) {
      heading.current?.focus();
      clearFocusedHeading();
    }
  }, [focusedHeading, clearFocusedHeading]);

  /**
   * A link from the item page names a note or asks for a draft. The panel opens
   * it through the same `open` and `beginDraft` its own list calls, so a region
   * is highlighted and a recording is seeked exactly as a click does it.
   */
  useEffect(() => {
    if (focus === null || focus === undefined || ready === null) {
      return;
    }
    const key = focus.kind === 'add' ? 'add' : `note:${focus.noteId}`;
    if (applied.current === key) {
      return;
    }
    if (focus.kind === 'add') {
      applied.current = key;
      notes.beginDraft({ kind: 'whole' });
      return;
    }
    const note = ready.notes.find((entry) => noteIdOf(entry.iri) === focus.noteId);
    if (note === undefined) {
      return;
    }
    applied.current = key;
    notes.open(note);
    notes.focusNote(note.iri);
    const span = spanOf(note.selector);
    if (span !== null) {
      setUrlSeek(span.startMs);
    }
  }, [focus, ready, notes]);

  /** The player mounts after the notes read, so the seek waits for it. */
  useEffect(() => {
    if (urlSeek === null || player === null || player === undefined) {
      return;
    }
    seek(player, urlSeek);
    setUrlSeek(null);
  }, [urlSeek, player]);

  const expanded = (note: Note): ReactNode | null => {
    switch (editing.kind) {
      case 'edit':
        if (editing.note.iri !== note.iri) {
          return null;
        }
        return (
          <Suspense fallback={<p role="status">{t('loading.form')}</p>}>
            <NoteForm
              key={note.iri}
              note={editing.note}
              rowName={notes.rowName(note)}
              draft={editing.draft}
              advanced={notes.advanced}
              busy={notes.busy}
              error={notes.formError}
              restored={editing.restored}
              onChange={notes.changeDraft}
              onSave={notes.save}
              onDelete={() => {
                notes.drop(editing.note);
              }}
              onCancel={notes.close}
              onAdvanced={notes.setAdvanced}
              onDiscardRestored={() => {
                notes.discardChange(editing.note);
              }}
            />
          </Suspense>
        );
      case 'conflict':
        if (editing.note.iri !== note.iri) {
          return null;
        }
        return (
          <Suspense fallback={<p role="status">{t('notes.loading')}</p>}>
            <ConflictView
              theirs={editing.theirs}
              draft={editing.draft}
              intent={editing.intent}
              busy={notes.busy}
              error={notes.formError}
              onChange={notes.changeDraft}
              onSaveMine={() => {
                notes.saveOver(editing.theirs, editing.draft);
              }}
              onUseTheirs={() => {
                notes.useTheirs(editing.theirs);
              }}
              onDeleteTheirs={() => {
                notes.drop(editing.theirs);
              }}
              onKeep={() => {
                notes.keepTheirs(editing.theirs);
              }}
            />
          </Suspense>
        );
      case 'reading':
        return editing.note.iri === note.iri ? (
          <p className="note-full-text">
            {editing.note.text === '' ? t('notes.noText') : editing.note.text}
          </p>
        ) : null;
      case 'locked':
        return editing.note.iri === note.iri ? (
          <p className="note-locked">{t('notes.unknownMotivation')}</p>
        ) : null;
      default:
        return null;
    }
  };

  return (
    <section className="media-notes" aria-labelledby={headingId}>
      <h2 id={headingId} ref={heading} tabIndex={-1}>
        {t('notes.heading')}
      </h2>

      {notes.facts === null ? null : <p className="media-notes-facts">{notes.facts}</p>}
      {ready !== null && !ready.writable ? (
        <p className="media-notes-read-only">{t('notes.readOnly')}</p>
      ) : null}
      {ready !== null && ready.stale ? (
        <p className="media-notes-stale">
          {t('notes.stale')}{' '}
          <button type="button" onClick={notes.refresh}>
            {t('notes.refresh')}
          </button>
        </p>
      ) : null}

      {notes.restored !== null && ready !== null && ready.writable && editing.kind === 'none' ? (
        <p className="media-notes-restored">
          {t('notes.draftBanner')}{' '}
          <button type="button" onClick={notes.keepRestored}>
            {t('notes.keep')}
          </button>
          <button type="button" onClick={notes.discardRestored}>
            {t('notes.discard')}
          </button>
        </p>
      ) : null}

      {/* The editors draw on the stage's slots; with no stage, they render here. */}
      {ready === null ? null : (
        <div className="media-notes-surface" data-surface={ready.surface.kind}>
          {ready.surface.kind === 'image' ? (
            <Suspense fallback={null}>
              <RegionEditor
                slots={stage}
                onClaim={onClaim}
                surface={ready.surface}
                rows={placedNotes(ready.notes)}
                selected={notes.selected}
                draft={shapeOf(editing)}
                writable={ready.writable}
                onDraw={notes.beginDraft}
                onSelect={notes.open}
                onMove={(note, selector) => {
                  // A nudge edits the open draft, so Save sends the shape the
                  // overlay shows; a note that is not open has nothing to edit.
                  if (editing.kind === 'edit' && editing.note.iri === note.iri) {
                    notes.changeDraft({ ...editing.draft, selector });
                  }
                }}
              />
            </Suspense>
          ) : ready.surface.kind === 'time' ? (
            <Suspense fallback={null}>
              <TimeEditor
                slots={stage}
                surface={ready.surface}
                rows={placedNotes(ready.notes)}
                selected={notes.selected}
                draft={shapeOf(editing)}
                writable={ready.writable}
                player={player ?? null}
                range={notes.range}
                problem={notes.timeProblem}
                onRange={notes.setRange}
                onDraw={notes.beginDraft}
                onSelect={notes.open}
                onReshape={(selector) => {
                  // The keys move the open box, saved or not, so Save sends the
                  // box the frame shows.
                  if (editing.kind === 'new' || editing.kind === 'edit' || editing.kind === 'conflict') {
                    notes.changeDraft({ ...editing.draft, selector });
                  }
                }}
              />
            </Suspense>
          ) : null}
        </div>
      )}

      {ready !== null && ready.surface.kind === 'time' ? (
        <Suspense fallback={null}>
          <Captions
            mediaId={media.id}
            writable={ready.writable}
            mediaLang={media.lang}
            onTracks={onTracks ?? (() => undefined)}
          />
        </Suspense>
      ) : mediaKind(media) === 'captions' ? (
        <Suspense fallback={null}>
          <UseAsCaptions media={media} />
        </Suspense>
      ) : null}

      {notes.panelError === null ? null : <ErrorNotice error={notes.panelError} />}

      {editing.kind === 'new' ? (
        <div className="note-draft">
          <Suspense fallback={<p role="status">{t('loading.form')}</p>}>
            <NoteForm
              note={null}
              rowName=""
              draft={editing.draft}
              advanced={notes.advanced}
              busy={notes.busy}
              error={notes.formError}
              restored={false}
              onChange={notes.changeDraft}
              onSave={notes.save}
              onDelete={null}
              onCancel={notes.close}
              onAdvanced={notes.setAdvanced}
              onDiscardRestored={null}
            />
          </Suspense>
        </div>
      ) : null}

      {editing.kind === 'gone' ? (
        <div className="note-draft note-gone">
          <p>{t('notes.gone')}</p>
          <div className="note-form-actions">
            <button
              type="button"
              onClick={() => {
                notes.saveAsNew(editing.draft);
              }}
            >
              {t('notes.gone.saveAsNew')}
            </button>
            <button type="button" onClick={notes.close}>
              {t('notes.discard')}
            </button>
          </div>
        </div>
      ) : null}

      {load.status === 'loading' ? (
        <div className="media-notes-list" aria-busy="true">
          {notes.slow ? (
            <>
              <p role="status">{t('notes.loading')}</p>
              {[0, 1, 2].map((row) => (
                <div key={row} className="note-skeleton" />
              ))}
            </>
          ) : null}
        </div>
      ) : load.status === 'error' ? (
        <>
          <p>{t('notes.loadFailed')}</p>
          <ErrorNotice error={load.error} />
          <button type="button" onClick={notes.retry}>
            {t('notes.retry')}
          </button>
        </>
      ) : (
        <NoteList
          surface={load.data.surface}
          notes={load.data.notes}
          writable={load.data.writable}
          selected={notes.selected}
          onSelect={(note) => {
            // A time note's entry does what its segment does: the editor hears
            // the moment the note is about, from either click.
            notes.open(note);
            const span = spanOf(note.selector);
            if (span !== null) {
              seek(player ?? null, span.startMs);
            }
          }}
          onAdd={() => {
            notes.beginDraft({ kind: 'whole' });
          }}
          expanded={expanded}
          focusIri={notes.focusedIri}
          onFocused={notes.focused}
        />
      )}

      {notes.status === null ? null : (
        <p className="media-notes-status" role="status">
          {notes.status}
        </p>
      )}
    </section>
  );
};
