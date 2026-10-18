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
import type { PageId, SiteId } from '../../../api/types';
import { encodeSitePage } from '../../../api/write';
import type { WireBody } from '../../../api/write';
import type { PageContent } from './draft';
import { toPageWrite } from './draft';

/** The body: the write a `PUT /api/site_pages/<id>` takes, plus the id when editing. */
export const previewBody = (
  draft: PageContent,
  siteId: SiteId,
  pageId: PageId | null,
): WireBody => ({
  ...encodeSitePage(toPageWrite(draft, siteId)),
  ...(pageId === null ? {} : { 'o:id': pageId }),
});

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
  | { readonly kind: 'error'; readonly message: string };

const parsedOrNull = (text: string): unknown => {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
};

const errorMessage = async (response: Response): Promise<string> => {
  const text = await response.text();
  const errors = parseApiErrors(parsedOrNull(text));
  for (const messages of Object.values(errors ?? {})) {
    const first = messages[0];
    if (first !== undefined) {
      return first;
    }
  }
  return text === '' ? `Preview failed with ${String(response.status)}.` : text;
};

export const requestPreview = async (
  siteSlug: string,
  body: WireBody,
): Promise<PreviewResult> => {
  const response = await fetch(previewUrl(siteSlug), {
    method: 'POST',
    headers: { Accept: 'text/html', 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    credentials: 'same-origin',
  });
  if (response.status === 404) {
    return { kind: 'unavailable' };
  }
  if (!response.ok) {
    return { kind: 'error', message: await errorMessage(response) };
  }
  return { kind: 'html', html: await response.text() };
};
