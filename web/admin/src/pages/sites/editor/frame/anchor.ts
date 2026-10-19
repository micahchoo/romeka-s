/**
 * A scroll position that survives a frame swap. It is named by a block, not by
 * pixels, so content that grows above the viewport does not move the reader.
 */
import type { BlockKey } from '../draft';
import type { FrameMap } from './markers';

export interface ScrollAnchor {
  /** `null`: no block in view, so the restore falls back to `scrollY`. */
  readonly key: BlockKey | null;
  /** How far the block's top is from the viewport top. */
  readonly offset: number;
  readonly scrollY: number;
}

/**
 * The selected block if it is in view, else the innermost block whose box
 * crosses the viewport top, else the first block below it.
 */
export const takeAnchor = (
  win: Window,
  map: Pick<FrameMap, 'elements'>,
  selected: BlockKey | null,
): ScrollAnchor => {
  const scrollY = win.scrollY;
  const chosen = (key: BlockKey, element: Element): ScrollAnchor => ({
    key,
    offset: element.getBoundingClientRect().top,
    scrollY,
  });
  const selectedElement = selected === null ? undefined : map.elements.get(selected);
  if (selected !== null && selectedElement !== undefined) {
    const rect = selectedElement.getBoundingClientRect();
    if (rect.bottom > 0 && rect.top < win.innerHeight) {
      return chosen(selected, selectedElement);
    }
  }
  let crossing: ScrollAnchor | null = null;
  let below: ScrollAnchor | null = null;
  for (const [key, element] of map.elements) {
    const rect = element.getBoundingClientRect();
    if (rect.top <= 0 && rect.bottom > 0) {
      crossing = chosen(key, element);
    } else if (below === null && rect.top > 0) {
      below = chosen(key, element);
    }
  }
  return crossing ?? below ?? { key: null, offset: 0, scrollY };
};

export const restoreAnchor = (
  win: Window,
  map: Pick<FrameMap, 'elements'>,
  anchor: ScrollAnchor,
): void => {
  const element = anchor.key === null ? undefined : map.elements.get(anchor.key);
  const top =
    element === undefined
      ? anchor.scrollY
      : win.scrollY + element.getBoundingClientRect().top - anchor.offset;
  win.scrollTo(0, Math.max(0, top));
};
