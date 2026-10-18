import { useEffect, useState } from 'react';

export type AsyncState<T> =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly error: Error }
  | { readonly status: 'ready'; readonly data: T };

export const toError = (value: unknown): Error =>
  value instanceof Error ? value : new Error(String(value));

/**
 * Runs `load` whenever the deps change and exposes loading, error and ready as
 * one state machine. Results from a superseded run are dropped.
 */
export const useAsync = <T>(load: () => Promise<T>, deps: readonly unknown[]): AsyncState<T> => {
  const [state, setState] = useState<AsyncState<T>>({ status: 'loading' });
  useEffect(() => {
    let cancelled = false;
    setState({ status: 'loading' });
    load().then(
      (data) => {
        if (!cancelled) {
          setState({ status: 'ready', data });
        }
      },
      (error: unknown) => {
        if (!cancelled) {
          setState({ status: 'error', error: toError(error) });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, deps);
  return state;
};
