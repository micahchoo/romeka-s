/**
 * The frame document parsed once, at load: the only module that reads
 * `data-romeka-block` and `data-romeka-region`. Everything after it speaks
 * `BlockKey`, `SiteRegion` and `Subject`.
 *
 * The server stamps `data-romeka-block="n"` on each block and group wrapper of
 * a preview render, where `n` is the block's one-based position in the request
 * body (`crates/omeka-server/src/site/dispatch.rs::render_preview`), and
 * `data-romeka-region` on the banner, the header and the footer.
 */
import type { BlockKey } from '../draft';
import { parseRegion } from '../regions';
import type { SiteRegion } from '../regions';
import type { Subject } from '../shell/selection';

const ATTRIBUTE = 'data-romeka-block';
const SELECTOR = `[${ATTRIBUTE}]`;
const REGION_ATTRIBUTE = 'data-romeka-region';
const REGION_SELECTOR = `[${REGION_ATTRIBUTE}]`;
const ANY_MARKER = `${SELECTOR}, ${REGION_SELECTOR}`;

export interface FrameMap {
  /** The element of each real block or group, by key, in document order. */
  readonly elements: ReadonlyMap<BlockKey, Element>;
  /** The elements of each site region, in document order: the header region is the banner and the header. */
  readonly regions: ReadonlyMap<SiteRegion, readonly Element[]>;
  /** What a DOM node belongs to: the innermost real block above it, else the region it sits in. */
  targetAt(node: Node): Subject | null;
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
  const regionOf = readRegions(doc);
  const regions = new Map<SiteRegion, Element[]>();
  for (const [element, region] of regionOf) {
    regions.set(region, [...(regions.get(region) ?? []), element]);
  }
  return {
    elements,
    regions,
    targetAt: (node) => {
      let marked = elementOf(node)?.closest(SELECTOR) ?? null;
      while (marked !== null) {
        const key = keyOf.get(marked);
        if (key !== undefined) {
          return { kind: 'block', key };
        }
        marked = marked.parentElement?.closest(SELECTOR) ?? null;
      }
      let framed = elementOf(node)?.closest(REGION_SELECTOR) ?? null;
      while (framed !== null) {
        const region = regionOf.get(framed);
        if (region !== undefined) {
          return { kind: 'region', region };
        }
        framed = framed.parentElement?.closest(REGION_SELECTOR) ?? null;
      }
      return null;
    },
  };
};

/**
 * A region marker is real when no other marker sits above it. A
 * `data-romeka-region` inside a block's or a footer column's stored HTML
 * always has one, so it is ignored.
 */
const readRegions = (doc: Document): ReadonlyMap<Element, SiteRegion> => {
  const regionOf = new Map<Element, SiteRegion>();
  for (const element of doc.querySelectorAll(REGION_SELECTOR)) {
    const region = parseRegion(element.getAttribute(REGION_ATTRIBUTE));
    if (region !== null && (element.parentElement?.closest(ANY_MARKER) ?? null) === null) {
      regionOf.set(element, region);
    }
  }
  return regionOf;
};
