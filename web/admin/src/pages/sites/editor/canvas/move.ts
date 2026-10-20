/**
 * The draft actions a canvas gesture becomes. Every helper returns a
 * `DraftAction` or `null` for a gesture that changes nothing, so the canvas
 * never edits a block itself and an out-of-range key never reaches the reducer.
 *
 * A flat index is a block's position in `PageDraft.blocks`, which the model
 * keeps canonical (`groups.ts` renders it after every structural edit).
 */
import type { BlockDraft, BlockKey, DraftAction } from '../draft';
import { parseNodes } from '../groups';
import { placementOf } from './grid';

/** The block's position in the flat list, or `-1` when it is absent. */
export const flatIndex = (blocks: readonly BlockDraft[], key: BlockKey): number =>
  blocks.findIndex((block) => block.key === key);

/** The group a block is a member of, or `null` for a top-level block. */
export const groupKeyOf = (blocks: readonly BlockDraft[], key: BlockKey): BlockKey | null => {
  for (const node of parseNodes(blocks)) {
    if (node.kind === 'group' && node.members.some((member) => member.key === key)) {
      return node.marker.key;
    }
  }
  return null;
};

/** The group directly above a top-level block, or `null` when there is none. */
export const groupAbove = (blocks: readonly BlockDraft[], key: BlockKey): BlockKey | null => {
  const nodes = parseNodes(blocks);
  for (const [index, node] of nodes.entries()) {
    if (node.kind !== 'block' || node.block.key !== key) {
      continue;
    }
    const above = nodes[index - 1];
    return above !== undefined && above.kind === 'group' ? above.marker.key : null;
  }
  return null;
};

/** One position up or down. The target index is read against the list without the block. */
export const moveByOne = (
  blocks: readonly BlockDraft[],
  key: BlockKey,
  delta: -1 | 1,
): DraftAction | null => {
  const index = flatIndex(blocks, key);
  const target = index + delta;
  if (index < 0 || target < 0 || target >= blocks.length) {
    return null;
  }
  return { type: 'moveBlock', blockKey: key, toIndex: target };
};

/** Puts a block into a group, before the member `before` (its slot) or at the end. */
export const intoGroup = (
  blocks: readonly BlockDraft[],
  key: BlockKey,
  group: BlockKey,
  before: BlockKey | null = null,
): DraftAction => ({
  type: 'moveBlock',
  blockKey: key,
  toIndex: Math.max(0, flatIndex(blocks, key)),
  into: group,
  ...(before === null ? {} : { before }),
});

/** Forces a block out of its group, keeping its place in the flow. */
export const outOfGroup = (blocks: readonly BlockDraft[], key: BlockKey): DraftAction => ({
  type: 'moveBlock',
  blockKey: key,
  toIndex: flatIndex(blocks, key) + 1,
  into: null,
});

/** A span change and the span it produces, for the announcement. */
export interface SpanChange {
  readonly action: DraftAction;
  readonly span: number;
}

export const resizeSpan = (
  block: BlockDraft,
  columns: number,
  delta: -1 | 1,
): SpanChange | null => {
  const current = placementOf(block, columns).span;
  const next = Math.min(columns, Math.max(1, current + delta));
  if (next === current) {
    return null;
  }
  return {
    action: {
      type: 'setLayoutData',
      blockKey: block.key,
      key: 'grid_column_span',
      value: String(next),
    },
    span: next,
  };
};

/** A position change and the column it produces, for the announcement. */
export interface PositionChange {
  readonly action: DraftAction;
  readonly position: number;
}

export const movePosition = (
  block: BlockDraft,
  columns: number,
  delta: -1 | 1,
): PositionChange | null => {
  const placement = placementOf(block, columns);
  const current = placement.position === 'auto' ? 1 : placement.position;
  const next = Math.min(columns, Math.max(1, current + delta));
  if (placement.position !== 'auto' && next === placement.position) {
    return null;
  }
  return {
    action: {
      type: 'setLayoutData',
      blockKey: block.key,
      key: 'grid_column_position',
      value: String(next),
    },
    position: next,
  };
};
