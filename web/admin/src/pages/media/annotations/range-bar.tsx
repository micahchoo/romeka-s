/**
 * The range bar: the recording's length as a track, a segment per time note,
 * the unsaved draft's range, and the play position as a line.
 *
 * It draws only what the server and the player already state. A segment is a
 * button, so a saved range can be reached without the pointer and a click
 * selects the same entry the list opens; the bar itself holds no state.
 *
 * When the server knows no length (`Romeka-Surface` carries no `duration`),
 * there is no scale to draw against: the notes become plain chips in a row and
 * the bar says the length is unknown, rather than placing them on a made-up
 * track.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { PlacedNote } from './note-list';
import { clockText } from './time-text';
import type { Note, Selector } from './wire';

export interface RangeBarProps {
  readonly durationMs: number | null;
  /** Every note on the media, so the bar shows the same times the list does. */
  readonly rows: readonly PlacedNote[];
  /** The open entry's IRI, which the list and the bar share. */
  readonly selected: string | null;
  /** The open draft's range: `iri` null means a note that is not saved yet. */
  readonly draft: { readonly iri: string | null; readonly selector: Selector } | null;
  readonly playheadMs: number;
  readonly onSelect: (note: Note) => void;
}

/** A segment's place on the track, in percent of the length, never inverted. */
const placeOf = (startMs: number, endMs: number, durationMs: number): { left: string; width: string } => {
  const left = Math.min(100, Math.max(0, (startMs / durationMs) * 100));
  const right = Math.min(100, Math.max(0, (endMs / durationMs) * 100));
  return { left: `${String(left)}%`, width: `${String(Math.max(0, right - left))}%` };
};

export const RangeBar = (props: RangeBarProps): ReactElement => {
  const { t } = useTranslator(annotations);
  const { durationMs, rows, draft } = props;
  const times = rows.flatMap((row) =>
    row.note.selector.kind === 'time' ? [{ row, range: row.note.selector }] : [],
  );
  const draftRange =
    draft !== null && draft.iri === null && draft.selector.kind === 'time' ? draft.selector : null;
  const known = durationMs !== null && durationMs > 0;

  const segment = (
    note: Note,
    startMs: number,
    endMs: number,
    label: string,
  ): ReactElement => (
    <button
      key={note.iri}
      type="button"
      className={`range-segment${props.selected === note.iri ? ' is-selected' : ''}`}
      style={known ? placeOf(startMs, endMs, durationMs) : undefined}
      aria-label={label}
      title={label}
      onClick={() => {
        props.onSelect(note);
      }}
    />
  );

  return (
    <div className="range-bar" role="group" aria-label={t('time.bar')}>
      {durationMs === null ? <p className="range-unknown">{t('time.unknownLength')}</p> : null}
      <div className={known ? 'range-track' : 'range-track is-flat'}>
        {times.map(({ row, range }) =>
          segment(
            row.note,
            range.startMs,
            range.endMs,
            `${t('notes.timeRange', {
              start: clockText(range.startMs),
              end: clockText(range.endMs),
            })}, ${row.note.text.split('\n')[0] ?? ''}`,
          ),
        )}
        {draftRange === null ? null : (
          <span
            className="range-draft"
            style={known ? placeOf(draftRange.startMs, draftRange.endMs, durationMs) : undefined}
            aria-hidden="true"
          />
        )}
        {known ? (
          <span
            className="range-playhead"
            style={{ left: `${String(Math.min(100, Math.max(0, (props.playheadMs / durationMs) * 100)))}%` }}
            aria-hidden="true"
          />
        ) : null}
      </div>
      {draftRange === null ? null : (
        <p className="range-draft-note">
          {t('notes.draft')}: {t('notes.timeRange', {
            start: clockText(draftRange.startMs),
            end: clockText(draftRange.endMs),
          })}
        </p>
      )}
      {durationMs === null ? null : (
        <p className="range-end">
          <span className="range-end-label">{t('time.length')}</span> {clockText(durationMs)}
        </p>
      )}
    </div>
  );
};
