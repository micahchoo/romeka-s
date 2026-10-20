/**
 * The preview a tab shows: `POST .../preview` for one request, sent again a
 * moment after the request last changed, so typing a regex sends one request
 * and not one per key. The last answer stays on screen while the next loads.
 */
import { useEffect, useState } from 'react';
import { api } from '../../api/client';
import type { Preview, PreviewRequest } from '../../api/import-preview';
import { previewImport } from '../../api/imports';
import type { ImportId } from '../../api/imports';
import { toError } from '../../app/useAsync';

/** How long a request must stay unchanged before it is sent. */
export const PREVIEW_DELAY_MS = 250;

export interface PreviewState {
  readonly preview: Preview | null;
  /** The request `preview` answers, so a caller can tell a stale page from a fresh one. */
  readonly answers: string | null;
  /** That request itself, so a caller can read the page the answer is for. */
  readonly answered: PreviewRequest | null;
  readonly loading: boolean;
  readonly error: Error | null;
}

export const requestKey = (request: PreviewRequest): string => JSON.stringify(request);

export const usePreview = (id: ImportId, request: PreviewRequest | null): PreviewState => {
  const key = request === null ? null : requestKey(request);
  const [state, setState] = useState<PreviewState>({
    preview: null,
    answers: null,
    answered: null,
    loading: true,
    error: null,
  });
  useEffect(() => {
    if (request === null || key === null) {
      return undefined;
    }
    let cancelled = false;
    setState((previous) => ({ ...previous, loading: true }));
    const timer = window.setTimeout(() => {
      previewImport(api, id, request).then(
        (preview) => {
          if (!cancelled) {
            setState({ preview, answers: key, answered: request, loading: false, error: null });
          }
        },
        (error: unknown) => {
          if (!cancelled) {
            setState((previous) => ({ ...previous, loading: false, error: toError(error) }));
          }
        },
      );
    }, PREVIEW_DELAY_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // `key` is the request's value; the object itself is rebuilt every render.
  }, [id, key]);
  return state;
};
