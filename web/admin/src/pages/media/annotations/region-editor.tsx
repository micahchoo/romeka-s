/**
 * The region editor: the picture with its regions drawn on it, the tools that
 * mark a new one, and the keys that do everything the pointer does.
 *
 * The overlay is an SVG whose `viewBox` is the media's own canvas, over a
 * rendition sized to the box, so one user unit is one canvas pixel whatever the
 * picture's own resolution is. The container says what the canvas is, and the
 * browser's screen matrix carries the pointer from the derivative to it, so a
 * rendition smaller than the canvas, as a camera RAW's is, still maps exactly
 * (`draw.ts`).
 *
 * Nothing here writes. A completed shape goes to the panel through `onDraw`,
 * which opens the note form, and a moved shape through `onMove`, which edits the
 * open draft; the panel owns the save. A reader gets the picture, its numbered
 * shapes and nothing to click.
 */
import type {
  KeyboardEvent as KeyboardEventOf,
  PointerEvent as PointerEventOf,
  ReactElement,
} from 'react';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useTranslator } from '../../../i18n';
import { annotations } from '../../../i18n/namespaces/annotations';
import type { CanvasSize, Matrix, Point, Region } from './draw';
import {
  addCorner,
  boundsOf,
  closePolygon,
  CORNER_COUNT_FROM,
  dragRect,
  MAX_CORNERS,
  nextZoom,
  nudge,
  onFirstCorner,
  regionAt,
  regionOf,
  renditionUrl,
  resize,
  selectorOf,
  snap,
  toCanvas,
  CLOSE_PX,
} from './draw';
import type { PlacedNote } from './note-list';
import type { DrawnRegion, LiveBox } from './region-shapes';
import { RegionShapes } from './region-shapes';
import type { Tool } from './region-toolbar';
import { RegionToolbar } from './region-toolbar';
import type { Note, Selector, Surface } from './wire';
import './region-editor.css';

export interface RegionEditorProps {
  readonly surface: Extract<Surface, { readonly kind: 'image' }>;
  /** The media's notes in list order, so a shape's number is its entry's number. */
  readonly rows: readonly PlacedNote[];
  /** The open entry's IRI, which the list and the overlay share. */
  readonly selected: string | null;
  /** The open draft's shape: `iri` null means a note that is not saved yet. */
  readonly draft: { readonly iri: string | null; readonly selector: Selector } | null;
  readonly writable: boolean;
  readonly onDraw: (selector: Selector) => void;
  readonly onSelect: (note: Note) => void;
  readonly onMove: (note: Note, selector: Selector) => void;
}

const ARROWS: Readonly<Record<string, Point>> = {
  ArrowLeft: { x: -1, y: 0 },
  ArrowRight: { x: 1, y: 0 },
  ArrowUp: { x: 0, y: -1 },
  ArrowDown: { x: 0, y: 1 },
};

const arrowOf = (key: string): Point | null => ARROWS[key] ?? null;

