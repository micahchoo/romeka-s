/**
 * A change to many names every entry it hits (`docs/import-v2.md`: changes
 * to many records name the records they hit), so acting on a filter means
 * reading every page the filter selects first. The pages are the server's
 * largest, read a few at a time, in order, so 40,000 items cost 80 reads and
 * the person sees how far the reading is.
 */
import type { Paged } from '../../../api/import-sections';
import type { Change, Decision, EntryRef } from '../../../api/import-plan';
import type { RecordView } from '../../../api/import-model';

/** The server's page ceiling (`preview/query.rs`, `MAX_PER_PAGE`). */
export const READ_PER_PAGE = 500;
const AT_ONCE = 4;
/** How many names the confirmation shows before "and N more". */
export const NAMES_SHOWN = 5;

export interface Gathered {
  /** What the filter selected when it was read. */
  readonly count: number;
  readonly entries: readonly EntryRef[];
  /** The first rows' names, for the confirmation. */
  readonly names: readonly string[];
}

export type LoadPage<T> = (page: number, perPage: number) => Promise<Paged<T>>;

const entryKey = (entry: EntryRef): string =>
  'file' in entry ? `f\u0000${entry.file.source}\u0000${entry.file.path}` : `r\u0000${entry.row.sheet}\u0000${entry.row.key}`;

/**
 * Reads every page of `load` and the entries `pick` takes from each row, each
 * entry once. `onRead` hears the rows read so far; a `signal` that aborts
 * stops the reading between pages.
 */
export const gather = async <T>(
  load: LoadPage<T>,
  pick: (row: T) => readonly EntryRef[],
  name: (row: T) => string,
  onRead: (read: number, total: number) => void,
  signal?: AbortSignal,
): Promise<Gathered> => {
  const first = await load(1, READ_PER_PAGE);
  const pages = Math.max(1, Math.ceil(first.total / READ_PER_PAGE));
  const results: Paged<T>[] = [first];
  let read = first.items.length;
  onRead(read, first.total);
  for (let next = 2; next <= pages; next += AT_ONCE) {
    if (signal?.aborted === true) {
      throw new DOMException('The reading was stopped', 'AbortError');
    }
    const batch = Array.from({ length: Math.min(AT_ONCE, pages - next + 1) }, (_, index) => load(next + index, READ_PER_PAGE));
    for (const page of await Promise.all(batch)) {
      results.push(page);
      read += page.items.length;
    }
    onRead(read, first.total);
  }
  const seen = new Set<string>();
  const entries: EntryRef[] = [];
  const names: string[] = [];
  for (const page of results) {
    for (const row of page.items) {
      if (names.length < NAMES_SHOWN) {
        names.push(name(row));
      }
      for (const entry of pick(row)) {
        const key = entryKey(entry);
        if (!seen.has(key)) {
          seen.add(key);
          entries.push(entry);
        }
      }
    }
  }
  return { count: read, entries, names };
};

/**
 * The one entry that names an item: its first file, else its first row. A
 * decision on a file targets the item that holds it, so one per item is
 * enough and the decision stays as short as the item count.
 */
export const itemEntry = (record: RecordView): readonly EntryRef[] => {
  const file = record.files[0];
  if (file !== undefined) {
    return [{ file }];
  }
  const row = record.rows[0];
  return row === undefined ? [] : [{ row }];
};

/** Every file of an item, for a change that moves files. */
export const fileEntries = (record: RecordView): readonly EntryRef[] => record.files.map((file) => ({ file }));

export const manyDecision = (entries: readonly EntryRef[], change: Change): Decision => ({ kind: 'many', entries, change });
