/**
 * The page editor's draft model: one pure value a block builder reads and
 * writes. It keeps the wire shape in `src/api/types.ts` at the edges (`wire.ts`)
 * and holds the page between them.
 *
 * This module names the model and its rules. The edits live by domain:
 * `blockList.ts` changes which blocks the page holds, `blockContent.ts` edits
 * inside one block, `history.ts` keeps the undo stacks, and `reducer.ts`
 * routes each action to one of them.
 */
import { BLOCK_REGISTRY } from '../blockRegistry';
import type { BlockRegistry } from '../blockSchema';
import type { TemplateBlock, TemplateContent } from '../templates';
import type { ItemId, JsonValue, MediaId } from '../../../api/types';
import { romkaFor, settingsOf } from './romeka';
import { schemaFor, validateFields } from './schema';
import { readPath } from './layoutPaths';
import { dataObject, layoutDataObject } from './values';

/** A stable client key. Keys survive moves and never name two blocks. */
export type BlockKey = string & { readonly __brand: 'BlockKey' };

/** The key of the block made `index`-th in a draft's life; only a draft edit or read mints one. */
export const blockKey = (index: number): BlockKey =>
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

/** One edit `draftReducer` accepts. */
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
      /** With `into`: take the slot of this member, which moves one slot on. Omit to append. */
      readonly before?: BlockKey;
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
      /** Several layout-data keys written as one edit, such as four padding sides. */
      readonly type: 'setLayoutDataValues';
      readonly blockKey: BlockKey;
      readonly values: Readonly<Record<string, JsonValue>>;
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
  | { readonly type: 'applyTemplate'; readonly template: TemplateContent }
  | { readonly type: 'insertBlocks'; readonly index: number; readonly blocks: readonly TemplateBlock[] }
  | { readonly type: 'undo' }
  | { readonly type: 'redo' }
  | { readonly type: 'markSaved' };

/** One schema violation, named by the block it belongs to (`null` is the page). */
export type ValidationIssue =
  | { readonly kind: 'required'; readonly blockKey: BlockKey | null; readonly field: string }
  | { readonly kind: 'min'; readonly blockKey: BlockKey; readonly field: string; readonly min: number }
  | { readonly kind: 'max'; readonly blockKey: BlockKey; readonly field: string; readonly max: number }
  | { readonly kind: 'type'; readonly blockKey: BlockKey; readonly field: string }
  /** An anchor an earlier block of the page already uses. */
  | { readonly kind: 'taken'; readonly blockKey: BlockKey; readonly field: string };

/** One action of the given type, which an edit function takes. */
export type ActionOf<Type extends DraftAction['type']> = Extract<DraftAction, { readonly type: Type }>;

/** A blank page, the create form's starting state. */
export const emptyDraft = (registry: BlockRegistry = BLOCK_REGISTRY): PageDraft => {
  const content: PageContent = {
    title: '',
    slug: '',
    isPublic: true,
    layout: 'normal',
    layoutData: {},
    blocks: [],
  };
  return { ...content, saved: content, past: [], future: [], nextKey: 0, registry, edit: null };
};

/** The key `addBlock` gives the block it appends, so a caller can select it. */
export const nextBlockKey = (state: PageDraft): BlockKey => blockKey(state.nextKey);

/** The layout-data path of a block's anchor (`blocks/style.rs`). */
export const ANCHOR_PATH = 'romeka.anchor';

/**
 * A block whose anchor an earlier block already uses. The renderer gives the
 * id to the first block only, so a second one would link nowhere.
 */
const anchorIssues = (blocks: readonly BlockDraft[]): readonly ValidationIssue[] => {
  const seen = new Set<string>();
  const issues: ValidationIssue[] = [];
  for (const block of blocks) {
    const anchor = readPath(layoutDataObject(block), ANCHOR_PATH);
    if (typeof anchor !== 'string' || anchor === '') {
      continue;
    }
    if (seen.has(anchor)) {
      issues.push({ kind: 'taken', blockKey: block.key, field: ANCHOR_PATH });
    }
    seen.add(anchor);
  }
  return issues;
};

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
  issues.push(...anchorIssues(state.blocks));
  return issues;
};
