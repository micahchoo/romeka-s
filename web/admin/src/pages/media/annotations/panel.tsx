/**
 * The notes panel on a media page: the facts line, the list, the form that
 * writes a note, and the conflict view when a colleague saved first.
 *
 * The state machine (the read, the drafts, the conditional writes) is
 * `use-notes.ts`; this file renders it and owns no request of its own.
 *
 * The region editor (A-4) and the time editor (A-5) mount at
 * `.media-notes-surface`: they read `surface` and `notes`, call `open` with the
 * entry they select, and start a draft with a drawn selector through
 * `beginDraft`. This unit starts whole-file notes only.
 */
import type { ReactElement, ReactNode } from 'react';
import { Suspense, lazy, useEffect, useId, useRef } from 'react';
import type { Media } from '../../../api/types';
import { ErrorNotice } from '../../../app/errors';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import type { Editing } from './use-notes';
import type { Note, Selector } from './wire';
import { NoteList, placedNotes } from './note-list';
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

export const MediaNotes = ({ media }: { readonly media: Media }): ReactElement => {
  const { t } = useTranslator(annotations, common);
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const notes = useNotes(media);
  const { load, ready, editing } = notes;
  const { focusedHeading, clearFocusedHeading } = notes;

  useEffect(() => {
    if (focusedHeading) {
      heading.current?.focus();
      clearFocusedHeading();
    }
  }, [focusedHeading, clearFocusedHeading]);

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

      {/* The region editor (A-4) mounts here; the time editor (A-5) joins it. */}
      {ready === null ? null : (
        <div className="media-notes-surface" data-surface={ready.surface.kind}>
          {ready.surface.kind === 'image' ? (
            <Suspense fallback={null}>
              <RegionEditor
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
          ) : null}
        </div>
      )}

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
          onSelect={notes.open}
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
