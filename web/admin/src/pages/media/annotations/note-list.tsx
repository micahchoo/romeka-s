/**
 * The notes list: one entry per note, ordered by its place in the media.
 *
 * Ordering is a pure function of the server's selectors: regions run
 * top-to-bottom then left-to-right, time ranges by their start, and whole-file
 * notes by date. A region's list number is the number its shape carries in the
 * region editor, so the two never disagree.
 *
 * The filter row appears once 20 notes exist; the list shows 50 at a time and
 * offers more. All of it is the panel's own data, so no count here is a claim
 * about what a visitor sees.
 */
import type { ReactElement, ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';
import { formatDateTime } from '../../../app/format';
import { useTranslator } from '../../../i18n';
import type { Translator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { KeyOf } from '../../../i18n';
import type { Note, Selector, Surface } from './wire';
import { clockText } from './time-text';

/** Where a note sits in the media. */
export type Place = 'region' | 'time' | 'whole';

/** A note with the place that orders it and its shape's number. */
export interface PlacedNote {
  readonly note: Note;
  readonly place: Place;
  /** The number its shape carries: a region's, or a box's on a video; else 0. */
  readonly number: number;
}

/** At this many notes the list offers a text filter and kind chips. */
const FILTER_FROM = 20;

/** How many entries the list shows before "Show 50 more". */
const PAGE = 50;

/** The notes of one media, in the order a reader walks the media. */
export const placedNotes = (notes: readonly Note[]): readonly PlacedNote[] => {
  const rows = [...notes].sort((left, right) => {
    const a = orderOf(left);
    const b = orderOf(right);
    // A tie keeps the server's order (id order), which is creation order.
    return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
  });
  // A picture's regions and a video's boxes are never on one media, so one
  // count numbers whichever shapes it has, in the order the list shows them.
  let number = 0;
  return rows.map((note) => {
    const place = placeOf(note.selector);
    const shaped = place === 'region' || note.selector.kind === 'box';
    if (shaped) {
      number += 1;
    }
    return { note, place, number: shaped ? number : 0 };
  });
};

/** How many notes each place holds, for the facts line and the filter. */
export const countsOf = (
  notes: readonly Note[],
): Readonly<Record<Place, number>> => {
  const counts: Record<Place, number> = { region: 0, time: 0, whole: 0 };
  for (const note of notes) {
    counts[placeOf(note.selector)] += 1;
  }
  return counts;
};

const placeOf = (selector: Selector): Place => {
  switch (selector.kind) {
    case 'rect':
    case 'polygon':
      return 'region';
    case 'time':
    case 'box':
      return 'time';
    case 'whole':
      return 'whole';
  }
};

/** The sort key: place kind, then its coordinate or time, then its position. */
const orderOf = (note: Note): readonly [number, number, number] => {
  switch (note.selector.kind) {
    case 'rect':
      return [0, note.selector.y, note.selector.x];
    case 'polygon':
      return [0, least(note.selector.points.map(([, y]) => y)), least(note.selector.points.map(([x]) => x))];
    case 'time':
    case 'box':
      return [1, note.selector.startMs, 0];
    case 'whole':
      return [2, changedAt(note), 0];
  }
};

const least = (values: readonly number[]): number =>
  values.reduce((low, value) => Math.min(low, value), Number.POSITIVE_INFINITY);

const changedAt = (note: Note): number =>
  note.modified === null ? 0 : Number.isNaN(Date.parse(note.modified)) ? 0 : Date.parse(note.modified);


/** The first line of a note, short enough for one place in a list. */
export const snippetOf = (text: string): string => {
  const line = text.split('\n')[0]?.trim() ?? '';
  return line.length > 120 ? `${line.slice(0, 119)}…` : line;
};

type AnnotationsKey = KeyOf<typeof annotations>;

/** The filter chips, in the order a note's place runs. */
const PLACES: readonly Place[] = ['region', 'time', 'whole'];

const FILTER_LABEL: Readonly<Record<Place, AnnotationsKey>> = {
  region: 'notes.filter.region',
  time: 'notes.filter.time',
  whole: 'notes.filter.whole',
};

export const placeLabel = (
  row: PlacedNote,
  surface: Surface,
  translator: Translator<AnnotationsKey>,
): string => {
  switch (row.place) {
    case 'region':
      return translator.t('notes.region', { number: row.number });
    case 'time': {
      const { selector } = row.note;
      if (selector.kind === 'box') {
        return translator.t('notes.box', {
          number: row.number,
          start: clockText(selector.startMs),
          end: clockText(selector.endMs),
        });
      }
      return selector.kind === 'time'
        ? translator.t('notes.timeRange', {
            start: clockText(selector.startMs),
            end: clockText(selector.endMs),
          })
        : '';
    }
    case 'whole':
      return translator.t(
        surface.kind === 'image'
          ? 'notes.wholeImage'
          : surface.kind === 'time'
            ? 'notes.wholeTime'
            : 'notes.wholeFile',
      );
  }
};

export interface NoteListProps {
  readonly surface: Surface;
  readonly notes: readonly Note[];
  readonly writable: boolean;
  readonly selected: string | null;
  readonly onSelect: (note: Note) => void;
  /** Starts a whole-file note on this media. */
  readonly onAdd: () => void;
  /** The open form for one note, or `null` when it is closed. */
  readonly expanded: (note: Note) => ReactNode | null;
  /** The entry to focus after a delete; the panel focuses its own heading otherwise. */
  readonly focusIri: string | null;
  readonly onFocused: () => void;
}

export const NoteList = (props: NoteListProps): ReactElement => {
  const translator = useTranslator(annotations);
  const { t, locale } = translator;
  const [query, setQuery] = useState('');
  const [places, setPlaces] = useState<ReadonlySet<Place>>(new Set());
  const [visible, setVisible] = useState(PAGE);
  const entries = useRef(new Map<string, HTMLButtonElement>());

  useEffect(() => {
    if (props.focusIri === null) {
      return;
    }
    entries.current.get(props.focusIri)?.focus();
    props.onFocused();
  }, [props.focusIri, props.onFocused]);

  const placed = placedNotes(props.notes);
  const counts = countsOf(props.notes);
  const filtered = placed.filter(
    (row) =>
      (places.size === 0 || places.has(row.place)) &&
      (query === '' || row.note.text.toLowerCase().includes(query.toLowerCase())),
  );
  const shown = filtered.slice(0, visible);
  const filtering = query !== '' || places.size > 0;
  const total = props.notes.length;

  const togglePlace = (place: Place): void => {
    setPlaces((current) => {
      const next = new Set(current);
      if (next.has(place)) {
        next.delete(place);
      } else {
        next.add(place);
      }
      return next;
    });
  };

  const clear = (): void => {
    setQuery('');
    setPlaces(new Set());
  };

  if (total === 0) {
    return (
      <div className="media-notes-list">
        <p className="media-notes-empty">
          {t(
            props.surface.kind === 'image'
              ? 'notes.empty.image'
              : props.surface.kind === 'time'
                ? 'notes.empty.time'
                : 'notes.empty.file',
          )}
        </p>
        {props.writable ? (
          <button type="button" onClick={props.onAdd}>
            {t(
              props.surface.kind === 'image'
                ? 'notes.add.image'
                : props.surface.kind === 'time'
                  ? 'notes.add.time'
                  : 'notes.add.file',
            )}
          </button>
        ) : null}
      </div>
    );
  }

  return (
    <div className="media-notes-list">
      {total >= FILTER_FROM ? (
        <div className="media-notes-filters" role="group" aria-label={t('notes.filterLabel')}>
          <label className="media-notes-filter-text">
            <span>{t('notes.filterPlaceholder')}</span>
            <input
              type="search"
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
              }}
            />
          </label>
          {PLACES.filter((place) => counts[place] > 0)
            .map((place) => (
              <button
                key={place}
                type="button"
                aria-pressed={places.has(place)}
                onClick={() => {
                  togglePlace(place);
                }}
              >
                {t(FILTER_LABEL[place])}
              </button>
            ))}
          {filtering ? (
            <button type="button" onClick={clear}>
              {t('notes.clearFilter')}
            </button>
          ) : null}
        </div>
      ) : null}

      {filtering ? (
        <p className="media-notes-count">
          {query === ''
            ? t('notes.shownCount', { shown: filtered.length, total })
            : t('notes.matchCount', { shown: filtered.length, total, query })}
        </p>
      ) : null}

      {filtered.length === 0 ? (
        <p className="media-notes-empty">{t('notes.noMatch')}</p>
      ) : (
        <ol className="media-notes-entries" aria-label={t('notes.listLabel')}>
          {shown.map((row) => {
            const open = props.selected === row.note.iri;
            const snippet = snippetOf(row.note.text);
            const content = props.expanded(row.note);
            return (
              <li key={row.note.iri} className={`note-entry${open ? ' is-selected' : ''}`}>
                <button
                  type="button"
                  className="note-open"
                  aria-expanded={open}
                  ref={(element) => {
                    if (element === null) {
                      entries.current.delete(row.note.iri);
                    } else {
                      entries.current.set(row.note.iri, element);
                    }
                  }}
                  onClick={() => {
                    props.onSelect(row.note);
                  }}
                >
                  {row.number === 0 ? null : (
                    <span className="note-number" aria-hidden="true">
                      {row.number}
                    </span>
                  )}
                  <span className="note-place">{placeLabel(row, props.surface, translator)}</span>
                  <span className="note-snippet">{snippet === '' ? t('notes.noText') : snippet}</span>
                  <span className="note-mark" aria-hidden="true">
                    {open ? '▾' : '▸'}
                  </span>
                </button>
                {content === null ? null : (
                  <div className="note-expanded">
                    <p className="note-meta">
                      {row.note.modified === null
                        ? null
                        : t('notes.changed', { when: formatDateTime(row.note.modified, locale) })}
                      {row.note.fromTrack ? ` ${t('notes.fromTrack')}` : ''}
                    </p>
                    {content}
                  </div>
                )}
              </li>
            );
          })}
        </ol>
      )}

      {shown.length < filtered.length ? (
        <div className="media-notes-more">
          <p className="media-notes-count">{t('notes.shownCount', { shown: shown.length, total })}</p>
          <button
            type="button"
            onClick={() => {
              setVisible((current) => current + PAGE);
            }}
          >
            {t('notes.showMore')}
          </button>
        </div>
      ) : null}
    </div>
  );
};
