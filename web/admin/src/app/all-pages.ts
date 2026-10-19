/**
 * Every member of a paged list. The first page says how many pages there are,
 * and the rest load together, so a list one page long costs one request and a
 * longer one costs one more round.
 */
import type { ApiPage } from '../api/types';

/** The page size a full read asks for: large enough that most lists fit one page. */
export const ALL_PAGES_PER_PAGE = 200;

export const allPages = async <T>(
  load: (page: number) => Promise<ApiPage<T>>,
): Promise<readonly T[]> => {
  const first = await load(1);
  const rest = await Promise.all(
    Array.from({ length: Math.max(first.lastPage - 1, 0) }, (_, index) => load(index + 2)),
  );
  return [first, ...rest].flatMap((page) => page.items);
};
