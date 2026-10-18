/**
 * The page editor's draft model: one pure value a block builder reads and
 * writes. It keeps the wire shape in `src/api/types.ts` at the edges and holds
 * the page between them.
 *
 * The model is lossless where upstream is narrow. A key the registry does not
 * name, a layout it does not register and a block's attachments all survive
 * `fromApi` and `toApi` unchanged, because `docs/compat.md` promises that an
 * upstream save ignores the extra rather than losing it.
 *
 * A block group is stored flat upstream: its `data.span` counts the blocks that
 * follow it. The group helpers derive that count from the blocks the group
 * owns, so the reducer cannot produce a span that disagrees with the list.
 * Removing a group removes only the marker and keeps its members; moving a
 * marker leaves its members where they were.
 */
import type {
  ItemId,
  JsonValue,
  MediaId,
  SiteBlockAttachmentWrite,
  SiteBlockWrite,
  SiteId,
  SitePage,
  SitePageWrite,
} from '../../../api/types';
import { BLOCK_REGISTRY } from '../blockRegistry';
import type { BlockRegistry } from '../blockSchema';
import {
  appendToGroup,
  canonicalBlocks,
  insertAsNodes,
  insertTemplates,
  isGroup,
  parseNodes,
  removeNodes,
  renderNodes,
  replaceLayoutNodes,
  spanOf,
  takeNode,
} from './groups';
import { defaultsFor, romkaSchemaFor, schemaFor, settingsDefaults, validateFields } from './schema';
import {
  carrierOf,
  newCarrier,
  romkaFor,
  settingsOf,
  withSettings,
  withoutPlainHash,
} from './romeka';
import {
  contentEquals,
  dataObject,
  layoutDataObject,
  layoutObject,
  readData,
  readLayoutData,
} from './values';

export { dataObject, layoutDataObject };

/** A stable client key. Keys survive moves and never name two blocks. */
export type BlockKey = string & { readonly __brand: 'BlockKey' };

const blockKey = (index: number): BlockKey =>
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions -- the one place a generated string becomes a key
  `b${String(index)}` as BlockKey;

/**
 * A block's `o:data`. Known layouts store a map of fields; an unknown layout is
 * kept in whichever of the two shapes the write accepts.
 */
export type BlockData = Readonly<Record<string, JsonValue>> | readonly JsonValue[];

/** A block's or page's `o:layout_data`, the same two shapes. */
export type LayoutData = Readonly<Record<string, JsonValue>> | readonly JsonValue[];

/** One `o:attachment` reference. The caption travels with the item and media. */
export interface BlockAttachment {
  readonly item: ItemId | null;
  readonly media: MediaId | null;
  readonly caption: string;
}

/** One block as the editor sees it. */
export interface BlockDraft {
  readonly key: BlockKey;
  readonly layout: string;
  readonly data: BlockData;
  readonly layoutData: LayoutData | null;
  readonly attachments: readonly BlockAttachment[];
}

/** `site_page.layout`: only `grid` selects the grid, every other value is normal. */
export type PageLayoutName = 'normal' | 'grid';

/** The page members an edit changes. History stores these, not the whole state. */
export interface PageContent {
  readonly title: string;
  readonly slug: string;
  readonly isPublic: boolean;
  readonly layout: PageLayoutName;
  readonly layoutData: LayoutData;
  readonly blocks: readonly BlockDraft[];
}

/** The draft, its saved baseline and the two undo stacks. */
export interface PageDraft extends PageContent {
  readonly saved: PageContent;
  readonly past: readonly PageContent[];
  readonly future: readonly PageContent[];
  /** The next client key to hand out; it only grows, so an undone key is never reused. */
  readonly nextKey: number;
  /** The schema the draft was read against, so every action uses the same one. */
  readonly registry: BlockRegistry;
  /**
   * The field the previous commit typed into, or `null` when the previous
   * commit was any other edit. Consecutive edits of one field share an undo
   * entry, so a typed word undoes in one step.
   */
  readonly edit: string | null;
}

