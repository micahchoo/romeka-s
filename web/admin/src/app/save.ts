import { useState } from 'react';
import { toError } from './useAsync';

export type SaveState =
  | { readonly status: 'idle' }
  | { readonly status: 'saving' }
  | { readonly status: 'saved' }
  | { readonly status: 'error'; readonly error: Error };

export interface Saver {
  readonly state: SaveState;
  readonly save: (action: () => Promise<void>) => void;
}

/**
 * The submit half of a form: idle until the first save, then saving, saved or
 * error from the promise the action returns.
 */
export const useSave = (): Saver => {
  const [state, setState] = useState<SaveState>({ status: 'idle' });
  const save = (action: () => Promise<void>): void => {
    setState({ status: 'saving' });
    action().then(
      () => {
        setState({ status: 'saved' });
      },
      (error: unknown) => {
        setState({ status: 'error', error: toError(error) });
      },
    );
  };
  return { state, save };
};
