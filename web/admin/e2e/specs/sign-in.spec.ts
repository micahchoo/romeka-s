/**
 * The sign-in surfaces in a real browser: the password form the person uses, and
 * the API key behind its disclosure.
 *
 * The harness proxy appends the key to every `/api` request server-side, so a
 * signed-out page still reads and writes. These specs therefore assert on the
 * URL the browser itself sent, and on the header state a user sees.
 *
 * The password spec drives the port's own account routes.
 */
import { expect, expectClean, test } from '../fixtures';
import { readHarness } from '../harness/state';

/** One character short of the 32 an API key half must be. */
const SHORT_IDENTITY = 'A'.repeat(31);

test('signing in from the header stores the key, survives a reload and reaches the API', async ({
  page,
  diagnostics,
}) => {
  const { keyIdentity, keyCredential } = readHarness();

  await page.goto('/#/items');
  await expect(page.getByText('Signed out')).toBeVisible();

  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.getByText('Use an API key').click();
  const form = page.getByRole('form', { name: 'Sign in with an API key' });
  const formBox = await form.boundingBox();
  const headerBox = await page.locator('header.app-header').boundingBox();
  if (formBox === null || headerBox === null) {
    throw new Error('the header and its sign-in form must have a box');
  }
  expect(formBox.width).toBeLessThanOrEqual(headerBox.width + 1);
  await form.getByLabel('Key identity').fill(keyIdentity);
  await form.getByLabel('Key credential').fill(keyCredential);
  await form.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByText(`Signed in as ${keyIdentity}`)).toBeVisible();
  await expectClean(diagnostics, 'sign in');

  // The key lives in localStorage, so a reload keeps the session.
  await page.reload();
  await expect(page.getByText(`Signed in as ${keyIdentity}`)).toBeVisible();
  const stored = await page.evaluate(() => window.localStorage.getItem('omeka.apiKey'));
  expect(stored?.includes(keyIdentity)).toBe(true);
  await expectClean(diagnostics, 'reload signed in');

  // The browser, not the proxy, puts the key on the query string. The Settings
  // page must render before the trip back, because a route whose chunk is still
  // loading leaves the items page hidden and reusable, and reopening it reuses
  // that tree instead of mounting a new list and asking again.
  await page.getByRole('link', { name: 'Settings' }).click();
  await expect(page.locator('#setting-installation_title')).toBeVisible();
  const signedInRequest = page.waitForRequest((request) => request.url().includes('/api/items'));
  await page.getByRole('link', { name: 'Items' }).click();
  const signedInParams = new URL((await signedInRequest).url()).searchParams;
  expect(signedInParams.get('key_identity')).toBe(keyIdentity);
  expect(signedInParams.get('key_credential')).toBe(keyCredential);
  await expectClean(diagnostics, 'signed-in request');

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page.getByText('Signed out')).toBeVisible();
  expect(await page.evaluate(() => window.localStorage.getItem('omeka.apiKey'))).toBeNull();
  await expectClean(diagnostics, 'sign out');

  // Signed out, the same request carries neither parameter.
  await page.getByRole('link', { name: 'Settings' }).click();
  await expect(page.locator('#setting-installation_title')).toBeVisible();
  const signedOutRequest = page.waitForRequest((request) => request.url().includes('/api/items'));
  await page.getByRole('link', { name: 'Items' }).click();
  const signedOutParams = new URL((await signedOutRequest).url()).searchParams;
  expect(signedOutParams.get('key_identity')).toBeNull();
  expect(signedOutParams.get('key_credential')).toBeNull();
  await expectClean(diagnostics, 'signed-out request');
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
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.getByText('Use an API key').click();
  const form = page.getByRole('form', { name: 'Sign in with an API key' });

  // A 31-character identity is the boundary the client rejects before the form
  // reaches the network.
  await form.getByLabel('Key identity').fill(SHORT_IDENTITY);
  await form.getByLabel('Key credential').fill(keyCredential);
  await form.getByRole('button', { name: 'Sign in' }).click();

  await expect(form.getByRole('alert')).toContainText('32');
  await expect(page.getByText('Signed out')).toBeVisible();
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
  await expect(page.getByText('Signed out')).toBeVisible();
  await expectClean(diagnostics, 'signed out');

  await page.getByRole('button', { name: 'Sign in' }).click();
  const form = page.getByRole('form', { name: 'Sign in' });
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
  expect(cookies).not.toContain('omeka_session');

  await page.reload();
  await expect(page.getByText(`Signed in as ${adminName}`)).toBeVisible();
  await expectClean(diagnostics, 'reload with a session');

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page.getByText('Signed out')).toBeVisible();
  await expectClean(diagnostics, 'sign out');

  // The server revoked the session, so the reload finds nobody signed in.
  await page.reload();
  await expect(page.getByText('Signed out')).toBeVisible();
  await expectClean(diagnostics, 'reload signed out');
});
