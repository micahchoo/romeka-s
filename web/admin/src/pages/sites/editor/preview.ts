/**
 * The live preview request. The shell POSTs the page write it would save to
 * `POST /s/<site-slug>/preview`, and the public site answers the rendered page
 * as HTML or the API's error document.
 *
 * The route is the public site, not `/api`, so the API key rides in the query
 * the way the admin's `/api` writes send it, and the session cookie rides on
 * `credentials: 'same-origin'`.
 */
import { KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, currentKey } from '../../../api/auth';
import { defaultBaseUrl } from '../../../api/client';
import { parseApiErrors } from '../../../api/parse';
import { localizeServerErrors } from '../../../i18n/serverText';
import type { ApiErrors, PageId, SiteId } from '../../../api/types';
import { encodeSitePage } from '../../../api/write';
import type { WireBody } from '../../../api/write';
import type { BlockKey, PageContent } from './draft';
import { toPageWrite } from './draft';
import { canonicalBlocks } from './groups';

/** What one render request sends, and the key of every block it sends, in body order. */
export interface PreviewRequest {
  readonly body: WireBody;
  /** `order[n - 1]` is the key of the block the server stamps `data-romeka-block="n"`. */
  readonly order: readonly BlockKey[];
}

/** One server render and the order it was rendered from. Never split. */
export interface PreviewRender {
  readonly html: string;
  readonly order: readonly BlockKey[];
}

/**
 * The body is the write a `PUT /api/site_pages/<id>` takes, plus the id when
 * editing. The body and the order come from one canonical block list, so they
 * cannot disagree.
 */
export const previewRequest = (
  draft: PageContent,
  siteId: SiteId,
  pageId: PageId | null,
): PreviewRequest => {
  const blocks = canonicalBlocks(draft.blocks);
  return {
    body: {
      ...encodeSitePage(toPageWrite({ ...draft, blocks }, siteId)),
      ...(pageId === null ? {} : { 'o:id': pageId }),
    },
    order: blocks.map((block) => block.key),
  };
};

/** The `/s/<slug>/preview` URL, with the signed-in key attached the way `/api` writes attach it. */
export const previewUrl = (siteSlug: string): string => {
  const params = new URLSearchParams();
  const key = currentKey();
  if (key !== null) {
    params.set(KEY_IDENTITY_PARAM, key.identity);
    params.set(KEY_CREDENTIAL_PARAM, key.credential);
  }
  const query = params.toString();
  const path = `/s/${encodeURIComponent(siteSlug)}/preview`;
  return `${defaultBaseUrl()}${path}${query === '' ? '' : `?${query}`}`;
};

export type PreviewResult =
  | { readonly kind: 'html'; readonly html: string }
  /** The route answered 404, so the server has no preview yet. */
  | { readonly kind: 'unavailable' }
  /** The route refused the body; `errors` is the API's error document, when it sent one. */
  | { readonly kind: 'error'; readonly message: string; readonly errors: ApiErrors | null };

const parsedOrNull = (text: string): unknown => {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
};

const errorResult = async (response: Response): Promise<PreviewResult> => {
  const text = await response.text();
  const parsed = parseApiErrors(parsedOrNull(text));
  const errors = parsed === null ? null : await localizeServerErrors(parsed);
  for (const messages of Object.values(errors ?? {})) {
    const first = messages[0];
    if (first !== undefined) {
      return { kind: 'error', message: first, errors };
    }
  }
  const message = text === '' ? `Preview failed with ${String(response.status)}.` : text;
  return { kind: 'error', message, errors };
};

export const requestPreview = async (
  siteSlug: string,
  body: WireBody,
  signal?: AbortSignal,
): Promise<PreviewResult> => {
  const response = await fetch(previewUrl(siteSlug), {
    method: 'POST',
    ...(signal === undefined ? {} : { signal }),
    headers: { Accept: 'text/html', 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    credentials: 'same-origin',
  });
  if (response.status === 404) {
    return { kind: 'unavailable' };
  }
  if (!response.ok) {
    return errorResult(response);
  }
  return { kind: 'html', html: await response.text() };
};
