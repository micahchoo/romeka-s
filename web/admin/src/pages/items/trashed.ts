/**
 * The item the item page just moved to the trash, held for the Items screen it opens next, which
 * offers the Undo. One slot is enough: one delete navigates to one screen.
 *
 * The screen reads the slot when it mounts and clears it in an effect, not in the read, because a
 * development build's `StrictMode` runs a state initializer twice. A reload loses the slot; the
 * run stays in the bulk panel's recent changes, with its Undo.
 */
import type { BulkId } from '../../api/bulk';
import type { ItemId } from '../../api/types';

export interface Trashed {
  /** The bulk delete that moved it: its undo is the Undo. */
  readonly run: BulkId;
  readonly item: ItemId;
  readonly name: string;
}

let slot: Trashed | null = null;

export const rememberTrashed = (trashed: Trashed): void => {
  slot = trashed;
};

export const peekTrashed = (): Trashed | null => slot;

/** Clear the slot if it still holds `trashed`; a later delete's entry stays. */
export const forgetTrashed = (trashed: Trashed): void => {
  if (slot === trashed) {
    slot = null;
  }
};
