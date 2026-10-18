/**
 * The record a write returned, held for the show page that follows the write.
 *
 * An API create and update answer with the same representation a read returns
 * (`Api\Manager::finalize`), so the page can render the row without asking for
 * it a second time. One slot per resource is enough: the write navigates to one
 * show page, and the slot is consumed by that read.
 *
 * Consuming on read keeps a stale copy from serving a later navigation. In a
 * development build `StrictMode` runs the loading effect twice, so the first
 * run consumes the slot and the second refetches the record the API already
 * returned. A production build renders the seeded record.
 */
import type { Item, ItemId } from '../api/types';

export class WrittenRecord<T, IdT extends number> {
  private entry: { readonly id: IdT; readonly value: T } | null = null;

  /** Keep `value` for the show page that renders `id` next. */
  remember(id: IdT, value: T): void {
    this.entry = { id, value };
  }

  /** The remembered record for `id`, consumed. `null` when there is none. */
  take(id: IdT): T | null {
    const entry = this.entry;
    this.entry = null;
    return entry !== null && entry.id === id ? entry.value : null;
  }
}

/** The item a create returned, for the item show page it navigates to. */
export const writtenItems = new WrittenRecord<Item, ItemId>();
