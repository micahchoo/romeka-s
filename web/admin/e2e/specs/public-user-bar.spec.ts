/**
 * The admin bar a signed-in curator sees on the public site: a styled strip
 * above the header with the way to the admin, the page's editor and the way
 * out, readable on every preset in the light and the dark scheme. Logout ends
 * the session and returns to the page; Log in returns to it too.
 *
 * The session comes from `POST /api/login`, the route the sign-in form uses,
 * so the public page sees a signed-in caller the way a browser does. The theme
 * writes go through the keyed `request` context, which holds no session: a
 * session-authenticated write must carry the base URL's origin.
 */
import type { APIRequestContext, Page } from '@playwright/test';

import { expect, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { SITE_SLUG, WELCOME_SLUG } from '../harness/seed';
import { SIGNED_OUT } from '../harness/session';
import { readHarness } from '../harness/state';

// The public site's own sign-in and logout are the subject, so no spec here
// starts with the administrator's session.
test.use({ storageState: SIGNED_OUT });

/** The bundled theme's presets (`crates/omeka-site/src/theme/presets.rs`). */
const PRESETS = ['exhibit', 'catalogue', 'gallery', 'journal', 'community'] as const;
const SCHEMES = ['light', 'dark'] as const;

const siteId = async (request: APIRequestContext): Promise<number> => {
  const sites: unknown = await (await request.get(`/api/sites?slug=${SITE_SLUG}`)).json();
  const site: unknown = Array.isArray(sites) ? sites[0] : null;
  if (typeof site === 'object' && site !== null && 'o:id' in site && typeof site['o:id'] === 'number') {
    return site['o:id'];
  }
  throw new Error(`no site ${SITE_SLUG} in ${JSON.stringify(sites)}`);
};

const storedThemeSettings = async (request: APIRequestContext, site: number): Promise<unknown> => {
  const document: unknown = await (await request.get(`/api/sites/${String(site)}`)).json();
  return typeof document === 'object' && document !== null && 'o:theme_settings' in document
    ? document['o:theme_settings']
    : null;
};

const storeThemeSettings = async (request: APIRequestContext, site: number, settings: unknown): Promise<void> => {
  const response = await request.patch(`/api/sites/${String(site)}`, {
    data: { 'o:theme_settings': settings },
  });
  expect(response.status(), await response.text()).toBe(200);
};

test('the signed-in user bar is styled and meets the axe budget on every preset and scheme', async ({
  page,
  request,
}) => {
  test.setTimeout(120_000);
  const { adminEmail, adminPassword, adminName } = readHarness();
  const signedIn = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
  });
  expect(signedIn.status(), await signedIn.text()).toBe(200);

  const site = await siteId(request);
  const original = await storedThemeSettings(request, site);
  try {
    for (const preset of PRESETS) {
      for (const scheme of SCHEMES) {
        await storeThemeSettings(request, site, { preset, color_scheme: scheme });
        await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
        const bar = page.getByRole('navigation', { name: 'Omeka User' });
        await expect(bar).toContainText(`Signed in as ${adminName}`);
        await expect(bar.getByRole('link', { name: 'Admin dashboard' })).toHaveAttribute('href', '/admin');
        await expect(bar.getByRole('link', { name: 'Edit this page' })).toBeVisible();
        await expect(bar.getByRole('button', { name: 'Logout' })).toBeVisible();
        // The sheet styles the bar: a surface fill, not the page's bare text.
        const fill = await bar.evaluate((element) => getComputedStyle(element).backgroundColor);
        const pageFill = await page.locator('body').evaluate((element) => getComputedStyle(element).backgroundColor);
        expect(fill, `${preset} ${scheme}`).not.toBe('rgba(0, 0, 0, 0)');
        expect(fill, `${preset} ${scheme}`).not.toBe(pageFill);
        await expectAxeBudget(page, `public user bar, ${preset} ${scheme}`);
      }
    }
  } finally {
    await storeThemeSettings(request, site, original);
  }
});

