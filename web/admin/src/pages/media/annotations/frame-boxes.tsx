/**
 * The boxes on a video's frames: an SVG exactly over the stage's player, in the
 * frame's own pixels (`Romeka-Surface` width and height), so a box lands on the
 * same part of the picture at any size the player is shown at.
 *
 * A box belongs to its range: it shows while the playhead is inside it, and the
 * open note's box shows whatever the time, so the editor can see what they are
 * changing. The layer takes the pointer only to draw (the Box tool) and on a
 * shown box, which opens its note; everywhere else the player's own controls
 * get the click.
 */
import type {
  KeyboardEvent as KeyboardEventOf,
  PointerEvent as PointerEventOf,
  ReactElement,
} from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { CanvasSize, Matrix, Point } from './draw';
import { boxBetween, dragRect, nudge, resize, snap, toCanvas } from './draw';
import type { PlacedNote } from './note-list';
import type { Note, Selector } from './wire';

export interface FrameBoxesProps {
  readonly frame: CanvasSize;
  readonly rows: readonly PlacedNote[];
  readonly selected: string | null;
  /** The open draft's shape: `iri` null means a note that is not saved yet. */
  readonly draft: { readonly iri: string | null; readonly selector: Selector } | null;
  readonly writable: boolean;
  /** The Box tool is pressed: a drag marks a box. */
  readonly drawing: boolean;
  readonly playheadMs: number;
  readonly onDraw: (box: { readonly x: number; readonly y: number; readonly w: number; readonly h: number }) => void;
  readonly onSelect: (note: Note) => void;
  /** The open draft's box moved or resized by the keys. */
  readonly onReshape: (selector: Selector) => void;
  readonly onCancel: () => void;
}

type BoxSelector = Extract<Selector, { readonly kind: 'box' }>;

interface Shown {
  readonly key: string;
  readonly box: BoxSelector;
  readonly number: number | null;
  readonly note: Note | null;
  readonly chosen: boolean;
}

const ARROWS: Readonly<Record<string, Point>> = {
  ArrowLeft: { x: -1, y: 0 },
  ArrowRight: { x: 1, y: 0 },
  ArrowUp: { x: 0, y: -1 },
  ArrowDown: { x: 0, y: 1 },
};

const inside = (box: BoxSelector, at: number): boolean => at >= box.startMs && at < box.endMs;

/** A box's number on a filled badge at its corner, as the list entry shows it. */
const Badge = ({
  x,
  y,
  unitPerPx,
  text,
}: {
  readonly x: number;
  readonly y: number;
  readonly unitPerPx: number;
  readonly text: string;
}): ReactElement => {
  const height = unitPerPx * 18;
  const width = unitPerPx * (10 + 7 * text.length);
  const left = x + unitPerPx * 4;
  const top = y + unitPerPx * 4;
  return (
    <g className="frame-box-label">
      <rect x={left} y={top} width={width} height={height} rx={unitPerPx * 4} />
      <text
        x={left + width / 2}
        y={top + height / 2}
        fontSize={unitPerPx * 12}
        textAnchor="middle"
        dominantBaseline="central"
      >
        {text}
      </text>
    </g>
  );
};

