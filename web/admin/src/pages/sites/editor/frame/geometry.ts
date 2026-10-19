/**
 * Boxes in overlay coordinates, and the insert slots the overlay draws.
 *
 * The overlay is a sibling of the frame inside the same positioned box, and the
 * frame is scaled from its top left corner (`../frameFit.ts`), so a rect read
 * inside the frame maps to the overlay by the frame's offset and scale.
 */
import type { BlockKey, PageDraft } from '../draft';
import { parseNodes } from '../groups';
import type { DraftNode } from '../groups';

export interface Box {
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

/** Where an insert point adds a block: the same two cases the reducer's `addBlock` takes. */
export type InsertTarget =
  | { readonly kind: 'top'; readonly index: number }
  /** Into the group, with `index` as the flat fallback when the block may not join. */
  | { readonly kind: 'group'; readonly group: BlockKey; readonly index: number };

export interface InsertSlot {
  readonly target: InsertTarget;
  /** A line in a normal page (zero height), a corner point in a grid page (zero size). */
  readonly box: Box;
  readonly shape: 'line' | 'chip';
}

/** The frame's scale: its drawn width over its layout width. */
const scaleOf = (frame: HTMLIFrameElement): number => {
  const drawn = frame.getBoundingClientRect().width;
  return frame.offsetWidth > 0 && drawn > 0 ? drawn / frame.offsetWidth : 1;
};

/** An element's box in overlay coordinates: its rect in the frame, scaled, plus the frame's offset. */
export const boxOf = (element: Element, frame: HTMLIFrameElement): Box => {
  const rect = element.getBoundingClientRect();
  const scale = scaleOf(frame);
  return {
    top: frame.offsetTop + rect.top * scale,
    left: frame.offsetLeft + rect.left * scale,
    width: rect.width * scale,
    height: rect.height * scale,
  };
};

const keyOf = (node: DraftNode): BlockKey =>
  node.kind === 'block' ? node.block.key : node.marker.key;

const point = (top: number, left: number): Box => ({ top, left, width: 0, height: 0 });

/**
 * The insert slots for the draft as rendered: one before each top-level node,
 * one after the last, and one at the bottom edge inside each group. The
 * indices are the Outline's flat indices. A node with no element (unparsed,
 * or newer than the render) gives no slot of its own.
 */
export const insertSlots = (
  draft: PageDraft,
  map: { readonly elements: ReadonlyMap<BlockKey, Element> },
  frame: HTMLIFrameElement,
): readonly InsertSlot[] => {
  const grid = draft.layout === 'grid';
  const slots: InsertSlot[] = [];
  const placed: Box[] = [];
  let index = 0;
  let previous: Box | null = null;
  for (const node of parseNodes(draft.blocks)) {
    const element = map.elements.get(keyOf(node));
    const size = node.kind === 'block' ? 1 : 1 + node.members.length;
    if (element !== undefined) {
      const box = boxOf(element, frame);
      placed.push(box);
      slots.push(
        grid
          ? { target: { kind: 'top', index }, box: point(box.top, box.left), shape: 'chip' }
          : {
              target: { kind: 'top', index },
              box: {
                top: previous === null ? box.top : (previous.top + previous.height + box.top) / 2,
                left: box.left,
                width: box.width,
                height: 0,
              },
              shape: 'line',
            },
      );
      if (node.kind === 'group') {
        slots.push({
          target: { kind: 'group', group: node.marker.key, index: index + size },
          box: { top: box.top + box.height, left: box.left, width: box.width, height: 0 },
          shape: 'line',
        });
      }
      previous = box;
    }
    index += size;
  }
  if (placed.length > 0) {
    const bottom = Math.max(...placed.map((box) => box.top + box.height));
    const left = Math.min(...placed.map((box) => box.left));
    const right = Math.max(...placed.map((box) => box.left + box.width));
    const end: InsertTarget = { kind: 'top', index: draft.blocks.length };
    slots.push(
      grid
        ? { target: end, box: point(bottom, left), shape: 'chip' }
        : { target: end, box: { top: bottom, left, width: right - left, height: 0 }, shape: 'line' },
    );
  }
  return slots;
};
