/**
 * The region editor's geometry, with no React and no DOM: a pointer in screen
 * pixels to a point on the media's canvas, the clamp the server would otherwise
 * refuse with a 400, the polygon's corner rules, the keys that move and resize a
 * shape, and the Image API rendition the overlay draws.
 *
 * A point on the canvas is an integer pixel of the upright original, the space
 * the server stores a region in (contract 5). The overlay's `viewBox` is that
 * canvas while the picture under it is whatever rendition the box asked for, so
 * the browser's screen matrix carries the derivative-to-canvas mapping. A
 * rendition smaller than the canvas, as a camera RAW's is, still places a point
 * on the canvas the container reports. That is why every function here takes the
 * matrix rather than a scale.
 */
import type { Selector } from './wire';

export interface CanvasSize {
  readonly width: number;
  readonly height: number;
}

export interface Point {
  readonly x: number;
  readonly y: number;
}

/** The six members of a 2D affine matrix, as `DOMMatrix` reports them. */
export interface Matrix {
  readonly a: number;
  readonly b: number;
  readonly c: number;
  readonly d: number;
  readonly e: number;
  readonly f: number;
}

/** A selector that addresses the picture, in the editor's own shape. */
export type Region =
  | {
      readonly kind: 'rect';
      readonly x: number;
      readonly y: number;
      readonly w: number;
      readonly h: number;
    }
  | { readonly kind: 'polygon'; readonly points: readonly Point[] };

const pairOf = (point: Point): readonly [number, number] => [point.x, point.y];

/** The editor's shape for a note's selector, or `null` when it names no region. */
export const regionOf = (selector: Selector): Region | null => {
  switch (selector.kind) {
    case 'rect':
      return { kind: 'rect', x: selector.x, y: selector.y, w: selector.w, h: selector.h };
    case 'polygon':
      return { kind: 'polygon', points: selector.points.map(([x, y]) => ({ x, y })) };
    default:
      return null;
  }
};

/** The selector a shape is stored as: `xywh=pixel:` or an SVG polygon (contract 6.3). */
export const selectorOf = (region: Region): Selector =>
  region.kind === 'rect'
    ? { kind: 'rect', x: region.x, y: region.y, w: region.w, h: region.h }
    : { kind: 'polygon', points: region.points.map(pairOf) };

/** The zoom steps. Six named sizes rather than free zoom (design rule 8). */
export const ZOOM_STEPS: readonly number[] = [1, 1.5, 2, 3, 4, 8];

export const MIN_ZOOM = ZOOM_STEPS[0] ?? 1;
export const MAX_ZOOM = ZOOM_STEPS[ZOOM_STEPS.length - 1] ?? 8;

/** A pointer that moved less than this many screen pixels is a click, not a drag. */
export const MIN_DRAG_PX = 4;

/** The server's polygon limit (contract 4.4): 3 to 500 corners. */
export const MAX_CORNERS = 500;
export const MIN_CORNERS = 3;

/** From this many corners the editor shows the count, so the limit is no surprise. */
export const CORNER_COUNT_FROM = 450;

/** A click within this many screen pixels of the first corner closes a polygon. */
export const CLOSE_PX = 8;

/**
 * The canvas point under a screen point. `inverse` is `getScreenCTM().inverse()`,
 * which already accounts for the viewBox, the scroll offset and any page zoom.
 */
export const toCanvas = (inverse: Matrix, screen: Point): Point => ({
  x: inverse.a * screen.x + inverse.c * screen.y + inverse.e,
  y: inverse.b * screen.x + inverse.d * screen.y + inverse.f,
});

const whole = (value: number, low: number, high: number): number =>
  Math.min(Math.max(Math.round(value), low), high);

/** A point snapped to a whole canvas pixel and kept inside the canvas. */
export const snap = (point: Point, size: CanvasSize): Point => ({
  x: whole(point.x, 0, size.width),
  y: whole(point.y, 0, size.height),
});

/** The box between two corners, in either drag direction, at least 1 by 1. */
export const boxBetween = (from: Point, to: Point): Region => ({
  kind: 'rect',
  x: Math.min(from.x, to.x),
  y: Math.min(from.y, to.y),
  w: Math.max(1, Math.abs(to.x - from.x)),
  h: Math.max(1, Math.abs(to.y - from.y)),
});

/** What a drag meant: a click to select, or a rectangle to mark. */
export type Drag =
  | { readonly gesture: 'click'; readonly at: Point }
  | { readonly gesture: 'rect'; readonly selector: Region };

export const dragRect = (
  start: Point,
  end: Point,
  inverse: Matrix,
  size: CanvasSize,
): Drag => {
  const at = snap(toCanvas(inverse, end), size);
  if (Math.hypot(end.x - start.x, end.y - start.y) < MIN_DRAG_PX) {
    return { gesture: 'click', at };
  }
  return { gesture: 'rect', selector: boxBetween(snap(toCanvas(inverse, start), size), at) };
};

/** One more corner, or `null` when the shape already holds the server's maximum. */
export const addCorner = (corners: readonly Point[], corner: Point): readonly Point[] | null =>
  corners.length >= MAX_CORNERS ? null : [...corners, corner];

/** The polygon a corner list closes, or `null` while it holds too few. */
export const closePolygon = (corners: readonly Point[]): Region | null =>
  corners.length < MIN_CORNERS ? null : { kind: 'polygon', points: corners };

