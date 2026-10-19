/**
 * A preview frame at a device's real width, scaled to fit the box it sits in.
 *
 * The page inside the frame lays out at `device` CSS pixels, so its media
 * queries and line lengths are the device's. A transform then shrinks the frame
 * to the box, never enlarges it, and the layout height grows by the same factor
 * so the scaled frame still fills the box's height.
 */
import type { CSSProperties, RefObject } from 'react';
import { useLayoutEffect, useState } from 'react';

export interface Box {
  readonly width: number;
  readonly height: number;
}

export interface FrameFit {
  /** The frame's layout width: the device width. */
  readonly width: number;
  /** The frame's layout height, before the scale. */
  readonly height: number;
  readonly scale: number;
}

export const fitFrame = (device: number, box: Box): FrameFit => {
  const scale = box.width > 0 ? Math.min(1, box.width / device) : 1;
  return { width: device, height: box.height / scale, scale };
};

/** The style that applies a fit to a frame positioned at its box's top left. */
export const frameStyle = (fit: FrameFit): CSSProperties => ({
  width: `${String(fit.width)}px`,
  height: `${String(fit.height)}px`,
  transform: `scale(${String(fit.scale)})`,
  transformOrigin: '0 0',
});

/** The content box of an element, kept current as the element resizes. */
export const useBoxSize = (ref: RefObject<HTMLElement | null>): Box => {
  const [box, setBox] = useState<Box>({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const node = ref.current;
    if (node === null) {
      return;
    }
    const read = (): void => {
      setBox({ width: node.clientWidth, height: node.clientHeight });
    };
    read();
    const observer = new ResizeObserver(read);
    observer.observe(node);
    return () => {
      observer.disconnect();
    };
  }, [ref]);
  return box;
};
