/**
 * The record a page just moved to the trash, held for the list screen it opens next, which
 * offers the Undo. One slot is enough: one delete navigates to one screen, and each screen takes
 * only a record of its own kind.
 *
 * The screen reads the slot when it mounts and clears it in an effect, not in the read, because a
 * development build's `StrictMode` runs a state initializer twice. A reload loses the slot; the
 * run stays in the bulk panel's recent changes, with its Undo.
 */
import type { BulkId, TrashOf } from '../../api/bulk';
import type { ItemId, ItemSetId, MediaId } from '../../api/types';

/** One record a delete of it alone moves to the trash. */
export type TrashTarget =
  | { readonly of: 'items'; readonly id: ItemId }
  | { readonly of: 'media'; readonly id: MediaId }
  | { readonly of: 'item_sets'; readonly id: ItemSetId };

export interface Trashed {
  /** The bulk delete that moved it: its undo is the Undo. */
  readonly run: BulkId;
  readonly target: TrashTarget;
  readonly name: string;
}

let slot: Trashed | null = null;

export const rememberTrashed = (trashed: Trashed): void => {
  slot = trashed;
};

/** The record in the slot, if it is of the kind `of`. */
export const peekTrashed = (of: TrashOf): Trashed | null => (slot?.target.of === of ? slot : null);

/** Clear the slot if it still holds `trashed`; a later delete's entry stays. */
export const forgetTrashed = (trashed: Trashed): void => {
  if (slot === trashed) {
    slot = null;
  }
};
