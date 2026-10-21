/**
 * The region editor through its public interface: pointer pixels to the media's
 * own canvas pixels, the clamp the server would otherwise refuse, the polygon
 * rules, the nudge and resize keys, and the polygon's keyboard path.
 *
 * The matrices are the ones a browser reports for a 4000 by 3000 canvas shown
 * in a 500 px box whose left edge is at 100 px: at 1× one screen pixel covers
 * 8 canvas pixels, so the inverse matrix scales by 8; at 4× the figure is
 * 2000 px wide and one screen pixel covers 2.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import {
  addCorner,
  boxFromText,
  closePolygon,
  dragRect,
  nextZoom,
  nudge,
  regionAt,
  renditionUrl,
  resize,
  snap,
  toCanvas,
} from '../src/pages/media/annotations/draw';
import type { CanvasSize, Matrix, Region } from '../src/pages/media/annotations/draw';
import type { Selector } from '../src/pages/media/annotations/wire';
import { RegionEditor } from '../src/pages/media/annotations/region-editor';
import type { Surface } from '../src/pages/media/annotations/wire';

const CANVAS: CanvasSize = { width: 4000, height: 3000 };
const AT_ONE: Matrix = { a: 8, b: 0, c: 0, d: 8, e: -800, f: -400 };
const AT_FOUR: Matrix = { a: 2, b: 0, c: 0, d: 2, e: -200, f: -100 };

/** `count` corners on the diagonal, which is enough to count them. */
const corners = (count: number): readonly { readonly x: number; readonly y: number }[] =>
  Array.from({ length: count }, (_, index) => ({ x: index, y: index }));

describe('a pointer becomes a canvas pixel', () => {
  it('maps the same pointer to a quarter of the canvas at 4×', () => {
    expect(snap(toCanvas(AT_ONE, { x: 350, y: 250 }), CANVAS)).toEqual({ x: 2000, y: 1600 });
    expect(snap(toCanvas(AT_FOUR, { x: 350, y: 250 }), CANVAS)).toEqual({ x: 500, y: 400 });
  });

  it('clamps a pointer outside the picture to its edges', () => {
    expect(snap(toCanvas(AT_ONE, { x: -1000, y: -1000 }), CANVAS)).toEqual({ x: 0, y: 0 });
    expect(snap(toCanvas(AT_ONE, { x: 10000, y: 10000 }), CANVAS)).toEqual({ x: 4000, y: 3000 });
  });
});

describe('a drag marks a rectangle', () => {
  it('is a click under four screen pixels', () => {
    expect(dragRect({ x: 100, y: 100 }, { x: 103, y: 101 }, AT_ONE, CANVAS).gesture).toBe('click');
  });

  it('is a rectangle past four screen pixels, from either corner', () => {
    const forward = dragRect({ x: 300, y: 200 }, { x: 500, y: 400 }, AT_ONE, CANVAS);
    const backward = dragRect({ x: 500, y: 400 }, { x: 300, y: 200 }, AT_ONE, CANVAS);
    expect(forward).toEqual({
      gesture: 'rect',
      selector: { kind: 'rect', x: 1600, y: 1200, w: 1600, h: 1600 },
    });
    expect(backward).toEqual(forward);
  });

  it('never leaves the canvas', () => {
    expect(dragRect({ x: -50, y: -50 }, { x: 9999, y: 9999 }, AT_ONE, CANVAS)).toEqual({
      gesture: 'rect',
      selector: { kind: 'rect', x: 0, y: 0, w: 4000, h: 3000 },
    });
  });
});

describe('a polygon', () => {
  it('takes five hundred corners and refuses the next', () => {
    expect(addCorner(corners(499), { x: 900, y: 900 })).toHaveLength(500);
    expect(addCorner(corners(500), { x: 900, y: 900 })).toBeNull();
  });

  it('closes only with three corners', () => {
    expect(closePolygon(corners(2))).toBeNull();
    expect(closePolygon(corners(3))).toEqual({ kind: 'polygon', points: corners(3) });
  });
});