export const FrameBoxes = (props: FrameBoxesProps): ReactElement => {
  const { t } = useTranslator(annotations);
  const { frame, rows, selected, draft, writable, drawing, playheadMs } = props;
  const overlay = useRef<SVGSVGElement>(null);
  const [shownSize, setShownSize] = useState<CanvasSize>({ width: 0, height: 0 });
  const [drag, setDrag] = useState<{ readonly start: Point; readonly at: Point; readonly from: Point } | null>(
    null,
  );

  useEffect(() => {
    const element = overlay.current;
    if (element === null) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry !== undefined) {
        setShownSize({ width: entry.contentRect.width, height: entry.contentRect.height });
      }
    });
    observer.observe(element);
    const rect = element.getBoundingClientRect();
    setShownSize({ width: rect.width, height: rect.height });
    return () => {
      observer.disconnect();
    };
  }, []);

  // The frame is letterboxed inside the element the same way the player
  // letterboxes the picture, so one screen pixel is this many frame pixels.
  const scale =
    shownSize.width > 0 && shownSize.height > 0
      ? Math.min(shownSize.width / frame.width, shownSize.height / frame.height)
      : 0;
  const unitPerPx = scale > 0 ? 1 / scale : 1;

  const open: { readonly iri: string | null; readonly selector: BoxSelector } | null =
    draft !== null && draft.selector.kind === 'box' ? { iri: draft.iri, selector: draft.selector } : null;
  const shown: Shown[] = rows.flatMap((row) => {
    const box = open !== null && open.iri === row.note.iri ? open.selector : row.note.selector;
    if (box.kind !== 'box') {
      return [];
    }
    const chosen = row.note.iri === selected;
    return chosen || inside(box, playheadMs)
      ? [{ key: row.note.iri, box, number: row.number, note: row.note, chosen }]
      : [];
  });
  if (open !== null && open.iri === null) {
    shown.push({ key: 'draft', box: open.selector, number: null, note: null, chosen: true });
  }

  const inverse = (): Matrix | null => {
    const matrix = overlay.current?.getScreenCTM();
    return matrix === null || matrix === undefined ? null : matrix.inverse();
  };

  const onPointerDown = (event: PointerEventOf<SVGSVGElement>): void => {
    if (!drawing || !writable || event.button !== 0) {
      return;
    }
    const matrix = inverse();
    if (matrix === null) {
      return;
    }
    const at = snap(toCanvas(matrix, { x: event.clientX, y: event.clientY }), frame);
    event.currentTarget.setPointerCapture(event.pointerId);
    setDrag({ start: at, at, from: { x: event.clientX, y: event.clientY } });
  };

  const onPointerMove = (event: PointerEventOf<SVGSVGElement>): void => {
    const matrix = inverse();
    if (drag === null || matrix === null) {
      return;
    }
    setDrag({ ...drag, at: snap(toCanvas(matrix, { x: event.clientX, y: event.clientY }), frame) });
  };

  const onPointerUp = (event: PointerEventOf<SVGSVGElement>): void => {
    const matrix = inverse();
    const live = drag;
    setDrag(null);
    if (live === null || matrix === null) {
      return;
    }
    const outcome = dragRect(live.from, { x: event.clientX, y: event.clientY }, matrix, frame);
    if (outcome.gesture === 'rect' && outcome.selector.kind === 'rect') {
      const { x, y, w, h } = outcome.selector;
      props.onDraw({ x, y, w, h });
    }
  };

  /** The open box moves by the arrows, and grows by Alt and the arrows. */
  const onKeyDown = (event: KeyboardEventOf<SVGSVGElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault();
      setDrag(null);
      props.onCancel();
      return;
    }
    const arrow = ARROWS[event.key];
    if (arrow === undefined || open === null || !writable) {
      return;
    }
    event.preventDefault();
    const box = open.selector;
    const distance = event.shiftKey ? 10 : 1;
    const region = { kind: 'rect' as const, x: box.x, y: box.y, w: box.w, h: box.h };
    const moved = event.altKey
      ? resize(region, arrow.x * distance, arrow.y * distance, frame)
      : nudge(region, arrow.x * distance, arrow.y * distance, frame);
    if (moved.kind === 'rect') {
      props.onReshape({ ...box, x: moved.x, y: moved.y, w: moved.w, h: moved.h });
    }
  };

  const live = drag === null ? null : boxBetween(drag.start, drag.at);

  return (
    <svg
      ref={overlay}
      className={drawing ? 'frame-boxes is-drawing' : 'frame-boxes'}
      viewBox={`0 0 ${String(frame.width)} ${String(frame.height)}`}
      preserveAspectRatio="xMidYMid meet"
      role={writable ? 'application' : 'presentation'}
      aria-label={writable ? t('time.frame') : undefined}
      tabIndex={writable && (drawing || open !== null) ? 0 : -1}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => {
        setDrag(null);
      }}
      onKeyDown={onKeyDown}
    >
      {shown.map(({ key, box, number, note, chosen }) => (
        <g
          key={key}
          className={`frame-box${chosen ? ' is-selected' : ''}${note === null ? ' is-draft' : ''}`}
          data-iri={note?.iri}
          data-number={number ?? undefined}
          onClick={
            note === null || drawing
              ? undefined
              : () => {
                  props.onSelect(note);
                }
          }
        >
          <rect className="frame-box-halo" x={box.x} y={box.y} width={box.w} height={box.h} />
          <rect className="frame-box-line" x={box.x} y={box.y} width={box.w} height={box.h} />
          <Badge
            x={box.x}
            y={box.y}
            unitPerPx={unitPerPx}
            text={number === null ? t('region.draft') : String(number)}
          />
        </g>
      ))}
      {live === null || live.kind !== 'rect' ? null : (
        <rect className="frame-box-live" x={live.x} y={live.y} width={live.w} height={live.h} />
      )}
    </svg>
  );
};
