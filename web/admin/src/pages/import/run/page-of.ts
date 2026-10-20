/** One page of rows the screen already holds, for `PagedList`, so a long list draws at most a page. */
import type { Paged } from '../../../api/import-sections';

export const pageOf =
  <T>(rows: readonly T[]) =>
  (page: number, perPage: number): Promise<Paged<T>> =>
    Promise.resolve({ total: rows.length, page, perPage, items: rows.slice((page - 1) * perPage, page * perPage) });
