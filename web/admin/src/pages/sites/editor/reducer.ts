/**
 * The draft reducer: one line per action, routed to the module that owns its
 * domain. The page's own members and a whole-page template are edited here.
 */
import type { ActionOf, DraftAction, PageDraft } from './draft';
import {
  adoptRomkaVersion,
  setAttachments,
  setBlockData,
  setField,
  setRomkaSetting,
  writeLayoutData,
} from './blockContent';
import { addBlock, insertBlocks, moveBlock, removeBlock, setLayout } from './blockList';
import { canonicalBlocks } from './groups';
import { commit, contentOf, markSaved, redo, undo } from './history';
import { layoutObject, readData } from './values';
import { contentFromApi, fromTemplateBlock } from './wire';

const setPage = (state: PageDraft, action: ActionOf<'setPage'>): PageDraft => {
  if (action.field === 'isPublic') {
    return commit(state, { ...contentOf(state), isPublic: action.value });
  }
  return commit(
    state,
    { ...contentOf(state), [action.field]: action.value },
    state.nextKey,
    `page:${action.field}`,
  );
};

const setPageLayoutData = (state: PageDraft, action: ActionOf<'setPageLayoutData'>): PageDraft =>
  commit(
    state,
    {
      ...contentOf(state),
      layoutData: { ...layoutObject(state.layoutData), [action.key]: action.value },
    },
    state.nextKey,
    `pageLayoutData:${action.key}`,
  );

const applyTemplate = (state: PageDraft, action: ActionOf<'applyTemplate'>): PageDraft => {
  const keyed = action.template.blocks.map((block, offset) =>
    fromTemplateBlock(state.nextKey + offset, block),
  );
  return commit(
    state,
    {
      ...contentOf(state),
      layout: action.template.layout,
      layoutData: readData(action.template.layoutData),
      blocks: canonicalBlocks(keyed),
    },
    state.nextKey + keyed.length,
  );
};

const loadPage = (state: PageDraft, action: ActionOf<'loadPage'>): PageDraft => {
  const content = contentFromApi(action.page, state.nextKey);
  const next = commit(state, content, state.nextKey + content.blocks.length);
  return action.saved ? { ...next, saved: contentOf(next) } : next;
};

export const draftReducer = (state: PageDraft, action: DraftAction): PageDraft => {
  switch (action.type) {
    case 'addBlock':
      return addBlock(state, action);
    case 'removeBlock':
      return removeBlock(state, action);
    case 'moveBlock':
      return moveBlock(state, action);
    case 'setLayout':
      return setLayout(state, action);
    case 'insertBlocks':
      return insertBlocks(state, action);
    case 'setField':
      return setField(state, action);
    case 'setBlockData':
      return setBlockData(state, action);
    case 'setLayoutData':
      return writeLayoutData(state, action.blockKey, { [action.key]: action.value });
    case 'setLayoutDataValues':
      return writeLayoutData(state, action.blockKey, action.values);
    case 'setRomkaSetting':
      return setRomkaSetting(state, action);
    case 'useRomkaVersion':
      return adoptRomkaVersion(state, action);
    case 'setAttachments':
      return setAttachments(state, action);
    case 'setPage':
      return setPage(state, action);
    case 'setPageLayout':
      return commit(state, { ...contentOf(state), layout: action.layout });
    case 'setPageLayoutData':
      return setPageLayoutData(state, action);
    case 'applyTemplate':
      return applyTemplate(state, action);
    case 'loadPage':
      return loadPage(state, action);
    case 'undo':
      return undo(state);
    case 'redo':
      return redo(state);
    case 'markSaved':
      return markSaved(state);
  }
};