describe('a selected shape', () => {
  const rect: Region = { kind: 'rect', x: 0, y: 10, w: 20, h: 20 };

  it('moves one pixel and stops at the edge', () => {
    expect(nudge(rect, -1, 1, CANVAS)).toEqual({ kind: 'rect', x: 0, y: 11, w: 20, h: 20 });
    expect(nudge(rect, 4000, 0, CANVAS)).toEqual({ kind: 'rect', x: 3980, y: 10, w: 20, h: 20 });
  });

  it('resizes from its bottom-right corner, inside the canvas', () => {
    expect(resize(rect, 5, -5, CANVAS)).toEqual({ kind: 'rect', x: 0, y: 10, w: 25, h: 15 });
    expect(resize(rect, 5000, 5000, CANVAS)).toEqual({ kind: 'rect', x: 0, y: 10, w: 4000, h: 2990 });
    expect(resize(rect, -50, -50, CANVAS)).toEqual({ kind: 'rect', x: 0, y: 10, w: 1, h: 1 });
  });

  it('is found under a click, the topmost shape first', () => {
    const regions: readonly Region[] = [
      { kind: 'rect', x: 10, y: 10, w: 20, h: 20 },
      { kind: 'polygon', points: [{ x: 100, y: 100 }, { x: 200, y: 100 }, { x: 150, y: 200 }] },
    ];
    expect(regionAt(regions, { x: 20, y: 20 })).toBe(0);
    expect(regionAt(regions, { x: 150, y: 150 })).toBe(1);
    expect(regionAt(regions, { x: 300, y: 300 })).toBeNull();
  });
});

describe('the picture the overlay draws', () => {
  const service = 'https://archive.test/iiif/image/3/4127';

  it('asks for the screen’s pixels and never more than the canvas', () => {
    expect(renditionUrl(service, CANVAS, 500, 1)).toBe(`${service}/full/!500,375/0/default.jpg`);
    expect(renditionUrl(service, CANVAS, 500, 2)).toBe(`${service}/full/!1000,750/0/default.jpg`);
    expect(renditionUrl(service, CANVAS, 2000, 2)).toBe(`${service}/full/!4000,3000/0/default.jpg`);
  });

  it('asks for the whole canvas before the box is measured', () => {
    expect(renditionUrl(service, CANVAS, 0, 1)).toBe(`${service}/full/!4000,3000/0/default.jpg`);
  });
});

describe('a region typed as numbers', () => {
  it('takes whole pixels inside the picture', () => {
    expect(boxFromText({ x: '5', y: '6', width: '10', height: '20' }, CANVAS)).toEqual({
      kind: 'rect',
      x: 5,
      y: 6,
      w: 10,
      h: 20,
    });
  });

  it('refuses a region that does not fit, is empty, or is not whole', () => {
    expect(boxFromText({ x: '70', y: '0', width: '20', height: '20' }, { width: 80, height: 120 })).toBeNull();
    expect(boxFromText({ x: '0', y: '0', width: '0', height: '10' }, CANVAS)).toBeNull();
    expect(boxFromText({ x: '1.5', y: '0', width: '10', height: '10' }, CANVAS)).toBeNull();
    expect(boxFromText({ x: '', y: '0', width: '10', height: '10' }, CANVAS)).toBeNull();
  });
});

describe('the zoom', () => {
  it('steps through six sizes and stops at each end', () => {
    expect(nextZoom(1, 1)).toBe(1.5);
    expect(nextZoom(4, 1)).toBe(8);
    expect(nextZoom(8, 1)).toBe(8);
    expect(nextZoom(1, -1)).toBe(1);
    expect(nextZoom(2, -1)).toBe(1.5);
  });
});

/** The upright canvas of `fixtures/media-kinds/image.jpg`: 120 by 80, turned. */
const SURFACE: Extract<Surface, { kind: 'image' }> = {
  kind: 'image',
  width: 80,
  height: 120,
  service: '/iiif/image/3/9',
};

describe('the polygon from the keyboard', () => {
  it('adds corners with Space, moves the cursor with the arrows, and closes on Enter', () => {
    const drawn: Selector[] = [];
    const { container } = render(
      <RegionEditor
        surface={SURFACE}
        rows={[]}
        selected={null}
        draft={null}
        writable
        onDraw={(selector) => {
          drawn.push(selector);
        }}
        onSelect={() => undefined}
        onMove={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Advanced' }));
    fireEvent.click(screen.getByRole('button', { name: 'Polygon' }));

    const overlay = container.querySelector('.region-overlay');
    if (!(overlay instanceof SVGSVGElement)) {
      throw new Error('the overlay is not an SVG element');
    }
    // The corner cursor starts at the centre of the canvas, 40 by 60.
    fireEvent.keyDown(overlay, { key: ' ' });
    fireEvent.keyDown(overlay, { key: 'ArrowRight', shiftKey: true });
    fireEvent.keyDown(overlay, { key: ' ' });
    fireEvent.keyDown(overlay, { key: 'ArrowDown' });
    fireEvent.keyDown(overlay, { key: ' ' });
    fireEvent.keyDown(overlay, { key: 'Enter' });

    expect(drawn).toEqual([
      {
        kind: 'polygon',
        points: [
          [40, 60],
          [50, 60],
          [50, 61],
        ],
      },
    ]);
    expect(screen.getByRole('button', { name: 'Move' })).toHaveAttribute('aria-pressed', 'true');
  });
});