export const RegionEditor = (props: RegionEditorProps): ReactElement => {
  const { t } = useTranslator(annotations);
  const { surface, rows, selected, draft, writable, onDraw, onSelect, onMove } = props;
  const size: CanvasSize = { width: surface.width, height: surface.height };
  const scroll = useRef<HTMLDivElement>(null);
  const overlay = useRef<SVGSVGElement>(null);
  const zoomedFrom = useRef<{
    readonly zoom: number;
    readonly left: number;
    readonly top: number;
    readonly width: number;
    readonly height: number;
  } | null>(null);
  const [box, setBox] = useState(0);
  const [zoom, setZoom] = useState(1);
  const [tool, setTool] = useState<Tool>('move');
  const [advanced, setAdvanced] = useState(false);
  const [hidden, setHidden] = useState(false);
  const [corners, setCorners] = useState<readonly Point[]>([]);
  const [drag, setDrag] = useState<LiveBox | null>(null);
  const [cursor, setCursor] = useState<Point>(() => ({
    x: Math.round(surface.width / 2),
    y: Math.round(surface.height / 2),
  }));
  const [refusal, setRefusal] = useState<string | null>(null);

  useEffect(() => {
    const element = scroll.current;
    if (element === null) {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry !== undefined) {
        setBox(entry.contentRect.width);
      }
    });
    observer.observe(element);
    setBox(element.clientWidth);
    return () => {
      observer.disconnect();
    };
  }, []);

  const shownWidth = box * zoom;
  const unitPerPx = shownWidth > 0 ? surface.width / shownWidth : 1;

  const geometryOf = (row: PlacedNote): Region | null =>
    row.number <= 0
      ? null
      : regionOf(draft !== null && draft.iri === row.note.iri ? draft.selector : row.note.selector);

  const regions = rows.flatMap((row) => {
    const geometry = geometryOf(row);
    return geometry === null ? [] : [{ row, geometry }];
  });
  const fresh = draft === null || draft.iri !== null ? null : regionOf(draft.selector);
  const selectedRegion: DrawnRegion | null =
    regions.find((entry) => entry.row.note.iri === selected) ?? null;
  const selectedBounds = selectedRegion === null ? null : boundsOf(selectedRegion.geometry);

  // Zooming grows the figure inside the scroll box, so scrolling is what keeps
  // the editor's place: the selected shape stays where it was on screen, or the
  // middle of the box when nothing is selected.
  useLayoutEffect(() => {
    const element = scroll.current;
    const from = zoomedFrom.current;
    zoomedFrom.current = null;
    if (element === null || from === null) {
      return;
    }
    const ratio = zoom / from.zoom;
    if (ratio === 1) {
      return;
    }
    const anchor = selectedRegion === null ? null : boundsOf(selectedRegion.geometry);
    const fx = anchor === null ? 0.5 : (anchor.x + anchor.width / 2) / surface.width;
    const fy = anchor === null ? 0.5 : (anchor.y + anchor.height / 2) / surface.height;
    element.scrollLeft = from.left + fx * from.width * (ratio - 1);
    element.scrollTop = from.top + fy * from.height * (ratio - 1);
  }, [zoom, selectedRegion, surface.width, surface.height]);

  /** The screen-to-canvas matrix, from the overlay's own current geometry. */
  const inverse = (): Matrix | null => {
    const matrix = overlay.current?.getScreenCTM();
    if (matrix === null || matrix === undefined) {
      return null;
    }
    return matrix.inverse();
  };

  const canvasAt = (client: Point): Point | null => {
    const matrix = inverse();
    return matrix === null ? null : snap(toCanvas(matrix, client), size);
  };

  const selectAt = (at: Point): void => {
    // A shape carries its own note, so an overlay click opens the same entry the
    // list opens; the panel keeps the one selection both read.
    const hit = regionAt(
      regions.map((entry) => entry.geometry),
      at,
    );
    const entry = hit === null ? undefined : regions[hit];
    if (entry !== undefined) {
      onSelect(entry.row.note);
    }
  };

  const zoomTo = (direction: -1 | 1): void => {
    const element = scroll.current;
    const next = nextZoom(zoom, direction);
    if (element === null || next === zoom) {
      return;
    }
    zoomedFrom.current = {
      zoom,
      left: element.scrollLeft,
      top: element.scrollTop,
      width: element.scrollWidth,
      height: element.scrollHeight,
    };
    setZoom(next);
  };

  const chooseTool = (next: Tool): void => {
    setTool(next);
    setCorners([]);
    setDrag(null);
    setRefusal(null);
    if (next === 'polygon') {
      setCursor({ x: Math.round(surface.width / 2), y: Math.round(surface.height / 2) });
    }
  };

  const addPoint = (at: Point): void => {
    const next = addCorner(corners, at);
    if (next === null) {
      setRefusal(t('region.tooMany', { max: MAX_CORNERS }));
      return;
    }
    setRefusal(null);
    setCorners(next);
  };

  const finish = (): void => {
    const shape = closePolygon(corners);
    if (shape === null) {
      setRefusal(t('region.tooFew'));
      return;
    }
    setCorners([]);
    setRefusal(null);
    setTool('move');
    onDraw(selectorOf(shape));
  };

  const cancel = (): void => {
    setCorners([]);
    setDrag(null);
    setRefusal(null);
    setTool('move');
  };

  const addRegion = (region: Region): void => {
    setRefusal(null);
    onDraw(selectorOf(region));
  };

  const onPointerDown = (event: PointerEventOf<SVGSVGElement>): void => {
    if (!writable || event.button !== 0) {
      return;
    }
    const at = canvasAt({ x: event.clientX, y: event.clientY });
    if (at === null) {
      return;
    }
    if (tool === 'polygon') {
      if (onFirstCorner(corners, at, CLOSE_PX * unitPerPx)) {
        finish();
        return;
      }
      addPoint(at);
      return;
    }
    // Shift from Move is the spring-loaded rectangle, so the tenth shape of a
    // session never needs a trip to the toolbar.
    if (tool === 'rect' || event.shiftKey) {
      event.currentTarget.setPointerCapture(event.pointerId);
      setDrag({ start: at, at, from: { x: event.clientX, y: event.clientY } });
      return;
    }
    selectAt(at);
  };

  const onPointerMove = (event: PointerEventOf<SVGSVGElement>): void => {
    if (drag === null) {
      return;
    }
    const at = canvasAt({ x: event.clientX, y: event.clientY });
    if (at !== null) {
      setDrag({ ...drag, at });
    }
  };

  const onPointerUp = (event: PointerEventOf<SVGSVGElement>): void => {
    if (drag === null) {
      return;
    }
    const matrix = inverse();
    const live = drag;
    setDrag(null);
    if (matrix === null) {
      return;
    }
    const outcome = dragRect(live.from, { x: event.clientX, y: event.clientY }, matrix, size);
    if (outcome.gesture === 'click') {
      selectAt(outcome.at);
      return;
    }
    setTool('move');
    onDraw(selectorOf(outcome.selector));
  };

  /** A polygon's keys: Space adds the corner under the cursor, Enter closes. */
  const polygonKey = (event: KeyboardEventOf<SVGSVGElement>): void => {
    if (event.key === 'Enter') {
      event.preventDefault();
      finish();
      return;
    }
    if (event.key === 'Backspace') {
      event.preventDefault();
      setCorners((current) => current.slice(0, -1));
      return;
    }
    if (event.key === ' ') {
      event.preventDefault();
      addPoint(cursor);
      return;
    }
    const arrow = arrowOf(event.key);
    if (arrow === null) {
      return;
    }
    event.preventDefault();
    const distance = event.shiftKey ? 10 : 1;
    setCursor((current) =>
      snap({ x: current.x + arrow.x * distance, y: current.y + arrow.y * distance }, size),
    );
  };

  const onKeyDown = (event: KeyboardEventOf<SVGSVGElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault();
      cancel();
      return;
    }
    if (!writable) {
      return;
    }
    if (tool === 'polygon') {
      polygonKey(event);
      return;
    }
    const arrow = arrowOf(event.key);
    if (arrow === null || selectedRegion === null) {
      return;
    }
    const geometry = selectedRegion.geometry;
    if (event.altKey && geometry.kind === 'polygon') {
      return;
    }
    event.preventDefault();
    const distance = event.shiftKey ? 10 : 1;
    const moved =
      event.altKey && geometry.kind === 'rect'
        ? resize(geometry, arrow.x * distance, arrow.y * distance, size)
        : nudge(geometry, arrow.x * distance, arrow.y * distance, size);
    onMove(selectedRegion.row.note, selectorOf(moved));
  };

  const hint =
    tool === 'polygon'
      ? t('region.hint.polygon')
      : tool === 'rect'
        ? t('region.hint.rect')
        : t('region.hint.move');
  const source = renditionUrl(surface.service, size, shownWidth, Math.max(1, devicePixelRatio));

  return (
    <section className="region-editor" aria-label={t('region.label')}>
      {writable ? (
        <>
          <RegionToolbar
            tool={tool}
            zoom={zoom}
            hidden={hidden}
            advanced={advanced}
            size={size}
            onTool={chooseTool}
            onZoom={zoomTo}
            onHidden={setHidden}
            onAdvanced={setAdvanced}
            onAdd={addRegion}
          />
          <p className="region-hint">{hint}</p>
          {corners.length >= CORNER_COUNT_FROM ? (
            <p className="region-count">
              {t('region.corners', { count: corners.length, max: MAX_CORNERS })}
            </p>
          ) : null}
          {refusal === null ? null : (
            <p className="region-refusal" role="alert">
              {refusal}
            </p>
          )}
        </>
      ) : null}

      {selectedBounds === null || selectedRegion === null ? null : (
        <p className="region-selected" role="status">
          {t('region.selected', {
            number: selectedRegion.row.number,
            x: selectedBounds.x,
            y: selectedBounds.y,
            width: selectedBounds.width,
            height: selectedBounds.height,
          })}
        </p>
      )}

      <div className="region-scroll" ref={scroll} data-zoom={zoom}>
        <div
          className="region-figure"
          style={{
            width: `${String(zoom * 100)}%`,
            aspectRatio: `${String(surface.width)} / ${String(surface.height)}`,
          }}
        >
          <img src={source} alt="" />
          <svg
            ref={overlay}
            className={tool === 'move' ? 'region-overlay' : 'region-overlay is-drawing'}
            viewBox={`0 0 ${String(surface.width)} ${String(surface.height)}`}
            preserveAspectRatio="none"
            role={writable ? 'application' : 'presentation'}
            aria-label={writable ? t('region.canvas') : undefined}
            tabIndex={writable ? 0 : -1}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={() => {
              setDrag(null);
            }}
            onKeyDown={onKeyDown}
          >
            <RegionShapes
              regions={regions}
              draft={fresh}
              selected={selected}
              hidden={hidden}
              dense={regions.length > 200}
              unitPerPx={unitPerPx}
              drag={drag}
              corners={corners}
              cursor={cursor}
            />
          </svg>
        </div>
      </div>
    </section>
  );
};