/** One edit the reducer accepts. */
export type DraftAction =
  | {
      readonly type: 'addBlock';
      readonly layout: string;
      readonly index: number;
      /** Append to this group instead of using the flat index. */
      readonly into?: BlockKey | null;
    }
  | { readonly type: 'removeBlock'; readonly blockKey: BlockKey }
  | {
      readonly type: 'moveBlock';
      readonly blockKey: BlockKey;
      readonly toIndex: number;
      /** Join this group, or `null` to force a top-level block. Omit for position only. */
      readonly into?: BlockKey | null;
    }
  | { readonly type: 'setLayout'; readonly blockKey: BlockKey; readonly layout: string }
  | {
      readonly type: 'setField';
      readonly blockKey: BlockKey;
      readonly field: string;
      readonly value: JsonValue;
    }
  | { readonly type: 'setBlockData'; readonly blockKey: BlockKey; readonly data: BlockData }
  | {
      readonly type: 'setLayoutData';
      readonly blockKey: BlockKey;
      readonly key: string;
      readonly value: JsonValue;
    }
  | {
      readonly type: 'setRomkaSetting';
      readonly blockKey: BlockKey;
      readonly field: string;
      readonly value: JsonValue;
    }
  | { readonly type: 'useRomkaVersion'; readonly blockKey: BlockKey }
  | {
      readonly type: 'setAttachments';
      readonly blockKey: BlockKey;
      readonly attachments: readonly BlockAttachment[];
    }
  | { readonly type: 'setPage'; readonly field: 'title' | 'slug'; readonly value: string }
  | { readonly type: 'setPage'; readonly field: 'isPublic'; readonly value: boolean }
  | { readonly type: 'setPageLayout'; readonly layout: PageLayoutName }
  | { readonly type: 'setPageLayoutData'; readonly key: string; readonly value: JsonValue }
  | { readonly type: 'insertBlocks'; readonly index: number; readonly blocks: readonly BlockDraft[] }
  | { readonly type: 'undo' }
  | { readonly type: 'redo' }
  | { readonly type: 'markSaved' };

/** One schema violation, named by the block it belongs to (`null` is the page). */
export type ValidationIssue =
  | { readonly kind: 'required'; readonly blockKey: BlockKey | null; readonly field: string }
  | { readonly kind: 'min'; readonly blockKey: BlockKey; readonly field: string; readonly min: number }
  | { readonly kind: 'max'; readonly blockKey: BlockKey; readonly field: string; readonly max: number }
  | { readonly kind: 'type'; readonly blockKey: BlockKey; readonly field: string };

const emptyContent = (): PageContent => ({
  title: '',
  slug: '',
  isPublic: true,
  layout: 'normal',
  layoutData: {},
  blocks: [],
});

const contentOf = (state: PageDraft): PageContent => ({
  title: state.title,
  slug: state.slug,
  isPublic: state.isPublic,
  layout: state.layout,
  layoutData: state.layoutData,
  blocks: state.blocks,
});

/**
 * Records one content value as the current draft. A commit tagged with the
 * same `edit` as the previous one keeps the history it already has, so a run
 * of edits to one field is one undo step.
 */
const commit = (
  state: PageDraft,
  content: PageContent,
  nextKey = state.nextKey,
  edit: string | null = null,
): PageDraft => {
  if (contentEquals(content, contentOf(state))) {
    return state;
  }
  const coalesced = edit !== null && state.edit === edit;
  return {
    ...content,
    saved: state.saved,
    past: coalesced ? state.past : [...state.past, contentOf(state)],
    future: [],
    nextKey,
    registry: state.registry,
    edit,
  };
};

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

/** A blank page, the create form's starting state. */
export const emptyDraft = (registry: BlockRegistry = BLOCK_REGISTRY): PageDraft => {
  const content = emptyContent();
  return { ...content, saved: content, past: [], future: [], nextKey: 0, registry, edit: null };
};

/** The key `addBlock` gives the block it appends, so a caller can select it. */
export const nextBlockKey = (state: PageDraft): BlockKey => blockKey(state.nextKey);

/**
 * Reads a stored page. Unknown data and layout-data keys are kept verbatim, an
 * unknown layout keeps its payload, and a group span is clamped to the blocks
 * it can own.
 */