const near = (a: Point, b: Point, tolerance: number): boolean =>
  Math.hypot(a.x - b.x, a.y - b.y) <= tolerance;

/** Whether a pointer landed on the first corner, so the shape closes. */
export const onFirstCorner = (corners: readonly Point[], point: Point, tolerance: number): boolean => {
  const first = corners[0];
  return first !== undefined && corners.length >= MIN_CORNERS && near(first, point, tolerance);
};

const insidePolygon = (points: readonly Point[], point: Point): boolean => {
  let inside = false;
  for (let index = 0, previous = points.length - 1; index < points.length; previous = index, index += 1) {
    const a = points[index];
    const b = points[previous];
    if (a === undefined || b === undefined) {
      continue;
    }
    const crosses = a.y > point.y !== b.y > point.y;
    if (crosses && point.x < ((b.x - a.x) * (point.y - a.y)) / (b.y - a.y) + a.x) {
      inside = !inside;
    }
  }
  return inside;
};

export const contains = (region: Region, point: Point): boolean =>
  region.kind === 'rect'
    ? point.x >= region.x &&
      point.x <= region.x + region.w &&
      point.y >= region.y &&
      point.y <= region.y + region.h
    : insidePolygon(region.points, point);

/** The index of the shape under a point, the last drawn (topmost) first. */
export const regionAt = (regions: readonly Region[], point: Point): number | null => {
  for (let index = regions.length - 1; index >= 0; index -= 1) {
    const region = regions[index];
    if (region !== undefined && contains(region, point)) {
      return index;
    }
  }
  return null;
};

/**
 * The shape moved by whole pixels. A shape that would cross the canvas edge
 * stops at it, so a keyboard nudge never asks the server for what it refuses.
 */
export const nudge = (region: Region, dx: number, dy: number, size: CanvasSize): Region => {
  if (region.kind === 'rect') {
    return {
      kind: 'rect',
      x: whole(region.x + dx, 0, Math.max(0, size.width - region.w)),
      y: whole(region.y + dy, 0, Math.max(0, size.height - region.h)),
      w: region.w,
      h: region.h,
    };
  }
  if (region.points.length === 0) {
    return region;
  }
  const xs = region.points.map((point) => point.x);
  const ys = region.points.map((point) => point.y);
  const stepX = Math.min(Math.max(dx, -Math.min(...xs)), size.width - Math.max(...xs));
  const stepY = Math.min(Math.max(dy, -Math.min(...ys)), size.height - Math.max(...ys));
  return {
    kind: 'polygon',
    points: region.points.map((point) => ({ x: point.x + stepX, y: point.y + stepY })),
  };
};

/** A rectangle resized from its bottom-right corner, never below 1 by 1 pixel. */
export const resize = (region: Region, dx: number, dy: number, size: CanvasSize): Region =>
  region.kind === 'rect'
    ? {
        kind: 'rect',
        x: region.x,
        y: region.y,
        w: whole(region.w + dx, 1, Math.max(1, size.width - region.x)),
        h: whole(region.h + dy, 1, Math.max(1, size.height - region.y)),
      }
    : region;

/** The box a shape occupies, which is also how a polygon is read aloud. */
export interface Bounds {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export const boundsOf = (region: Region): Bounds => {
  if (region.kind === 'rect') {
    return { x: region.x, y: region.y, width: region.w, height: region.h };
  }
  if (region.points.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }
  const xs = region.points.map((point) => point.x);
  const ys = region.points.map((point) => point.y);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y };
};

/**
 * The rendition for a picture shown `shownWidth` CSS pixels wide at `dpr`, which
 * is the box times the zoom. Never larger than the canvas: the Image API does
 * not upscale, and a size past the region is a 400.
 */
export const renditionUrl = (
  service: string,
  size: CanvasSize,
  shownWidth: number,
  dpr: number,
): string => {
  const wanted = Math.round(shownWidth * dpr);
  const width = wanted > 0 ? Math.min(size.width, Math.max(1, wanted)) : size.width;
  const height = Math.min(size.height, Math.max(1, Math.round((width * size.height) / size.width)));
  return `${service}/full/!${String(width)},${String(height)}/0/default.jpg`;
};

/** The zoom one step further out or in, stopping at the ends of the ladder. */
export const nextZoom = (zoom: number, direction: -1 | 1): number => {
  const index = ZOOM_STEPS.indexOf(zoom);
  const next = ZOOM_STEPS[Math.min(Math.max(index + direction, 0), ZOOM_STEPS.length - 1)];
  return next === undefined ? zoom : next;
};

/** The four numbers of "Add a region by numbers", as the input fields carry them. */
export interface BoxFields {
  readonly x: string;
  readonly y: string;
  readonly width: string;
  readonly height: string;
}

const wholeNumber = (text: string): number | null =>
  /^\d+$/.test(text.trim()) ? Number.parseInt(text, 10) : null;

/** The region a typed box names, or `null` when it cannot be stored. */
export const boxFromText = (fields: BoxFields, size: CanvasSize): Region | null => {
  const x = wholeNumber(fields.x);
  const y = wholeNumber(fields.y);
  const w = wholeNumber(fields.width);
  const h = wholeNumber(fields.height);
  if (x === null || y === null || w === null || h === null) {
    return null;
  }
  if (w < 1 || h < 1 || x + w > size.width || y + h > size.height) {
    return null;
  }
  return { kind: 'rect', x, y, w, h };
};
