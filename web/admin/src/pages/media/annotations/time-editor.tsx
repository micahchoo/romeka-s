/**
 * The time editor: the range bar, the two fields that name a range, the keys
 * that take a range from the player, and on a video the boxes on its frames.
 *
 * On a media page it draws on the stage (`StageSlots`): its tools above the
 * player, the boxes exactly over the picture, the bar and the fields under it.
 * The fields show while a note with a range is open, since they edit that note.
 *
 * It writes nothing. **Note from here** hands a range to the panel through
 * `onDraw`, which opens the note form, and the Start and End fields edit the
 * open draft through `onRange`; the panel owns the save. A reader gets the bar
 * and its segments and no control.
 *
 * The player belongs to the stage (A-1), which is why the page hands it here:
 * one `<audio>`/`<video>` on the page means the bar's line moves while it
 * plays, and "Play this range" seeks the element the editor is listening to.
 *
 * A range that the server would refuse is stated at the field as it is typed,
 * and the panel refuses to send it, so the reason is never hidden behind a
 * disabled Save.
 */
import type { KeyboardEvent as KeyboardEventOf, ReactElement } from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslator } from '../../../i18n';
import type { KeyOf, Translator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { PlacedNote } from './note-list';
import { pause, play, positionOf, seek } from './player';
import type { StageSlots } from '../stage/slots';
import { FrameBoxes } from './frame-boxes';
import { InSlot } from './in-slot';
import { RangeBar } from './range-bar';
import type { TimeProblem, TimeRange } from './time-text';
import { clockText, defaultRange, parseClock } from './time-text';
import type { Note, Selector, Surface } from './wire';
import { spanOf } from './wire';
import './time-editor.css';

export interface TimeEditorProps {
  /** The stage's places to draw in, or null to render in the panel. */
  readonly slots?: StageSlots | null;
  readonly surface: Extract<Surface, { readonly kind: 'time' }>;
  readonly rows: readonly PlacedNote[];
  readonly selected: string | null;
  readonly draft: { readonly iri: string | null; readonly selector: Selector } | null;
  readonly writable: boolean;
  /** The stage's player element, or `null` when the page has none. */
  readonly player: HTMLMediaElement | null;
  /** The range the fields show: the open draft's, else the next one. */
  readonly range: TimeRange | null;
  /** Why the range cannot be saved, computed by the panel. */
  readonly problem: TimeProblem | null;
  readonly onRange: (startMs: number, endMs: number) => void;
  readonly onDraw: (selector: Selector) => void;
  readonly onSelect: (note: Note) => void;
  /** The open draft's box moved by the keys. */
  readonly onReshape?: (selector: Selector) => void;
}

/** The sentence for a range the server would refuse, at the range itself. */
const problemText = (
  kind: TimeProblem,
  at: TimeRange,
  length: number | null,
  translate: Translator<KeyOf<typeof annotations>>,
): string =>
  kind === 'order'
    ? translate.t('time.problem.order', { start: clockText(at.startMs) })
    : translate.t('time.problem.pastEnd', {
        length: length === null ? '' : clockText(length),
      });

export const TimeEditor = (props: TimeEditorProps): ReactElement => {
  const translator = useTranslator(annotations);
  const { t } = translator;
  const { surface, rows, selected, draft, writable, player, range, problem } = props;
  const durationMs = surface.durationMs;
  const [startText, setStartText] = useState('');
  const [endText, setEndText] = useState('');
  const [bad, setBad] = useState<'start' | 'end' | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [playheadMs, setPlayheadMs] = useState(0);
  const [boxing, setBoxing] = useState(false);
  const slots = props.slots ?? null;
  const frame = surface.frame;
  const playingTo = useRef<number | null>(null);
  const latest = useRef<{ range: TimeRange | null; durationMs: number | null }>({
    range,
    durationMs,
  });
  latest.current = { range, durationMs };

  const startMs = range?.startMs ?? null;
  const endMs = range?.endMs ?? null;

  useEffect(() => {
    if (startMs === null || endMs === null) {
      return;
    }
    setStartText(clockText(startMs));
    setEndText(clockText(endMs));
  }, [startMs, endMs]);

  // One listener set for the player's own timeline, so the playhead is the
  // element's truth and the range's end stops playback wherever the editor
  // started it from.
  useEffect(() => {
    const element = player;
    if (element === null) {
      return;
    }
    const tick = (): void => {
      setPlayheadMs(positionOf(element));
      const stop = playingTo.current;
      if (stop !== null && positionOf(element) >= stop) {
        playingTo.current = null;
        pause(element);
      }
    };
    element.addEventListener('timeupdate', tick);
    element.addEventListener('seeked', tick);
    element.addEventListener('loadedmetadata', tick);
    tick();
    return () => {
      element.removeEventListener('timeupdate', tick);
      element.removeEventListener('seeked', tick);
      element.removeEventListener('loadedmetadata', tick);
    };
  }, [player]);

  /**
   * `[` and `]` belong to the player's own focus, which is where an editor is
   * after pressing play: one key each, no mode, and the position is read from
   * the element at the key, not from the last tick.
   */
  useEffect(() => {
    const element = player;
    if (element === null || !writable) {
      return;
    }
    const onKey = (event: KeyboardEvent): void => {
      const now = positionOf(element);
      const current = latest.current;
      if (event.key === '[') {
        event.preventDefault();
        const end =
          current.range !== null && current.range.endMs > now
            ? current.range.endMs
            : defaultRange(now, current.durationMs).endMs;
        setRefusal(null);
        props.onRange(now, end);
      }
      if (event.key === ']') {
        event.preventDefault();
        const start =
          current.range !== null && current.range.startMs < now
            ? current.range.startMs
            : Math.max(0, now - 5000);
        setRefusal(null);
        props.onRange(start, now);
      }
    };
    element.addEventListener('keydown', onKey);
    return () => {
      element.removeEventListener('keydown', onKey);
    };
  }, [player, writable, props]);

  const noteFromHere = (): void => {
    if (!writable) {
      return;
    }
    const next = defaultRange(positionOf(player), durationMs);
    pause(player);
    setBad(null);
    setRefusal(null);
    setBoxing(false);
    props.onDraw({ kind: 'time', startMs: next.startMs, endMs: next.endMs });
  };

  /** The Box tool holds the frame still, so the box is drawn on what it names. */
  const chooseBox = (): void => {
    if (!boxing) {
      pause(player);
    }
    setBoxing(!boxing);
    setRefusal(null);
  };

  const drawBox = (box: { readonly x: number; readonly y: number; readonly w: number; readonly h: number }): void => {
    const next = defaultRange(positionOf(player), durationMs);
    setBoxing(false);
    setBad(null);
    setRefusal(null);
    props.onDraw({ kind: 'box', ...box, startMs: next.startMs, endMs: next.endMs });
  };

  const setFromPlayer = (which: 'start' | 'end'): void => {
    const now = positionOf(player);
    setBad(null);
    setRefusal(null);
    if (which === 'start') {
      props.onRange(now, range !== null && range.endMs > now ? range.endMs : defaultRange(now, durationMs).endMs);
      return;
    }
    props.onRange(range !== null && range.startMs < now ? range.startMs : Math.max(0, now - 5000), now);
  };

  /** A typed field: the text stays as typed, and the range moves when it parses. */
  const typed = (which: 'start' | 'end', text: string): void => {
    if (which === 'start') {
      setStartText(text);
    } else {
      setEndText(text);
    }
    const milliseconds = parseClock(text);
    if (milliseconds === null) {
      setBad(which);
      return;
    }
    setBad(null);
    setRefusal(null);
    if (which === 'start') {
      props.onRange(milliseconds, endMs ?? defaultRange(milliseconds, durationMs).endMs);
      return;
    }
    props.onRange(startMs ?? Math.max(0, milliseconds - 5000), milliseconds);
  };

  const blur = (which: 'start' | 'end'): void => {
    // The canonical form comes back on blur: `1:02.5`, not `62` or `1:2.5`.
    setBad(null);
    if (which === 'start' && startMs !== null) {
      setStartText(clockText(startMs));
    }
    if (which === 'end' && endMs !== null) {
      setEndText(clockText(endMs));
    }
  };

  const playRange = (): void => {
    if (range === null) {
      setRefusal(t('time.noRange'));
      return;
    }
    if (problem !== null) {
      setRefusal(problemText(problem, range, durationMs, translator));
      return;
    }
    setRefusal(null);
    playingTo.current = range.endMs;
    seek(player, range.startMs);
    play(player);
  };

  const select = (note: Note): void => {
    props.onSelect(note);
    const span = spanOf(note.selector);
    if (span !== null) {
      seek(player, span.startMs);
    }
  };

  // The fields edit the open note's range, so they are there while one is.
  const editingRange = draft !== null && spanOf(draft.selector) !== null;

  const tools = writable ? (
    <div className="time-tools">
      <div className="time-toolbar">
        <button type="button" className="tool-plain" onClick={noteFromHere}>
          {t('time.fromHere')}
        </button>
        {frame === null ? null : (
          <button type="button" className="tool-plain" aria-pressed={boxing} onClick={chooseBox}>
            {t('time.tool.box')}
          </button>
        )}
        <button type="button" className="tool-plain" onClick={playRange}>
          {t('time.playRange')}
        </button>
      </div>
      {boxing ? <p className="time-hint">{t('time.hint.box')}</p> : null}
    </div>
  ) : null;

  return (
    <section className="time-editor" aria-label={t('time.label')}>
      <InSlot into={slots?.tools ?? null}>{tools}</InSlot>
      {frame === null ? null : (
        <InSlot into={slots?.over ?? null}>
          <FrameBoxes
            frame={frame}
            rows={rows}
            selected={selected}
            draft={draft}
            writable={writable}
            drawing={boxing}
            playheadMs={playheadMs}
            onDraw={drawBox}
            onSelect={select}
            onReshape={(selector) => {
              props.onReshape?.(selector);
            }}
            onCancel={() => {
              setBoxing(false);
            }}
          />
        </InSlot>
      )}
      <InSlot into={slots?.under ?? null}>
        <RangeBar
          durationMs={durationMs}
          rows={rows}
          selected={selected}
          draft={draft}
          playheadMs={playheadMs}
          onSelect={select}
        />

        {writable && editingRange ? (
          <>
            <div className="time-fields">
              <div className="time-field">
                <label className="note-field">
                  <span>{t('time.start')}</span>
                  <input
                    type="text"
                    inputMode="decimal"
                    value={startText}
                    aria-invalid={bad === 'start'}
                    onChange={(event) => {
                      typed('start', event.target.value);
                    }}
                    onBlur={() => {
                      blur('start');
                    }}
                    onKeyDown={(event: KeyboardEventOf<HTMLInputElement>) => {
                      if (event.key === 'Enter') {
                        event.preventDefault();
                        blur('start');
                      }
                    }}
                  />
                </label>
                {player === null ? null : (
                  <button
                    type="button"
                    className="time-set"
                    aria-label={t('time.setStartFromPlayer')}
                    onClick={() => {
                      setFromPlayer('start');
                    }}
                  >
                    {t('time.setFromPlayer')}
                  </button>
                )}
              </div>
              <div className="time-field">
                <label className="note-field">
                  <span>{t('time.end')}</span>
                  <input
                    type="text"
                    inputMode="decimal"
                    value={endText}
                    aria-invalid={bad === 'end'}
                    onChange={(event) => {
                      typed('end', event.target.value);
                    }}
                    onBlur={() => {
                      blur('end');
                    }}
                    onKeyDown={(event: KeyboardEventOf<HTMLInputElement>) => {
                      if (event.key === 'Enter') {
                        event.preventDefault();
                        blur('end');
                      }
                    }}
                  />
                </label>
                {player === null ? null : (
                  <button
                    type="button"
                    className="time-set"
                    aria-label={t('time.setEndFromPlayer')}
                    onClick={() => {
                      setFromPlayer('end');
                    }}
                  >
                    {t('time.setFromPlayer')}
                  </button>
                )}
              </div>
            </div>

            {bad === null ? null : <p className="note-field-error">{t('time.problem.format')}</p>}
            {problem === null || range === null ? null : (
              <p className="note-field-error" role="alert">
                {problemText(problem, range, durationMs, translator)}
              </p>
            )}
            <p className="time-hint">{t('time.hint')}</p>
          </>
        ) : null}
        {refusal === null ? null : (
          <p className="time-refusal" role="alert">
            {refusal}
          </p>
        )}
      </InSlot>
    </section>
  );
};
