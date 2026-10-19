/**
 * The draft's history: the saved baseline and the two undo stacks. Every edit
 * goes through `commit`, so an edit that changes nothing leaves no undo entry
 * and a run of edits to one field is one undo step.
 */
import type { PageContent, PageDraft } from './draft';
import { contentEquals } from './values';

export const contentOf = (state: PageDraft): PageContent => ({
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
export const commit = (
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

export const undo = (state: PageDraft): PageDraft => {
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
};

export const redo = (state: PageDraft): PageDraft => {
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
};

export const markSaved = (state: PageDraft): PageDraft => ({
  ...state,
  saved: contentOf(state),
  edit: null,
});

/** Whether the page differs from the state it was loaded or last saved as. */
export const isDirty = (state: PageDraft): boolean =>
  !contentEquals(contentOf(state), state.saved);
