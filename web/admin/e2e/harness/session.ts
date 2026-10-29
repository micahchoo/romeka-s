/**
 * The administrator's browser session every spec starts with.
 *
 * The admin shows a caller who is not signed in the sign-in panel and nothing
 * else, so a spec that drives a page needs a session the way a person does.
 * `globalSetup` signs in once through `POST /api/login` and writes the cookie
 * as a Playwright storage state, which `playwright.config.ts` hands to every
 * context.
 *
 * The cookie is scoped to `/api`. The proxy already keys every `/api` request
 * as the administrator, so the session changes only what `GET /api/me`
 * answers; the public site under `/s/` stays anonymous, as its specs expect.
 *
 * The proxy keeps this session out of every sign-in and sign-out, and out of
 * any request that carries a session of its own (`proxy.ts`), so a spec that
 * signs in as someone else acts as them and never ends the shared session. A
 * spec that needs the signed-out admin starts without it:
 * `test.use({ storageState: SIGNED_OUT })`.
 */
import { writeFileSync } from 'node:fs';

import type { APIRequestContext } from '@playwright/test';

import { sessionStateFile } from './state';

/** The session cookie the port sets (`omeka_api::session`). */
const COOKIE = 'romeka_session';

/** The storage state of a browser with no session. */
export const SIGNED_OUT = { cookies: [], origins: [] };

/** Sign the administrator in, write the storage state and answer the session token. */
export const writeAdminSession = async (baseUrl: string, email: string, password: string): Promise<string> => {
  const response = await fetch(`${baseUrl}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json', Origin: baseUrl },
    body: JSON.stringify({ email, password }),
  });
  if (!response.ok) {
    throw new Error(`the administrator sign-in answered ${String(response.status)}: ${await response.text()}`);
  }
  const token = response.headers
    .getSetCookie()
    .map((cookie) => cookie.split(';')[0] ?? '')
    .find((pair) => pair.startsWith(`${COOKIE}=`))
    ?.slice(COOKIE.length + 1);
  if (token === undefined || token === '') {
    throw new Error(`the administrator sign-in set no ${COOKIE} cookie`);
  }
  // The admin lessons open on a user's first visit to a page, and every spec
  // shares this user, so which lesson shows would depend on the specs run
  // before. They stay off here; `lessons.spec.ts` signs in its own users.
  const user: unknown = await response.json();
  const id = typeof user === 'object' && user !== null && 'o:id' in user ? String(user['o:id']) : '';
  const hidden = await fetch(`${baseUrl}/api/users/${id}`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
      Origin: baseUrl,
      Cookie: `${COOKIE}=${token}`,
    },
    body: JSON.stringify({ 'o:user_settings': { romeka_hide_lessons: true } }),
  });
  if (!hidden.ok) {
    throw new Error(`hiding the administrator's lessons answered ${String(hidden.status)}: ${await hidden.text()}`);
  }
  const state = {
    cookies: [
      {
        name: COOKIE,
        value: token,
        domain: new URL(baseUrl).hostname,
        path: '/api',
        expires: -1,
        httpOnly: true,
        secure: false,
        sameSite: 'Lax',
      },
    ],
    origins: [],
  };
  writeFileSync(sessionStateFile(), JSON.stringify(state, null, 2));
  return token;
};

/**
 * A language switch while signed in is also the user's `locale` setting, and
 * every spec shares this administrator, so a spec that switches unsets it
 * afterwards. `request` is the page's, so `GET /api/me` names the user.
 */
export const unsetAdminLocale = async (request: APIRequestContext): Promise<void> => {
  const me: unknown = await (await request.get('/api/me')).json();
  const id = typeof me === 'object' && me !== null && 'o:id' in me ? String(me['o:id']) : '';
  const reset = await request.patch(`/api/users/${id}`, { data: { 'o:user_settings': { locale: null } } });
  if (!reset.ok()) {
    throw new Error(`unsetting the administrator's locale answered ${String(reset.status())}: ${await reset.text()}`);
  }
};
