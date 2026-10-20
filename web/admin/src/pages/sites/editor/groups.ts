/**
 * The flat block list and the block groups inside it. A group is a marker plus
 * the blocks it owns; the wire stores the count as `data.span`, so every
 * structural edit goes through here to keep that count equal to the members.
 */
import type { BlockDraft, BlockKey } from './draft';
import { dataObject } from './values';

/** The layout upstream uses for a block group. */
export const GROUP_LAYOUT = 'blockGroup';

export const isGroup = (block: BlockDraft): boolean => block.layout === GROUP_LAYOUT;

export const spanOf = (block: BlockDraft): number => {
  const value = dataObject(block).span;
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(0, Math.trunc(value));
  }
  if (typeof value === 'string') {
    const parsed = Number.parseInt(value, 10);
    return Number.isNaN(parsed) ? 0 : Math.max(0, parsed);
  }
  return 0;
};

const withSpan = (block: BlockDraft, span: number): BlockDraft => ({
  ...block,
  data: { ...dataObject(block), span },
});

export interface BlockNode {
  readonly kind: 'block';
  readonly block: BlockDraft;
}

export interface GroupNode {
  readonly kind: 'group';
  readonly marker: BlockDraft;
  readonly members: readonly BlockDraft[];
}

export type DraftNode = BlockNode | GroupNode;

const nodeSize = (node: DraftNode): number =>
  node.kind === 'block' ? 1 : 1 + node.members.length;

export const flatLength = (nodes: readonly DraftNode[]): number =>
  nodes.reduce((total, node) => total + nodeSize(node), 0);

/**
 * The flat list as groups with their members. A span larger than the blocks
 * before the next marker is clamped, which is what the renderer does.
 */
export const parseNodes = (blocks: readonly BlockDraft[]): readonly DraftNode[] => {
  const nodes: DraftNode[] = [];
  let index = 0;
  while (index < blocks.length) {
    const block = blocks[index];
    if (block === undefined) {
      break;
    }
    if (!isGroup(block)) {
      nodes.push({ kind: 'block', block });
      index += 1;
      continue;
    }
    const span = spanOf(block);
    const members: BlockDraft[] = [];
    let cursor = index + 1;
    while (cursor < blocks.length && members.length < span) {
      const candidate = blocks[cursor];
      if (candidate === undefined || isGroup(candidate)) {
        break;
      }
      members.push(candidate);
      cursor += 1;
    }
    nodes.push({ kind: 'group', marker: block, members });
    index = cursor;
  }
  return nodes;
};

/** The group node whose marker is `key`, `null` for any other block. */
export const groupNodeOf = (blocks: readonly BlockDraft[], key: BlockKey): GroupNode | null => {
  for (const node of parseNodes(blocks)) {
    if (node.kind === 'group' && node.marker.key === key) {
      return node;
    }
  }
  return null;
};

/** The flat list again, with every group's span equal to the members it owns. */
export const renderNodes = (nodes: readonly DraftNode[]): readonly BlockDraft[] =>
  nodes.flatMap((node) =>
    node.kind === 'block'
      ? [isGroup(node.block) ? withSpan(node.block, 0) : node.block]
      : [withSpan(node.marker, node.members.length), ...node.members],
  );

export const canonicalBlocks = (blocks: readonly BlockDraft[]): readonly BlockDraft[] =>
  renderNodes(parseNodes(blocks));

interface InsertionPoint {
  readonly nodeIndex: number;
  /** The group a plain block joins, or `null` for a top-level insertion. */
  readonly group: GroupNode | null;
  readonly memberIndex: number;
}

/**
 * Where a flat index lands. Strictly inside a group joins before that member;
 * the boundary after a group is top-level.
 */
const findInsertion = (nodes: readonly DraftNode[], index: number): InsertionPoint => {
  let offset = 0;
  for (let nodeIndex = 0; nodeIndex < nodes.length; nodeIndex += 1) {
    const node = nodes[nodeIndex];
    if (node === undefined) {
      break;
    }
    const size = nodeSize(node);
    if (index <= offset) {
      return { nodeIndex, group: null, memberIndex: 0 };
    }
    if (index < offset + size) {
      return node.kind === 'group'
        ? { nodeIndex, group: node, memberIndex: index - offset - 1 }
        : { nodeIndex, group: null, memberIndex: 0 };
    }
    offset += size;
  }
  return { nodeIndex: nodes.length, group: null, memberIndex: 0 };
};

/**
 * Inserts `incoming` at `index`. Only leading plain blocks join a group; a
 * group marker in the sequence lands after the group, because groups do not
 * nest.
 */
const spliceNodes = (
  nodes: readonly DraftNode[],
  point: InsertionPoint,
  incoming: readonly DraftNode[],
): readonly DraftNode[] => {
  if (point.group === null) {
    return [...nodes.slice(0, point.nodeIndex), ...incoming, ...nodes.slice(point.nodeIndex)];
  }
  const leading: BlockDraft[] = [];
  let cursor = 0;
  while (cursor < incoming.length) {
    const node = incoming[cursor];
    if (node === undefined || node.kind !== 'block' || isGroup(node.block)) {
      break;
    }
    leading.push(node.block);
    cursor += 1;
  }
  const group = point.group;
  const members = [
    ...group.members.slice(0, point.memberIndex),
    ...leading,
    ...group.members.slice(point.memberIndex),
  ];
  const updated = nodes.map((node, nodeIndex) =>
    nodeIndex === point.nodeIndex ? { kind: 'group' as const, marker: group.marker, members } : node,
  );
  return [
    ...updated.slice(0, point.nodeIndex + 1),
    ...incoming.slice(cursor),
    ...updated.slice(point.nodeIndex + 1),
  ];
};

