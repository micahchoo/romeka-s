/**
 * A preview frame at a device's real width, scaled to fit the box it sits in.
 *
 * The page inside the frame lays out at `device` CSS pixels, so its media
 * queries and line lengths are the device's. A transform then shrinks the frame
 * to the box, never enlarges it, and the layout height grows by the same factor
 * so the scaled frame still fills the box's height. `fit` lays the page out at
 * the box's own width, but never narrower than [`FIT_MIN_WIDTH`]: an editor
 * panel is phone-narrow, and a page laid out there shows its phone header.
 */
import type { CSSProperties, RefObject } from 'react';
import { useLayoutEffect, useState } from 'react';
import type { KeyOf } from '../../../i18n';
import type { sites } from '../../../i18n/namespaces/sites';

/** A device width in CSS pixels, or `fit`: the width of the box itself. */
export type DeviceWidth = number | 'fit';

/** The narrowest layout Fit gives the page: a small desktop, past the public theme's widest breakpoint (60rem). */
export const FIT_MIN_WIDTH = 1024;

/** The device choices every preview offers. Fit is first and the default. */
export const DEVICES: readonly { readonly key: KeyOf<typeof sites>; readonly width: DeviceWidth }[] = [
  { key: 'editor.widthFit', width: 'fit' },
  { key: 'editor.widthDesktop', width: 1280 },
  { key: 'editor.widthTablet', width: 768 },
  { key: 'editor.widthMobile', width: 390 },
];

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
  /** The offset that centres a frame narrower than its box. */
  readonly left: number;
}

export const fitFrame = (device: DeviceWidth, box: Box): FrameFit => {
  const width = device === 'fit' ? Math.max(box.width, FIT_MIN_WIDTH) : device;
  const scale = box.width > 0 && width > 0 ? Math.min(1, box.width / width) : 1;
  return {
    width,
    height: box.height / scale,
    scale,
    left: Math.max(0, (box.width - width * scale) / 2),
  };
};

/** The style that applies a fit to a frame positioned at its box's top left. */
export const frameStyle = (fit: FrameFit): CSSProperties => ({
  left: `${String(fit.left)}px`,
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
