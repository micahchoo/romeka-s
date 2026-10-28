/**
 * The user's saved lists as changes: save one, rename one, delete one. A change is checked and
 * applied to the lists `GET /api/me` answers at that moment, then the whole list is written back
 * as the `romeka_saved_lists` setting, so a list saved in another browser since this page loaded
 * is kept. Two writes at the same moment still keep only the last.
 */
import { api } from '../../api/client';
import { setSession } from '../../api/session';
import type { SavedList } from '../../api/types';

/** The most saved lists one user keeps (`MOST_SAVED_LISTS` in `crates/omeka-api/src/settings.rs`). */
export const MOST_SAVED_LISTS = 100;

/** The longest name the server stores, in characters. */
export const LONGEST_NAME = 200;

export type ListChange =
  | { readonly kind: 'save'; readonly list: SavedList }
  | { readonly kind: 'rename'; readonly from: string; readonly to: string }
  | { readonly kind: 'delete'; readonly name: string };

/** Why a change cannot apply: no name, a name in use, the list full, or the list gone. */
export type ChangeProblem = 'empty' | 'taken' | 'tooMany' | 'gone';

export type ChangeResult =
  | { readonly ok: true; readonly lists: readonly SavedList[] }
  | { readonly ok: false; readonly problem: ChangeProblem };

const has = (lists: readonly SavedList[], name: string): boolean => lists.some((list) => list.name === name);

/** What stops `change` on `lists`, or `null`. Names compare trimmed, as the server stores them. */
export const problemOf = (lists: readonly SavedList[], change: ListChange): ChangeProblem | null => {
  switch (change.kind) {
    case 'save':
      if (change.list.name === '') {
        return 'empty';
      }
      if (has(lists, change.list.name)) {
        return 'taken';
      }
      return lists.length >= MOST_SAVED_LISTS ? 'tooMany' : null;
    case 'rename':
      if (!has(lists, change.from)) {
        return 'gone';
      }
      if (change.to === '') {
        return 'empty';
      }
      return change.to !== change.from && has(lists, change.to) ? 'taken' : null;
    case 'delete':
      return null;
  }
};

/** `lists` after `change`, which `problemOf` passed. A new list goes last. */
export const applyChange = (lists: readonly SavedList[], change: ListChange): readonly SavedList[] => {
  switch (change.kind) {
    case 'save':
      return [...lists, change.list];
    case 'rename':
      return lists.map((list) => (list.name === change.from ? { ...list, name: change.to } : list));
    case 'delete':
      return lists.filter((list) => list.name !== change.name);
  }
};

/** Read the lists, apply `change`, write them, and give the session the result for the nav. */
export const changeSavedLists = async (change: ListChange): Promise<ChangeResult> => {
  const user = await api.getMe();
  if (user === null) {
    throw new Error('Not signed in');
  }
  const problem = problemOf(user.savedLists, change);
  if (problem !== null) {
    return { ok: false, problem };
  }
  const lists = applyChange(user.savedLists, change);
  await api.updateUserSettings(user.id, { savedLists: lists });
  setSession({ ...user, savedLists: lists });
  return { ok: true, lists };
};
