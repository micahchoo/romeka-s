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
import { spanOf } from './wire';

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

interface Range {
  readonly startMs: number;
  readonly endMs: number;
}

/**
 * The row each range sits on: the first row where it overlaps nothing already
 * placed, so a range never hides another and the bar grows only as tall as the
 * most ranges that overlap at one moment.
 */
export const lanesOf = (ranges: readonly Range[]): readonly number[] => {
  const rows: Range[][] = [];
  return ranges.map((range) => {
    const free = rows.findIndex((row) =>
      row.every((placed) => placed.endMs <= range.startMs || placed.startMs >= range.endMs),
    );
    const lane = free === -1 ? rows.length : free;
    (rows[lane] ??= []).push(range);
    return lane;
  });
};

/** One row of the bar, in rem: a segment and the gap under it. */
const LANE_REM = 1.25;

/** A segment's place on the track, in percent of the length, never inverted. */
const placeOf = (startMs: number, endMs: number, durationMs: number): { left: string; width: string } => {
  const left = Math.min(100, Math.max(0, (startMs / durationMs) * 100));
  const right = Math.min(100, Math.max(0, (endMs / durationMs) * 100));
  return { left: `${String(left)}%`, width: `${String(Math.max(0, right - left))}%` };
};

export const RangeBar = (props: RangeBarProps): ReactElement => {
  const { t } = useTranslator(annotations);
  const { durationMs, rows, draft } = props;
  // A box on a video's frames is a range too, so it has its segment.
  const times = rows.flatMap((row) => {
    const range = spanOf(row.note.selector);
    return range === null ? [] : [{ row, range }];
  });
  const draftRange = draft !== null && draft.iri === null ? spanOf(draft.selector) : null;
  const known = durationMs !== null && durationMs > 0;
  const lanes = lanesOf([...times.map(({ range }) => range), ...(draftRange === null ? [] : [draftRange])]);
  const laneCount = Math.max(1, ...lanes.map((lane) => lane + 1));
  const top = (index: number): string => `calc(${String((lanes[index] ?? 0) * LANE_REM)}rem + 3px)`;

  const segment = (
    note: Note,
    index: number,
    number: number,
    startMs: number,
    endMs: number,
    label: string,
  ): ReactElement => (
    <button
      key={note.iri}
      type="button"
      className={`range-segment${note.selector.kind === 'box' ? ' is-box' : ''}${
        props.selected === note.iri ? ' is-selected' : ''
      }`}
      style={known ? { ...placeOf(startMs, endMs, durationMs), top: top(index) } : undefined}
      aria-label={label}
      title={label}
      onClick={() => {
        props.onSelect(note);
      }}
    >
      {number > 0 ? <span aria-hidden="true">{number}</span> : null}
    </button>
  );

  return (
    <div className="range-bar" role="group" aria-label={t('time.bar')}>
      {durationMs === null ? <p className="range-unknown">{t('time.unknownLength')}</p> : null}
      <div
        className={known ? 'range-track' : 'range-track is-flat'}
        style={known ? { height: `calc(${String(laneCount * LANE_REM)}rem + 6px)` } : undefined}
      >
        {times.map(({ row, range }, index) =>
          segment(
            row.note,
            index,
            row.number,
            range.startMs,
            range.endMs,
            `${
              row.note.selector.kind === 'box'
                ? t('notes.box', {
                    number: row.number,
                    start: clockText(range.startMs),
                    end: clockText(range.endMs),
                  })
                : t('notes.timeRange', {
                    start: clockText(range.startMs),
                    end: clockText(range.endMs),
                  })
            }, ${row.note.text.split('\n')[0] ?? ''}`,
          ),
        )}
        {draftRange === null ? null : (
          <span
            className="range-draft"
            style={
              known
                ? { ...placeOf(draftRange.startMs, draftRange.endMs, durationMs), top: top(times.length) }
                : undefined
            }
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
      <div className="range-foot">
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
    </div>
  );
};