export const fromApi = (page: SitePage, registry: BlockRegistry): PageDraft => {
  const content: PageContent = {
    title: page.title,
    slug: page.slug,
    isPublic: page.isPublic,
    layout: page.layout === 'grid' ? 'grid' : 'normal',
    layoutData: readData(page.layoutData),
    blocks: canonicalBlocks(
      page.blocks.map((block, index) => ({
        key: blockKey(index),
        layout: block.layout,
        data: readData(block.data),
        layoutData: readLayoutData(block.layoutData),
        attachments: block.attachments.map((entry) => ({
          item: entry.item === null ? null : entry.item.id,
          media: entry.media === null ? null : entry.media.id,
          caption: entry.caption,
        })),
      })),
    ),
  };
  return {
    ...content,
    saved: content,
    past: [],
    future: [],
    nextKey: content.blocks.length,
    registry,
    edit: null,
  };
};

/** The `o:block` writes for a draft. A group's span is written as upstream's string. */
export const toApi = (draft: PageContent): readonly SiteBlockWrite[] =>
  canonicalBlocks(draft.blocks).map((block) => ({
    layout: block.layout,
    data: isGroup(block) ? { ...dataObject(block), span: String(spanOf(block)) } : block.data,
    layoutData: block.layoutData,
    attachments: block.attachments.map(
      (entry): SiteBlockAttachmentWrite => ({
        item: entry.item,
        media: entry.media,
        caption: entry.caption,
      }),
    ),
  }));

/** The whole page write, which `api.createSitePage` and `api.updateSitePage` take. */
export const toPageWrite = (draft: PageContent, siteId: SiteId): SitePageWrite => ({
  site: siteId,
  title: draft.title,
  slug: draft.slug,
  isPublic: draft.isPublic,
  layout: draft.layout === 'grid' ? 'grid' : null,
  layoutData: draft.layoutData,
  blocks: toApi(draft),
});

