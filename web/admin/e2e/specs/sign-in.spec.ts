/**
 * The sign-in surfaces in a real browser: the password form the person uses, and
 * the API key behind its disclosure.
 *
 * Signed out, every admin address shows the sign-in panel and nothing else,
 * and signing in opens the page the address names. The harness proxy appends
 * the key to every `/api` request server-side, so these specs assert on the URL
 * the browser itself sent, and on what the person sees.
 *
 * The password spec drives the port's own account routes. Every spec here
 * starts with no session (`harness/session.ts`).
 */
import { expect, expectClean, test } from '../fixtures';
import { SIGNED_OUT } from '../harness/session';
import { readHarness } from '../harness/state';

test.use({ storageState: SIGNED_OUT });

const signInForm = (page: import('@playwright/test').Page) => page.getByRole('form', { name: 'Sign in' });
const nav = (page: import('@playwright/test').Page) => page.getByRole('navigation', { name: 'Admin pages' });

test('signed out, every admin address shows only the sign-in panel and asks the API for nothing else', async ({
  page,
  diagnostics,
}) => {
  const asked: string[] = [];
  page.on('request', (request) => {
    const { pathname } = new URL(request.url());
    if (pathname.startsWith('/api')) {
      asked.push(pathname);
    }
  });
  for (const address of ['/', '/#/items', '/#/items/1', '/#/users/1', '/#/settings']) {
    await page.goto(address);
    await expect(signInForm(page)).toBeVisible();
    await expect(page.getByText('Sign in to manage this collection.')).toBeVisible();
    await expect(nav(page)).toHaveCount(0);
    await expect(page.getByRole('button', { name: /Edit|Delete/u })).toHaveCount(0);
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
  await expectClean(diagnostics, 'signed-out addresses');
  // The installation title is the one answer a signed-out caller may read, for the panel's heading.
  expect([...new Set(asked)].sort()).toEqual(['/api/me', '/api/romeka/installation']);
  const folder = process.env.ACCESS_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: `${folder}/access-signed-out.png`, fullPage: true });
  }
});

test('signing in from an admin address opens that page', async ({ page, diagnostics }) => {
  const { adminName, adminEmail, adminPassword } = readHarness();
  await page.goto('/#/users');
  await signInForm(page).getByLabel('Email').fill(adminEmail);
  await signInForm(page).getByLabel('Password').fill(adminPassword);
  await signInForm(page).getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByText(`Signed in as ${adminName}`)).toBeVisible();
  await expect(page).toHaveURL(/#\/users$/u);
  await expect(page.getByRole('heading', { level: 1, name: 'Users' })).toBeVisible();
  await expect(nav(page).getByRole('link', { name: 'Users' })).toHaveAttribute('aria-current', 'page');
  await expectClean(diagnostics, 'sign in back to the address');
});

/** One character short of the 32 an API key half must be. */
const SHORT_IDENTITY = 'A'.repeat(31);

