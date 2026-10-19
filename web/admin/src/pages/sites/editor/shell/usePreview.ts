/**
 * The live preview's request loop: one render request shortly after the last
 * draft change, the newest answer kept while the next one is in flight.
 */
import { useEffect, useRef, useState } from 'react';
import type { PageId, SiteId } from '../../../../api/types';
import { toError } from '../../../../app/useAsync';
import type { PageContent } from '../draft';
import { previewRequest, requestPreview } from '../preview';
import type { PreviewRender } from '../preview';

/** How long the loop waits after the last change before it asks for a render. */
export const PREVIEW_DELAY_MS = 400;

export type PreviewStatus = 'loading' | 'ready' | 'unavailable' | 'error';

export interface PreviewState {
  /** The newest render that arrived. Kept while the next one is in flight. */
  readonly render: PreviewRender | null;
  readonly status: PreviewStatus;
  readonly message: string | null;
}

const isAbort = (error: unknown): boolean =>
  error instanceof DOMException && error.name === 'AbortError';

/**
 * The first render is asked for at once; every later one 400 ms after the last
 * draft change. Sending a request aborts the one in flight, and each answer
 * carries its request's sequence number, so an answer that arrives after a
 * newer one never replaces it. A body equal to the last one sent is not sent
 * again (undo back to the same text, a no-op edit).
 */
export const usePreview = (
  draft: PageContent,
  siteSlug: string,
  siteId: SiteId,
  pageId: PageId | null,
): PreviewState => {
  const [state, setState] = useState<PreviewState>({
    render: null,
    status: 'loading',
    message: null,
  });
  const lastSent = useRef<string | null>(null);
  const sent = useRef(0);
  const shown = useRef(0);
  const inFlight = useRef<AbortController | null>(null);

  useEffect(() => {
    const { body, order } = previewRequest(draft, siteId, pageId);
    const text = JSON.stringify(body);
    if (text === lastSent.current) {
      return;
    }
    const timer = setTimeout(
      () => {
        lastSent.current = text;
        inFlight.current?.abort();
        const controller = new AbortController();
        inFlight.current = controller;
        sent.current += 1;
        const seq = sent.current;
        setState((current) => ({ ...current, status: 'loading' }));
        const settle = (next: (current: PreviewState) => PreviewState): void => {
          if (seq <= shown.current) {
            return;
          }
          shown.current = seq;
          setState(next);
        };
        requestPreview(siteSlug, body, controller.signal).then(
          (result) => {
            if (result.kind === 'html') {
              settle(() => ({ render: { html: result.html, order }, status: 'ready', message: null }));
            } else if (result.kind === 'unavailable') {
              settle((current) => ({ ...current, status: 'unavailable', message: null }));
            } else {
              settle((current) => ({ ...current, status: 'error', message: result.message }));
            }
          },
          (error: unknown) => {
            if (!isAbort(error)) {
              settle((current) => ({ ...current, status: 'error', message: toError(error).message }));
            }
          },
        );
      },
      lastSent.current === null ? 0 : PREVIEW_DELAY_MS,
    );
    return () => {
      clearTimeout(timer);
    };
  }, [draft, siteSlug, siteId, pageId]);

  useEffect(
    () => () => {
      inFlight.current?.abort();
    },
    [],
  );

  return state;
};
