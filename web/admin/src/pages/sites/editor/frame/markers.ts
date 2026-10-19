/**
 * The frame document parsed once, at load: the only module that reads
 * `data-romeka-block`. Everything after it speaks `BlockKey`.
 *
 * The server stamps `data-romeka-block="n"` on each block and group wrapper of
 * a preview render, where `n` is the block's one-based position in the request
 * body (`crates/omeka-server/src/site/dispatch.rs::render_preview`).
 */
import type { BlockKey } from '../draft';

const ATTRIBUTE = 'data-romeka-block';
const SELECTOR = `[${ATTRIBUTE}]`;

export interface FrameMap {
  /** The element of each real block or group, by key, in document order. */
  readonly elements: ReadonlyMap<BlockKey, Element>;
  /** The block a DOM node belongs to: the innermost real marker above it. */
  blockAt(node: Node): BlockKey | null;
}

/** The node itself when it is an element, else its parent. Realm-safe, so a
 * node from the frame's own window qualifies. */
export const elementOf = (node: Node): Element | null => {
  const view = node.ownerDocument?.defaultView;
  const ElementType = view?.Element ?? Element;
  return node instanceof ElementType ? node : node.parentElement;
};

/**
 * A marker is real when its `n` names a key in `order` and its nearest marked
 * ancestor is absent or a real group. A `data-romeka-block` inside a block's
 * own stored HTML always has a non-group block above it, so it is ignored.
 */
export const readFrame = (
  doc: Document,
  order: readonly BlockKey[],
  groups: ReadonlySet<BlockKey>,
): FrameMap => {
  const keyOf = new Map<Element, BlockKey>();
  const elements = new Map<BlockKey, Element>();
  for (const element of doc.querySelectorAll(SELECTOR)) {
    const n = Number(element.getAttribute(ATTRIBUTE));
    const key = Number.isInteger(n) && n > 0 ? order[n - 1] : undefined;
    if (key === undefined || elements.has(key)) {
      continue;
    }
    const above = element.parentElement?.closest(SELECTOR) ?? null;
    if (above !== null) {
      const aboveKey = keyOf.get(above);
      if (aboveKey === undefined || !groups.has(aboveKey)) {
        continue;
      }
    }
    keyOf.set(element, key);
    elements.set(key, element);
  }
  return {
    elements,
    blockAt: (node) => {
      let marked = elementOf(node)?.closest(SELECTOR) ?? null;
      while (marked !== null) {
        const key = keyOf.get(marked);
        if (key !== undefined) {
          return key;
        }
        marked = marked.parentElement?.closest(SELECTOR) ?? null;
      }
      return null;
    },
  };
};