test('signing in from the header stores the key, survives a reload and reaches the API', async ({
  page,
  diagnostics,
}) => {
  const { keyIdentity, keyCredential } = readHarness();

  await page.goto('/#/items');
  await expect(signInForm(page)).toBeVisible();

  await page.getByText('Use an API key').click();
  const form = page.getByRole('form', { name: 'Sign in with an API key' });
  const formBox = await form.boundingBox();
  const mainBox = await page.locator('main.app-main').boundingBox();
  if (formBox === null || mainBox === null) {
    throw new Error('the page and its sign-in form must have a box');
  }
  expect(formBox.width).toBeLessThanOrEqual(mainBox.width + 1);
  await form.getByLabel('Key identity').fill(keyIdentity);
  await form.getByLabel('Key credential').fill(keyCredential);
  await form.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByText(`Signed in as ${keyIdentity}`)).toBeVisible();
  await expectClean(diagnostics, 'sign in');

  // The key lives in localStorage, so a reload keeps the session.
  await page.reload();
  await expect(page.getByText(`Signed in as ${keyIdentity}`)).toBeVisible();
  const stored = await page.evaluate(() => window.localStorage.getItem('romeka.apiKey'));
  expect(stored?.includes(keyIdentity)).toBe(true);
  await expectClean(diagnostics, 'reload signed in');

  // The browser, not the proxy, puts the key on the query string. The Users
  // page must render before the trip back, because a route whose chunk is still
  // loading leaves the items page hidden and reusable, and reopening it reuses
  // that tree instead of mounting a new list and asking again.
  await page.getByRole('link', { name: 'Users' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Users' })).toBeVisible();
  const signedInRequest = page.waitForRequest((request) => request.url().includes('/api/items'));
  await page.getByRole('link', { name: 'Items' }).click();
  const signedInParams = new URL((await signedInRequest).url()).searchParams;
  expect(signedInParams.get('key_identity')).toBe(keyIdentity);
  expect(signedInParams.get('key_credential')).toBe(keyCredential);
  await expectClean(diagnostics, 'signed-in request');

  // A key holder has no role the admin knows, so the nav offers only the pages
  // every role may use.
  await expect(nav(page).getByRole('link', { name: 'Settings' })).toHaveCount(0);

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(signInForm(page)).toBeVisible();
  await expect(nav(page)).toHaveCount(0);
  expect(await page.evaluate(() => window.localStorage.getItem('romeka.apiKey'))).toBeNull();
  await expectClean(diagnostics, 'sign out');
});

test('a malformed key is refused before any request', async ({ page, diagnostics }) => {
  const { keyCredential } = readHarness();
  const keyRequests: string[] = [];
  page.on('request', (request) => {
    if (request.url().includes('key_identity')) {
      keyRequests.push(request.url());
    }
  });

  await page.goto('/#/items');
  await page.getByText('Use an API key').click();
  const form = page.getByRole('form', { name: 'Sign in with an API key' });

  // A 31-character identity is the boundary the client rejects before the form
  // reaches the network.
  await form.getByLabel('Key identity').fill(SHORT_IDENTITY);
  await form.getByLabel('Key credential').fill(keyCredential);
  await form.getByRole('button', { name: 'Sign in' }).click();

  await expect(form.getByRole('alert')).toContainText('32');
  await expect(nav(page)).toHaveCount(0);
  await expectClean(diagnostics, 'malformed key');
  expect(keyRequests).toEqual([]);
});

test('the password form signs in, keeps the session across a reload and signs out', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const { adminName, adminEmail, adminPassword } = readHarness();

  await page.goto('/#/items');
  await expect(signInForm(page)).toBeVisible();
  await expectClean(diagnostics, 'signed out');

  const form = signInForm(page);
  await form.getByLabel('Email').fill(adminEmail);
  await form.getByLabel('Password').fill(adminPassword);
  const login = page.waitForRequest(
    (request) => new URL(request.url()).pathname === '/api/login',
  );
  await form.getByRole('button', { name: 'Sign in' }).click();
  const sent = await login;
  // The browser, not the app, sets the same-origin marker the server checks for
  // a cookie-authenticated write. `allHeaders` is the list that carries the
  // headers the browser generates, `headers` the ones a script set.
  const headers = await sent.allHeaders();
  expect(headers['origin']).toBe(baseURL);
  expect(sent.postDataJSON()).toEqual({ email: adminEmail, password: adminPassword });

  await expect(page.getByText(`Signed in as ${adminName}`)).toBeVisible();
  await expectClean(diagnostics, 'password sign in');

  // The session cookie is httpOnly, so nothing the page can read says it exists.
  const cookies = await page.evaluate(() => document.cookie);
  expect(cookies).not.toContain('romeka_session');

  await page.reload();
  await expect(page.getByText(`Signed in as ${adminName}`)).toBeVisible();
  await expectClean(diagnostics, 'reload with a session');

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(signInForm(page)).toBeVisible();
  await expectClean(diagnostics, 'sign out');

  // The server revoked the session, so the reload finds nobody signed in.
  await page.reload();
  await expect(signInForm(page)).toBeVisible();
  await expectClean(diagnostics, 'reload signed out');
});
