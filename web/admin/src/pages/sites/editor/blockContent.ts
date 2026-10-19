/**
 * The edits inside one block: its `o:data` fields, its `o:layout_data`
 * styling, a Romeka block's settings and its `o:attachment` rows. None of
 * them changes which blocks the page holds, so a group's span is safe here.
 *
 * Typing edits carry an `edit` tag, so a typed word or a linked padding
 * undoes in one step (see `commit`).
 */
import type { JsonValue } from '../../../api/types';
import type { ActionOf, BlockDraft, BlockKey, PageDraft } from './draft';
import { isGroup } from './groups';
import { commit, contentOf } from './history';
import { carrierOf, romkaFor, settingsOf, withSettings, withoutPlainHash } from './romeka';
import { dataObject, layoutDataObject } from './values';

const findBlock = (state: PageDraft, key: BlockKey): BlockDraft | undefined =>
  state.blocks.find((block) => block.key === key);

/** Commits `next` in place of the block with the same key. */
const replaceBlock = (state: PageDraft, next: BlockDraft, edit: string | null = null): PageDraft =>
  commit(
    state,
    {
      ...contentOf(state),
      blocks: state.blocks.map((block) => (block.key === next.key ? next : block)),
    },
    state.nextKey,
    edit,
  );

export const setField = (state: PageDraft, action: ActionOf<'setField'>): PageDraft => {
  const target = findBlock(state, action.blockKey);
  if (target === undefined || (isGroup(target) && action.field === 'span')) {
    return state;
  }
  return replaceBlock(
    state,
    { ...target, data: { ...dataObject(target), [action.field]: action.value } },
    `field:${action.blockKey}:${action.field}`,
  );
};

export const setBlockData = (state: PageDraft, action: ActionOf<'setBlockData'>): PageDraft => {
  const target = findBlock(state, action.blockKey);
  return target === undefined ? state : replaceBlock(state, { ...target, data: action.data });
};

/**
 * Merges `values` into one block's `o:layout_data`. Consecutive writes of the
 * same keys share an undo entry, so typing into a linked padding undoes at once.
 */
export const writeLayoutData = (
  state: PageDraft,
  blockKey: BlockKey,
  values: Readonly<Record<string, JsonValue>>,
): PageDraft => {
  const target = findBlock(state, blockKey);
  if (target === undefined) {
    return state;
  }
  return replaceBlock(
    state,
    { ...target, layoutData: { ...layoutDataObject(target), ...values } },
    `layoutData:${blockKey}:${Object.keys(values).sort().join(',')}`,
  );
};

export const setRomkaSetting = (
  state: PageDraft,
  action: ActionOf<'setRomkaSetting'>,
): PageDraft => {
  const target = findBlock(state, action.blockKey);
  if (target === undefined || romkaFor(state.registry, target) === undefined) {
    return state;
  }
  const settings = { ...settingsOf(target), [action.field]: action.value };
  return replaceBlock(
    state,
    {
      ...target,
      layoutData: target.layoutData === null ? null : withSettings(target.layoutData, settings),
    },
    `romkaSetting:${action.blockKey}:${action.field}`,
  );
};

export const adoptRomkaVersion = (
  state: PageDraft,
  action: ActionOf<'useRomkaVersion'>,
): PageDraft => {
  const target = findBlock(state, action.blockKey);
  if (target === undefined || target.layoutData === null || carrierOf(target) === null) {
    return state;
  }
  return replaceBlock(state, { ...target, layoutData: withoutPlainHash(target.layoutData) });
};

export const setAttachments = (state: PageDraft, action: ActionOf<'setAttachments'>): PageDraft => {
  const target = findBlock(state, action.blockKey);
  return target === undefined
    ? state
    : replaceBlock(state, { ...target, attachments: action.attachments });
};
