/**
 * The item page's notes, drawn in two places from one read: the first file's
 * summary beside its card, and "All notes in this item" grouped by file.
 *
 * Every note links to the media page's notes panel at that note, which is the
 * only place a note is opened, seeked or highlighted (`annotations/link.ts`).
 * A file's group is capped, so an item with a thousand notes still renders a
 * list a reader can scan; the rest are the media page's own pages.
 */
import type { ReactElement } from 'react';
import { useId } from 'react';
import type { Media } from '../../../api/types';
import type { AsyncState } from '../../../app/useAsync';
import { KIND_LABEL, mediaKind, mediaName } from '../../../app/media';
import { useTranslator } from '../../../i18n';
import type { KeyOf, Translator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import { common } from '../../../i18n/namespaces/common';
import { resources } from '../../../i18n/namespaces/resources';
import { annotateHref, mediaHref, noteHref } from '../../media/annotations/link';
import { placeLabel, placedNotes, snippetOf } from '../../media/annotations/note-list';
import type { PlacedNote } from '../../media/annotations/note-list';
import type { Surface } from '../../media/annotations/wire';
import { noteIdOf } from '../../media/annotations/wire';
import type { ItemNotesRead } from './read';
import './item-notes.css';

/** How many notes a file's group shows before it points at the media page. */
const PER_FILE = 20;

/** How many of the first file's notes the inline summary shows. */
const FIRST_SHOWN = 3;

type SectionsTranslator = Translator<
  KeyOf<typeof annotations> | KeyOf<typeof common> | KeyOf<typeof resources>
>;

/** A note's place, then its first line, as one link to the note's own row. */
const NoteLine = ({
  row,
  media,
  surface,
  translator,
}: {
  readonly row: PlacedNote;
  readonly media: Media;
  readonly surface: Surface;
  readonly translator: SectionsTranslator;
}): ReactElement => {
  const snippet = snippetOf(row.note.text);
  return (
    <li className="item-note-line">
      <a className="item-note-place" href={noteHref(media.id, noteIdOf(row.note.iri))}>
        {placeLabel(row, surface, translator)}
      </a>{' '}
      <span className="item-note-text">
        {snippet === '' ? translator.t('notes.noText') : snippet}
      </span>
    </li>
  );
};

export const ItemNotesSections = ({
  media,
  state,
  totalFiles,
  onRetry,
}: {
  /** The files on the item page, in item order. */
  readonly media: readonly Media[];
  readonly state: AsyncState<ItemNotesRead>;
  /** The count the item read states, which may exceed the files on this page. */
  readonly totalFiles: number;
  readonly onRetry: () => void;
}): ReactElement => {
  const translator = useTranslator(annotations, common, resources);
  const { t } = translator;
  const firstHeading = useId();
  const allHeading = useId();

  if (state.status === 'loading') {
    return (
      <p className="item-notes-status" role="status">
        {t('notes.loading')}
      </p>
    );
  }
  if (state.status === 'error') {
    return (
      <div className="item-notes-status">
        <p>{t('notes.itemFailed')}</p>
        <button type="button" onClick={onRetry}>
          {t('notes.retry')}
        </button>
      </div>
    );
  }

  const read = state.data;
  const first = media[0];
  if (first === undefined) {
    return <p className="item-notes-status">{t('notes.itemEmpty')}</p>;
  }
  const firstCount = read.counts.get(first.id)?.count ?? 0;
  const firstNotes = read.files.get(first.id);
  const firstName = mediaName(first) ?? t('common.untitled');
  const firstRows = firstNotes === undefined ? [] : placedNotes(firstNotes.notes);
  const grouped = media.flatMap((entry) => {
    const known = read.counts.get(entry.id);
    const count = known?.count ?? 0;
    const file = read.files.get(entry.id);
    return count === 0 || file === undefined
      ? []
      : [{ entry, file, count, writable: known?.writable === true }];
  });

  return (
    <>
      <section className="item-first-notes" aria-labelledby={firstHeading}>
        <h3 id={firstHeading}>{t('notes.firstHeading', { name: firstName })}</h3>
        {firstCount === 0 || firstNotes === undefined ? (
          <p className="item-notes-status">{t('notes.firstNone')}</p>
        ) : (
          <>
            <p className="item-notes-count">
              {t(firstCount === 1 ? 'notes.facts.fileOne' : 'notes.facts.file', {
                total: firstCount,
              })}
            </p>
            <ol className="item-note-lines">
              {firstRows.slice(0, FIRST_SHOWN).map((row) => (
                <NoteLine
                  key={row.note.iri}
                  row={row}
                  media={first}
                  surface={firstNotes.surface}
                  translator={translator}
                />
              ))}
            </ol>
            {firstCount > FIRST_SHOWN ? (
              <a className="item-notes-more" href={mediaHref(first.id)}>
                {t('notes.firstMore', { count: firstCount - FIRST_SHOWN })}
              </a>
            ) : null}
          </>
        )}
      </section>

      <section className="item-all-notes" aria-labelledby={allHeading}>
        <h3 id={allHeading}>{t('notes.itemHeading')}</h3>
        {grouped.length === 0 ? (
          <p className="item-notes-status">{t('notes.itemEmpty')}</p>
        ) : (
          <>
            <p className="item-notes-count">
              {t('notes.itemFiles', { files: grouped.length, total: media.length })}
            </p>
            {totalFiles > media.length ? (
              <p className="item-notes-status">
                {t('notes.itemCapped', { shown: media.length, total: totalFiles })}
              </p>
            ) : null}
            {grouped.map(({ entry, file, count, writable }) => {
              const shown = Math.min(PER_FILE, file.notes.length);
              return (
                <div className="item-notes-file" key={String(entry.id)}>
                  <h4>
                    <a href={writable ? annotateHref(entry.id) : mediaHref(entry.id)}>
                      {mediaName(entry) ?? t('common.untitled')}
                    </a>
                    <span className="item-notes-kind">{t(KIND_LABEL[mediaKind(entry)])}</span>
                  </h4>
                  <ol className="item-note-lines">
                    {placedNotes(file.notes)
                      .slice(0, PER_FILE)
                      .map((row) => (
                        <NoteLine
                          key={row.note.iri}
                          row={row}
                          media={entry}
                          surface={file.surface}
                          translator={translator}
                        />
                      ))}
                  </ol>
                  {count > shown ? (
                    <a className="item-notes-more" href={mediaHref(entry.id)}>
                      {t('notes.itemMore', { count: count - shown })}
                    </a>
                  ) : null}
                </div>
              );
            })}
          </>
        )}
        {read.unread.length === 0 ? null : (
          <p className="item-notes-status">
            {t('notes.itemUnread', { count: read.unread.length })}
          </p>
        )}
      </section>
    </>
  );
};
