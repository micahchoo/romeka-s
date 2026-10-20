/**
 * The picker's results: the pages of `/api/items` read so far for one query.
 * "Show more" appends the next page, so earlier rows stay in view; a new query
 * starts over at page one; a failed page keeps the rows already read and can
 * be retried.
 */
import { useEffect, useReducer } from 'react';
import { api } from '../../../../api/client';
import type { ResourceQuery } from '../../../../api/client';
import type { Item } from '../../../../api/types';
import { toError } from '../../../../app/useAsync';

export const PAGE_SIZE = 24;

export interface Listing {
  readonly key: string;
  /** The last page asked for; the rows hold every page before it once it is read. */
  readonly page: number;
  readonly attempt: number;
  readonly status: 'loading' | 'ready' | 'error';
  readonly rows: readonly Item[];
  /** `null` until the first page of this query answers. */
  readonly total: number | null;
  readonly error: Error | null;
}

type Event =
  | { readonly type: 'query'; readonly key: string }
  | { readonly type: 'more' }
  | { readonly type: 'retry' }
  | {
      readonly type: 'read';
      readonly key: string;
      readonly page: number;
      readonly items: readonly Item[];
      readonly total: number;
    }
  | { readonly type: 'failed'; readonly key: string; readonly page: number; readonly error: Error };

const start = (key: string): Listing => ({
  key,
  page: 1,
  attempt: 0,
  status: 'loading',
  rows: [],
  total: null,
  error: null,
});

const reduce = (state: Listing, event: Event): Listing => {
  switch (event.type) {
    case 'query':
      return event.key === state.key ? state : start(event.key);
    case 'more':
      return state.status === 'ready' ? { ...state, page: state.page + 1, status: 'loading' } : state;
    case 'retry':
      return { ...state, attempt: state.attempt + 1, status: 'loading', error: null };
    case 'read':
      if (event.key !== state.key || event.page !== state.page) {
        return state;
      }
      return {
        ...state,
        status: 'ready',
        rows: event.page === 1 ? event.items : [...state.rows, ...event.items],
        total: event.total,
      };
    case 'failed':
      if (event.key !== state.key || event.page !== state.page) {
        return state;
      }
      return { ...state, status: 'error', error: event.error };
  }
};

export const useItemListing = (
  query: ResourceQuery,
): { readonly listing: Listing; readonly more: () => void; readonly retry: () => void } => {
  const key = JSON.stringify(query);
  const [listing, dispatch] = useReducer(reduce, key, start);
  useEffect(() => {
    dispatch({ type: 'query', key });
  }, [key]);
  useEffect(() => {
    // A query that changed in the same render resets the listing first, and
    // the reset runs this effect again for the new query.
    if (listing.key !== key) {
      return undefined;
    }
    let cancelled = false;
    const { key: asked, page } = listing;
    api.listItems({ ...query, page, perPage: PAGE_SIZE }).then(
      (result) => {
        if (!cancelled) {
          dispatch({ type: 'read', key: asked, page, items: result.items, total: result.total });
        }
      },
      (error: unknown) => {
        if (!cancelled) {
          dispatch({ type: 'failed', key: asked, page, error: toError(error) });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [listing.key, listing.page, listing.attempt]);
  return {
    listing,
    more: () => {
      dispatch({ type: 'more' });
    },
    retry: () => {
      dispatch({ type: 'retry' });
    },
  };
};
