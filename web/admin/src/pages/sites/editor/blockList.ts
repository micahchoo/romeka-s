/**
 * The edits that change which blocks the page holds and in what order: add,
 * remove, move, change a block's layout, and insert a template's blocks.
 *
 * A block group is stored flat upstream: its `data.span` counts the blocks that
 * follow it. Each edit goes through the group nodes of `groups.ts`, so a span
 * never disagrees with the list. Removing a group removes only the marker and
 * keeps its members; moving a marker leaves its members where they were.
 */
import type { BlockRegistry } from '../blockSchema';
import { blockKey } from './draft';
import type { ActionOf, BlockDraft, BlockKey, PageDraft } from './draft';
import {
  appendToGroup,
  insertAsNodes,
  insertTemplates,
  isGroup,
  parseNodes,
  removeNodes,
  renderNodes,
  replaceLayoutNodes,
  takeNode,
} from './groups';
import { commit, contentOf } from './history';
import { newCarrier } from './romeka';
import { defaultsFor, romkaSchemaFor, settingsDefaults } from './schema';
import { fromTemplateBlock } from './wire';

const makeBlock = (registry: BlockRegistry, key: BlockKey, layout: string): BlockDraft => {
  const romka = romkaSchemaFor(registry, layout);
  if (romka !== undefined) {
    return {
      key,
      layout: romka.carrier,
      data: { html: '' },
      layoutData: newCarrier(
        romka.name,
        settingsDefaults(registry, romka.name),
        registry.romekaVersion,
      ),
      attachments: [],
    };
  }
  return {
    key,
    layout,
    data: defaultsFor(registry, layout),
    layoutData: null,
    attachments: [],
  };
};

export const addBlock = (state: PageDraft, action: ActionOf<'addBlock'>): PageDraft => {
  const key = blockKey(state.nextKey);
  const block = makeBlock(state.registry, key, action.layout);
  const nodes = parseNodes(state.blocks);
  const appended =
    action.into === undefined || action.into === null
      ? null
      : appendToGroup(nodes, action.into, block);
  const blocks =
    appended === null
      ? insertAsNodes(nodes, action.index, parseNodes([block]))
      : renderNodes(appended);
  return commit(state, { ...contentOf(state), blocks }, state.nextKey + 1);
};

export const removeBlock = (state: PageDraft, action: ActionOf<'removeBlock'>): PageDraft => {
  const blocks = renderNodes(removeNodes(parseNodes(state.blocks), action.blockKey));
  return commit(state, { ...contentOf(state), blocks });
};

export const moveBlock = (state: PageDraft, action: ActionOf<'moveBlock'>): PageDraft => {
  const { nodes, removed } = takeNode(parseNodes(state.blocks), action.blockKey);
  if (removed === null) {
    return state;
  }
  if (action.into !== undefined && action.into !== null) {
    const appended = appendToGroup(nodes, action.into, removed, action.before ?? null);
    if (appended !== null) {
      return commit(state, { ...contentOf(state), blocks: renderNodes(appended) });
    }
  }
  const incoming = parseNodes([removed]);
  const blocks =
    action.into === null || isGroup(removed)
      ? insertTemplates(nodes, action.toIndex, incoming)
      : insertAsNodes(nodes, action.toIndex, incoming);
  return commit(state, { ...contentOf(state), blocks });
};

export const setLayout = (state: PageDraft, action: ActionOf<'setLayout'>): PageDraft => {
  const target = state.blocks.find((block) => block.key === action.blockKey);
  if (target === undefined || target.layout === action.layout) {
    return state;
  }
  const replacement: BlockDraft = {
    ...target,
    layout: action.layout,
    data: defaultsFor(state.registry, action.layout),
  };
  const blocks = renderNodes(
    replaceLayoutNodes(parseNodes(state.blocks), action.blockKey, replacement),
  );
  return commit(state, { ...contentOf(state), blocks });
};

export const insertBlocks = (state: PageDraft, action: ActionOf<'insertBlocks'>): PageDraft => {
  const keyed = action.blocks.map((block, offset) => fromTemplateBlock(state.nextKey + offset, block));
  const blocks = insertTemplates(parseNodes(state.blocks), action.index, parseNodes(keyed));
  return commit(state, { ...contentOf(state), blocks }, state.nextKey + keyed.length);
};
