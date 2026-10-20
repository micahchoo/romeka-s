/**
 * "Make a copy": a new draft with the plan of an import, decisions kept
 * (`POST /api/romeka/imports { copyFrom }`), opened at once. The past imports
 * and the frame of a started import both offer it.
 */
import { useState } from 'react';
import type { ImportId } from '../../api/import-plan';
import { importApi } from '../../api/import-routes';
import { toError } from '../../app/useAsync';
import { firstPlace, placeHref } from './screens';

export type CopyState =
  | { readonly status: 'idle' }
  | { readonly status: 'copying'; readonly from: ImportId }
  | { readonly status: 'failed'; readonly from: ImportId; readonly error: Error };

export const useCopy = (): { readonly state: CopyState; readonly copy: (from: ImportId) => void } => {
  const [state, setState] = useState<CopyState>({ status: 'idle' });
  const copy = (from: ImportId): void => {
    setState({ status: 'copying', from });
    importApi.create({ copyFrom: from }).then(
      (created) => {
        window.location.hash = placeHref(created.id, firstPlace(created));
      },
      (error: unknown) => {
        setState({ status: 'failed', from, error: toError(error) });
      },
    );
  };
  return { state, copy };
};
