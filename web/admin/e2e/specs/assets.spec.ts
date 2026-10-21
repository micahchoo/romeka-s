/**
 * Site assets of any kind, end to end: a builder drops a PDF into an asset
 * block, describes it, and the public page links the file under that text. The
 * asset table takes an upload above its rows and names each file's kind.
 *
 * The PDF comes from `fixtures/media-kinds`, the same bytes the media matrix
 * uses, so the server's sniffing sees a real document.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { request as newRequest } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { REPO_ROOT, readHarness } from '../harness/state';

const SLUG = 'asset-picker';
const DESCRIPTION = 'The 1954 harbour plan (PDF)';

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

let site = 0;

const post = async (api: APIRequestContext, path: string, data: unknown): Promise<number> => {
  const response = await api.post(path, { data });
  expect(response.status(), await response.text()).toBe(200);
  return idOf(await response.json());
};

test.beforeAll(async () => {
  const api = await newRequest.newContext({ baseURL: readHarness().baseUrl });
  try {
    site = await post(api, '/api/sites', {
      'o:title': 'Asset picker',
      'o:slug': SLUG,
      'o:theme': 'default',
      'o:is_public': true,
    });
  } finally {
    await api.dispose();
  }
});

test.afterAll(async () => {
  const api = await newRequest.newContext({ baseURL: readHarness().baseUrl });
  try {
    await api.delete(`/api/sites/${String(site)}`);
  } finally {
    await api.dispose();
  }
});

test.beforeEach(async ({ page }) => {
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

const createPage = async (title: string, slug: string): Promise<number> => {
  const api = await newRequest.newContext({ baseURL: readHarness().baseUrl });
  try {
    return await post(api, '/api/site_pages', {
      'o:site': { 'o:id': site },
      'o:title': title,
      'o:slug': slug,
      'o:is_public': true,
      'o:block': [
        {
          'o:layout': 'asset',
          'o:data': [{ id: '', page: '', alt_link_title: '', caption: '' }],
        },
      ],
    });
  } finally {
    await api.dispose();
  }
};

const openAssetBlock = async (page: Page, pageId: number) => {
  await page.goto(`/#/sites/${String(site)}/pages/${String(pageId)}`);
  await page.getByRole('listitem', { name: /^Block 1: Asset/ }).click();
  const form = page.getByRole('region', { name: 'Block inspector' });
  await form.locator('.repeat-row').first().getByLabel('Upload').waitFor();
  return form;
};

test('a PDF dropped into an asset block is described and linked publicly', async ({ page, diagnostics }) => {
  const pageId = await createPage('Ship plan', 'ship-plan');
  const form = await openAssetBlock(page, pageId);
  const row = form.locator('.repeat-row').first();

  await row.getByLabel('Upload').setInputFiles({
    name: 'doc.pdf',
    mimeType: 'application/pdf',
    buffer: corpus('doc.pdf'),
  });
  const description = row.locator('.asset-describe textarea');
  await expect(description).toBeFocused();
  await description.fill(DESCRIPTION);
  await row.getByRole('button', { name: 'Save description' }).click();
  await expect(row.getByText('Saved.')).toBeVisible();

  // The picker names the file's kind rather than drawing it as a broken image.
  await row.getByRole('button', { name: 'Choose asset' }).click();
  const tile = row.locator('.asset-grid button').filter({ hasText: 'doc.pdf' });
  await expect(tile.locator('.media-image-kind')).toHaveText('PDF');
  await expect(tile.locator('img')).toHaveCount(0);

  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await expectClean(diagnostics, 'the asset block with a described PDF');

  diagnostics.reset();
  const response = await page.goto(`/s/${SLUG}/page/ship-plan`);
  expect(response?.status()).toBe(200);
  const link = page.getByRole('link', { name: DESCRIPTION });
  await expect(link).toHaveAttribute('href', /\/files\/asset\/.*\.pdf$/);
  await expect(link.locator('.asset-facts')).toContainText('PDF');
  await expectClean(diagnostics, 'the public asset block with a PDF');
});

test('the asset table uploads any file and names its kind', async ({ page, diagnostics }) => {
  await page.goto('/#/assets');
  await expect(page.locator('main table')).toBeVisible();

  await page.getByLabel('Upload').setInputFiles({
    name: 'harbour-plan.pdf',
    mimeType: 'application/pdf',
    buffer: corpus('doc.pdf'),
  });
  const description = page.locator('.asset-describe textarea');
  await expect(description).toBeFocused();
  await description.fill('The harbour plan, drawn in 1954');
  await page.getByRole('button', { name: 'Save description' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  const row = page.getByRole('row', { name: /harbour-plan\.pdf/ }).last();
  await expect(row).toContainText('PDF');
  await expectClean(diagnostics, 'the asset table with an upload');
});