/** The reducer. */
export const draftReducer = (state: PageDraft, action: DraftAction): PageDraft => {
  switch (action.type) {
    case 'addBlock': {
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
    }
    case 'removeBlock': {
      const blocks = renderNodes(removeNodes(parseNodes(state.blocks), action.blockKey));
      return commit(state, { ...contentOf(state), blocks });
    }
    case 'moveBlock': {
      const { nodes, removed } = takeNode(parseNodes(state.blocks), action.blockKey);
      if (removed === null) {
        return state;
      }
      if (action.into !== undefined && action.into !== null) {
        const appended = appendToGroup(nodes, action.into, removed);
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
    }
    case 'setLayout': {
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
    }
    case 'setField': {
      const target = state.blocks.find((block) => block.key === action.blockKey);
      if (target === undefined || (isGroup(target) && action.field === 'span')) {
        return state;
      }
      const next: BlockDraft = {
        ...target,
        data: { ...dataObject(target), [action.field]: action.value },
      };
      return commit(
        state,
        {
          ...contentOf(state),
          blocks: state.blocks.map((block) => (block.key === action.blockKey ? next : block)),
        },
        state.nextKey,
        `field:${action.blockKey}:${action.field}`,
      );
    }
    case 'setBlockData': {
      if (!state.blocks.some((block) => block.key === action.blockKey)) {
        return state;
      }
      return commit(state, {
        ...contentOf(state),
        blocks: state.blocks.map((block) =>
          block.key === action.blockKey ? { ...block, data: action.data } : block,
        ),
      });
    }
    case 'setLayoutData': {
      const target = state.blocks.find((block) => block.key === action.blockKey);
      if (target === undefined) {
        return state;
      }
      const next: BlockDraft = {
        ...target,
        layoutData: { ...layoutDataObject(target), [action.key]: action.value },
      };
      return commit(
        state,
        {
          ...contentOf(state),
          blocks: state.blocks.map((block) => (block.key === action.blockKey ? next : block)),
        },
        state.nextKey,
        `layoutData:${action.blockKey}:${action.key}`,
      );
    }
    case 'setRomkaSetting': {
      const target = state.blocks.find((block) => block.key === action.blockKey);
      if (target === undefined || romkaFor(state.registry, target) === undefined) {
        return state;
      }
      const settings = { ...settingsOf(target), [action.field]: action.value };
      const next: BlockDraft = {
        ...target,
        layoutData:
          target.layoutData === null
            ? null
            : withSettings(target.layoutData, settings),
      };
      return commit(
        state,
        {
          ...contentOf(state),
          blocks: state.blocks.map((block) => (block.key === action.blockKey ? next : block)),
        },
        state.nextKey,
        `romkaSetting:${action.blockKey}:${action.field}`,
      );
    }
    case 'useRomkaVersion': {
      const target = state.blocks.find((block) => block.key === action.blockKey);
      if (target === undefined || target.layoutData === null || carrierOf(target) === null) {
        return state;
      }
      const next: BlockDraft = {
        ...target,
        layoutData: withoutPlainHash(target.layoutData),
      };
      return commit(state, {
        ...contentOf(state),
        blocks: state.blocks.map((block) => (block.key === action.blockKey ? next : block)),
      });
    }
    case 'setAttachments': {      const target = state.blocks.find((block) => block.key === action.blockKey);
      if (target === undefined) {
        return state;
      }
      return commit(state, {
        ...contentOf(state),
        blocks: state.blocks.map((block) =>
          block.key === action.blockKey ? { ...block, attachments: action.attachments } : block,
        ),
      });
    }
    case 'setPage':
      if (action.field === 'isPublic') {
        return commit(state, { ...contentOf(state), isPublic: action.value });
      }
      return commit(
        state,
        { ...contentOf(state), [action.field]: action.value },
        state.nextKey,
        `page:${action.field}`,
      );
    case 'setPageLayout':
      return commit(state, { ...contentOf(state), layout: action.layout });
    case 'setPageLayoutData':
      return commit(
        state,
        {
          ...contentOf(state),
          layoutData: { ...layoutObject(state.layoutData), [action.key]: action.value },
        },
        state.nextKey,
        `pageLayoutData:${action.key}`,
      );
    case 'insertBlocks': {
      const keyed = action.blocks.map((block, offset) => ({
        ...block,
        key: blockKey(state.nextKey + offset),
      }));
      const blocks = insertTemplates(parseNodes(state.blocks), action.index, parseNodes(keyed));
      return commit(state, { ...contentOf(state), blocks }, state.nextKey + action.blocks.length);
    }
    case 'undo': {
      const previous = state.past[state.past.length - 1];
      if (previous === undefined) {
        return state;
      }
      return {
        ...previous,
        saved: state.saved,
        past: state.past.slice(0, -1),
        future: [contentOf(state), ...state.future],
        nextKey: state.nextKey,
        registry: state.registry,
        edit: null,
      };
    }
    case 'redo': {
      const next = state.future[0];
      if (next === undefined) {
        return state;
      }
      return {
        ...next,
        saved: state.saved,
        past: [...state.past, contentOf(state)],
        future: state.future.slice(1),
        nextKey: state.nextKey,
        registry: state.registry,
        edit: null,
      };
    }
    case 'markSaved':
      return { ...state, saved: contentOf(state), edit: null };
  }
};

/** Whether the page differs from the state it was loaded or last saved as. */
export const isDirty = (state: PageDraft): boolean =>
  !contentEquals(contentOf(state), state.saved);

/** Every schema violation in the draft, page title included. */
export const validate = (state: PageDraft): readonly ValidationIssue[] => {
  const issues: ValidationIssue[] =
    state.title.trim() === '' ? [{ kind: 'required', blockKey: null, field: 'title' }] : [];
  for (const block of state.blocks) {
    const romka = romkaFor(state.registry, block);
    if (romka !== undefined) {
      issues.push(...validateFields(block.key, romka.fields, settingsOf(block), ''));
      continue;
    }
    const schema = schemaFor(state.registry, block.layout);
    if (schema === undefined) {
      continue;
    }
    issues.push(...validateFields(block.key, schema.fields, dataObject(block), ''));
  }
  return issues;
};
