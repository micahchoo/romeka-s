/**
 * Page history in a real browser against the real server: two saves become
 * versions, each saying what it changed, the first one renders through the preview route, Restore puts it in
 * the draft and the next Save stores it; and a save made from a page someone
 * else saved since is refused with their name, and Keep mine saves over theirs.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';
import { SITE_SLUG } from '../harness/seed';

const record = async (response: Awaited<ReturnType<APIRequestContext['get']>>): Promise<Record<string, unknown>> => {
  const body: unknown = await response.json();
  if (typeof body !== 'object' || body === null || Array.isArray(body)) {
    throw new Error(`expected a JSON object, got ${JSON.stringify(body)}`);
  }
  return Object.fromEntries(Object.entries(body));
};

const siteId = async (page: Page): Promise<number> => {
  const sites: unknown = await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json();
  const site: unknown = Array.isArray(sites) ? sites[0] : sites;
  if (typeof site === 'object' && site !== null && 'o:id' in site && typeof site['o:id'] === 'number') {
    return site['o:id'];
  }
  throw new Error(`no site ${SITE_SLUG}`);
};

/** A fresh page with one html block, so each test starts from its own history. */
const newPage = async (page: Page, slug: string): Promise<{ readonly site: number; readonly id: number }> => {
  const site = await siteId(page);
  const created = await record(
    await page.request.post('/api/site_pages', {
      data: {
        'o:site': { 'o:id': site },
        'o:title': 'Opening title',
        'o:slug': slug,
        'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Opening text.</p>' } }],
      },
    }),
  );
  const id = created['o:id'];
  if (typeof id !== 'number') {
    throw new Error('the page create answered no id');
  }
  return { site, id };
};

const storedTitle = async (page: Page, id: number): Promise<unknown> =>
  (await record(await page.request.get(`/api/site_pages/${String(id)}`)))['o:title'];

const saveTitle = async (page: Page, title: string): Promise<void> => {
  await page.getByLabel('Title', { exact: true }).fill(title);
  await page.getByRole('region', { name: 'Editor actions' }).getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
};

test.beforeEach(async ({ page }) => {
  // The preview POST is not keyed by the harness proxy, so the browser holds
  // the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

test('two saves are versions; the first one restores and saves again', async ({ page }) => {
  const { site, id } = await newPage(page, 'history-restore');
  await page.goto(`/#/sites/${String(site)}/pages/${String(id)}`);

  await saveTitle(page, 'First save');
  await saveTitle(page, 'Second save');

  const toolbar = page.getByRole('region', { name: 'Editor actions' });
  await toolbar.getByRole('button', { name: 'History' }).click();
  const panel = page.getByRole('region', { name: 'Page history' });
  const entries = panel.getByRole('button', { name: / · / });
  await expect(entries).toHaveCount(3);
  await expect(entries.first()).toContainText('E2E Admin · 1 block');
  await expect(entries.first()).toContainText('Title changed from “First save” to “Second save”');
  await expect(entries.nth(1)).toContainText('Title changed from “Opening title” to “First save”');
  await expect(entries.nth(2)).toContainText('Author not recorded · 1 block');
  await expect(entries.nth(2)).toContainText('First saved version');

  // The middle entry is the first save.
  const rendered = page.waitForResponse(
    (response) => response.url().includes(`/s/${SITE_SLUG}/preview`) && response.request().method() === 'POST',
  );
  await entries.nth(1).click();
  const render = await rendered;
  expect(render.status()).toBe(200);
  const body: unknown = render.request().postDataJSON();
  expect(body).toMatchObject({ 'o:id': id, 'o:title': 'First save' });
  await expect(page.getByText(/^Viewing the version from Today /)).toBeVisible();
  await expect(page.frameLocator('iframe.history-frame').getByText('Opening text.')).toBeVisible();
  expect(await storedTitle(page, id)).toBe('Second save');

  await page.getByRole('button', { name: 'Restore this version' }).click();
  await expect(page.getByText(/Viewing the version from/)).toHaveCount(0);
  await expect(page.getByText('Unsaved changes')).toBeVisible();
  await toolbar.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  expect(await storedTitle(page, id)).toBe('First save');
  await expect(entries).toHaveCount(4);
  await expect(entries.first()).toContainText('E2E Admin');
});

test('a save over a newer save names who saved, and Keep mine saves over it', async ({ page }) => {
  const { site, id } = await newPage(page, 'history-conflict');
  await page.goto(`/#/sites/${String(site)}/pages/${String(id)}`);
  await expect(page.getByLabel('Title', { exact: true })).toHaveValue('Opening title');

  // Another writer saves in a later second than this editor's load.
  await page.waitForTimeout(1100);
  const theirs = await page.request.put(`/api/site_pages/${String(id)}`, {
    data: {
      'o:site': { 'o:id': site },
      'o:title': 'Their title',
      'o:slug': 'history-conflict',
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Their text.</p>' } }],
    },
  });
  expect(theirs.status()).toBe(200);

  await page.getByLabel('Title', { exact: true }).fill('My title');
  await page.getByRole('region', { name: 'Editor actions' }).getByRole('button', { name: 'Save' }).click();

  const dialog = page.getByRole('alertdialog', { name: 'Someone else saved this page' });
  await expect(dialog).toContainText(/^Someone else saved this pageSaved by E2E Admin at \d{1,2}:\d{2}\s[AP]M since you opened this page/);
  expect(await storedTitle(page, id)).toBe('Their title');

  await dialog.getByRole('button', { name: 'Keep mine' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  expect(await storedTitle(page, id)).toBe('My title');
});