const signIn = async (page: Page): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const signedIn = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
  });
  expect(signedIn.status(), await signedIn.text()).toBe(200);
};

const WELCOME_PATH = `/s/${SITE_SLUG}/page/${WELCOME_SLUG}`;

test('Logout ends the session and returns to the page', async ({ page }) => {
  await signIn(page);
  await page.goto(WELCOME_PATH);
  const bar = page.getByRole('navigation', { name: 'Omeka User' });
  await bar.getByRole('button', { name: 'Logout' }).click();

  await expect(page).toHaveURL(WELCOME_PATH);
  await expect(page.getByRole('navigation', { name: 'Omeka User' })).toHaveCount(0);
  expect((await page.request.get('/api/me')).status()).toBe(401);
});

test('Edit this page opens the editor on that page', async ({ page }) => {
  await signIn(page);
  await page.goto(WELCOME_PATH);
  const bar = page.getByRole('navigation', { name: 'Omeka User' });
  await bar.getByRole('link', { name: 'Edit this page' }).click();

  await expect(page).toHaveURL(/\/#\/sites\/\d+\/pages\/\d+$/);
  await expect(page.getByRole('button', { name: /Insert block at position/ }).first()).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Title', exact: true })).toHaveValue('Welcome');
});

test('Log in on the public site returns the reader to the page', async ({ page, request }) => {
  const site = await siteId(request);
  const settings = async (value: unknown): Promise<void> => {
    const response = await request.patch(`/api/sites/${String(site)}`, {
      data: { 'o:site_settings': { show_user_bar: value } },
    });
    expect(response.status(), await response.text()).toBe(200);
  };
  await settings(1);
  try {
    await page.goto(WELCOME_PATH);
    await page.getByRole('navigation', { name: 'Omeka User' }).getByRole('link', { name: 'Log in' }).click();
    await expect(page).toHaveURL(`/login?redirect=${encodeURIComponent(WELCOME_PATH)}`);

    const { adminEmail, adminPassword, adminName } = readHarness();
    const form = page.getByRole('form', { name: 'Sign in' });
    await form.getByLabel('Email').fill(adminEmail);
    await form.getByLabel('Password').fill(adminPassword);
    await form.getByRole('button', { name: 'Sign in' }).click();

    await expect(page).toHaveURL(WELCOME_PATH);
    await expect(page.getByRole('navigation', { name: 'Omeka User' })).toContainText(`Signed in as ${adminName}`);
  } finally {
    await settings(null);
  }
});

test('Logout from a private page returns the reader to the site home, not a 404', async ({ page, request }) => {
  const site = await siteId(request);
  const slug = `staff-notes-${String(Date.now())}`;
  const created = await request.post('/api/site_pages', {
    data: {
      'o:site': { 'o:id': site },
      'o:title': 'Staff notes',
      'o:slug': slug,
      'o:is_public': false,
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>For staff only.</p>' } }],
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const document: unknown = await created.json();
  const pageId = typeof document === 'object' && document !== null && 'o:id' in document ? document['o:id'] : null;
  try {
    await signIn(page);
    await page.goto(`/s/${SITE_SLUG}/page/${slug}`);
    await expect(page.getByText('For staff only.')).toBeVisible();
    await page.getByRole('navigation', { name: 'Omeka User' }).getByRole('button', { name: 'Logout' }).click();

    // The site root answers with the first page its menu links.
    await expect(page).toHaveURL(new RegExp(`/s/${SITE_SLUG}(/page/${WELCOME_SLUG})?$`));
    await expect(page.getByText('This is the first page of your site.')).toBeVisible();
    await expect(page.getByRole('navigation', { name: 'Omeka User' })).toHaveCount(0);
  } finally {
    if (typeof pageId === 'number') {
      await request.delete(`/api/site_pages/${String(pageId)}`);
    }
  }
});
