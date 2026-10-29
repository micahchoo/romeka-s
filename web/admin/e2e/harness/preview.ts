/**
 * A create answers before its media's preview exists: the derivative worker
 * makes it afterwards. A picture takes notes on a region only once that
 * preview is ready, because until then the server has no canvas to draw on.
 */
import type { APIRequestContext } from '@playwright/test';

import { record } from './json';

/** Wait until the derivative worker has settled a media's preview. */
export const settled = async (request: APIRequestContext, media: number): Promise<void> => {
  const deadline = Date.now() + 30_000;
  for (;;) {
    const document: unknown = await (await request.get(`/api/media/${String(media)}`)).json();
    const preview = record(document, 'media')['o-module-romeka:preview'];
    const state =
      typeof preview === 'object' && preview !== null && 'state' in preview ? preview.state : null;
    if (state === null || state === 'ready' || state === 'none') {
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(`media ${String(media)} preview stayed ${JSON.stringify(state)}`);
    }
    await new Promise((done) => setTimeout(done, 100));
  }
};