export const insertAsNodes = (
  nodes: readonly DraftNode[],
  index: number,
  incoming: readonly DraftNode[],
): readonly BlockDraft[] => {
  const clamped = Math.max(0, Math.min(index, flatLength(nodes)));
  return renderNodes(spliceNodes(nodes, findInsertion(nodes, clamped), incoming));
};

/**
 * Puts a plain block into a named group: before the member `before`, which is
 * the slot a section shows that member in, or at the end. `null` when the
 * block may not join.
 */
export const appendToGroup = (
  nodes: readonly DraftNode[],
  groupKey: BlockKey,
  block: BlockDraft,
  before: BlockKey | null = null,
): readonly DraftNode[] | null => {
  if (isGroup(block)) {
    return null;
  }
  const groupIndex = nodes.findIndex(
    (node) => node.kind === 'group' && node.marker.key === groupKey,
  );
  const group = nodes[groupIndex];
  if (group === undefined || group.kind !== 'group') {
    return null;
  }
  const at = group.members.findIndex((member) => member.key === before);
  const members =
    at < 0
      ? [...group.members, block]
      : [...group.members.slice(0, at), block, ...group.members.slice(at)];
  return nodes.map((node, index) =>
    index === groupIndex ? { kind: 'group' as const, marker: group.marker, members } : node,
  );
};

/** The node boundary at or after a flat index; templates never join a group. */
const topLevelIndex = (nodes: readonly DraftNode[], index: number): number => {
  let offset = 0;
  for (let nodeIndex = 0; nodeIndex < nodes.length; nodeIndex += 1) {
    const node = nodes[nodeIndex];
    if (node === undefined) {
      break;
    }
    const size = nodeSize(node);
    if (index <= offset) {
      return nodeIndex;
    }
    if (index < offset + size) {
      return nodeIndex + 1;
    }
    offset += size;
  }
  return nodes.length;
};

export const insertTemplates = (
  nodes: readonly DraftNode[],
  index: number,
  incoming: readonly DraftNode[],
): readonly BlockDraft[] => {
  const clamped = Math.max(0, Math.min(index, flatLength(nodes)));
  const at = topLevelIndex(nodes, clamped);
  return renderNodes([...nodes.slice(0, at), ...incoming, ...nodes.slice(at)]);
};

export const removeNodes = (
  nodes: readonly DraftNode[],
  key: BlockKey,
): readonly DraftNode[] => {
  const result: DraftNode[] = [];
  for (const node of nodes) {
    if (node.kind === 'block') {
      if (node.block.key !== key) {
        result.push(node);
      }
      continue;
    }
    if (node.marker.key === key) {
      // Remove only the marker; its members become top-level blocks.
      result.push(...node.members.map((block) => ({ kind: 'block' as const, block })));
      continue;
    }
    result.push({
      kind: 'group',
      marker: node.marker,
      members: node.members.filter((member) => member.key !== key),
    });
  }
  return result;
};

export const takeNode = (
  nodes: readonly DraftNode[],
  key: BlockKey,
): { readonly nodes: readonly DraftNode[]; readonly removed: BlockDraft | null } => {
  let removed: BlockDraft | null = null;
  const result: DraftNode[] = [];
  for (const node of nodes) {
    if (node.kind === 'block') {
      if (node.block.key === key) {
        removed = node.block;
      } else {
        result.push(node);
      }
      continue;
    }
    if (node.marker.key === key) {
      removed = node.marker;
      result.push(...node.members.map((block) => ({ kind: 'block' as const, block })));
      continue;
    }
    const memberIndex = node.members.findIndex((member) => member.key === key);
    if (memberIndex < 0) {
      result.push(node);
      continue;
    }
    removed = node.members[memberIndex] ?? null;
    result.push({
      kind: 'group',
      marker: node.marker,
      members: node.members.filter((_, index) => index !== memberIndex),
    });
  }
  return { nodes: result, removed };
};

export const replaceLayoutNodes = (
  nodes: readonly DraftNode[],
  key: BlockKey,
  replacement: BlockDraft,
): readonly DraftNode[] => {
  const result: DraftNode[] = [];
  for (const node of nodes) {
    if (node.kind === 'block') {
      if (node.block.key !== key) {
        result.push(node);
      } else {
        result.push(
          isGroup(replacement)
            ? { kind: 'group', marker: replacement, members: [] }
            : { kind: 'block', block: replacement },
        );
      }
      continue;
    }
    if (node.marker.key === key) {
      if (isGroup(replacement)) {
        result.push({ kind: 'group', marker: replacement, members: node.members });
      } else {
        result.push({ kind: 'block', block: replacement });
        result.push(...node.members.map((block) => ({ kind: 'block' as const, block })));
      }
      continue;
    }
    const memberIndex = node.members.findIndex((member) => member.key === key);
    if (memberIndex < 0) {
      result.push(node);
      continue;
    }
    if (isGroup(replacement)) {
      // A group cannot hold a group, so this one closes the outer group.
      result.push({
        kind: 'group',
        marker: node.marker,
        members: node.members.slice(0, memberIndex),
      });
      result.push({ kind: 'group', marker: replacement, members: [] });
      result.push(
        ...node.members.slice(memberIndex + 1).map((block) => ({ kind: 'block' as const, block })),
      );
    } else {
      result.push({
        kind: 'group',
        marker: node.marker,
        members: node.members.map((member) => (member.key === key ? replacement : member)),
      });
    }
  }
  return result;
};
