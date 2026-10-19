/**
 * The parent's listeners on one frame document. The frame runs no script
 * (`sandbox="allow-same-origin"`), so these capture-phase listeners are the
 * only code that reacts to the pointer inside it. The parent never writes to
 * the document: no style, no node, no attribute.
 */

export interface FrameHandlers {
  /** The pointer moved over a node, or left the document (`null`). Hover only. */
  readonly onPoint: (node: Node | null) => void;
  /** A click landed on a node. */
  readonly onPick: (node: Node) => void;
}

/**
 * `click` and `auxclick` always `preventDefault()`, so no link navigates the
 * frame; `submit` too, and the sandbox has no `allow-forms` besides. Returns
 * the detach function.
 */
export const guardFrame = (doc: Document, handlers: FrameHandlers): (() => void) => {
  const NodeType = doc.defaultView?.Node ?? Node;
  const nodeOf = (event: Event): Node | null =>
    event.target instanceof NodeType ? event.target : null;

  const onMove = (event: Event): void => {
    handlers.onPoint(nodeOf(event));
  };
  const onLeave = (event: Event): void => {
    if (event.target === doc.documentElement) {
      handlers.onPoint(null);
    }
  };
  const onClick = (event: Event): void => {
    event.preventDefault();
    const node = nodeOf(event);
    if (node !== null) {
      handlers.onPick(node);
    }
  };
  const prevent = (event: Event): void => {
    event.preventDefault();
  };

  const listeners: readonly (readonly [string, (event: Event) => void])[] = [
    ['pointermove', onMove],
    ['pointerleave', onLeave],
    ['click', onClick],
    ['auxclick', prevent],
    ['submit', prevent],
  ];
  for (const [type, listener] of listeners) {
    doc.addEventListener(type, listener, true);
  }
  return () => {
    for (const [type, listener] of listeners) {
      doc.removeEventListener(type, listener, true);
    }
  };
};
