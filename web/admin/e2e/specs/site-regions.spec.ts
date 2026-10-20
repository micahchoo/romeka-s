/**
 * The site header and footer in the page editor against the real server: the
 * preview marks them, a click in the preview selects the footer, an edit in
 * the site-wide inspector re-renders the preview, and its own Save reaches the
 * public page and the Theme page.
 */
import type { Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '../fixtures';
import { AXE_TAGS } from '../harness/axe';
import { readHarness } from '../harness/state';
import { SITE_SLUG } from '../harness/seed';

const COLUMN_TEXT = 'Reading room open Tuesday to Saturday.';

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const siteId = async (page: Page): Promise<number> => {
  const sites: unknown = await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json();
  return Array.isArray(sites) ? idOf(sites[0]) : idOf(sites);
};

/** The site's stored theme settings, so the spec can put them back for the specs after it. */
const storedTheme = async (page: Page, site: number): Promise<unknown> => {
  const body: unknown = await (await page.request.get(`/api/sites/${String(site)}`)).json();
  return typeof body === 'object' && body !== null && 'o:theme_settings' in body ? body['o:theme_settings'] : null;
};

const shown = (page: Page) => page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');

test.beforeEach(async ({ page }) => {
  // The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
  // not key, so the browser holds the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

test('a click on the footer in the preview edits it site-wide, and its save reaches every page', async ({
  page,
  diagnostics,
}) => {
  const site = await siteId(page);
  const before = await storedTheme(page, site);
  const created = await page.request.post('/api/site_pages', {
    data: {
      'o:site': { 'o:id': site },
      'o:title': 'Site regions',
      'o:slug': 'site-regions',
      'o:is_public': true,
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Body copy of the page.</p>' } }],
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  await page.goto(`/#/sites/${String(site)}/pages/${String(idOf(await created.json()))}`);

  try {
    await expect(shown(page).locator('footer[data-romeka-region="footer"]')).toHaveCount(1);
    await expect(shown(page).locator('header[data-romeka-region="header"]')).toHaveCount(1);

    await shown(page).locator('header .site-title').hover();
    await expect(page.locator('.live-outline--hover .live-label')).toHaveText('Site header');

    await shown(page).locator('footer .footer-title').click();
    const inspector = page.getByRole('region', { name: 'Block inspector' });
    await expect(inspector.getByText('Site-wide. Changes every page.')).toBeVisible();
    await expect(inspector.getByRole('navigation', { name: 'Selection' })).toHaveText('Page›Site footer');
    await expect(page.locator('.live-outline--selected .live-label')).toHaveText('Site footer');

    // The editor with a region selected meets the zero serious and critical
    // budget. Legacy mode and the frames excluded, as the live canvas spec
    // explains: the frames hold the public page, scanned on its own.
    await diagnostics.idle();
    const axe = await new AxeBuilder({ page })
      .withTags([...AXE_TAGS])
      .exclude('iframe')
      .setLegacyMode(true)
      .analyze();
    const faults = axe.violations.filter(
      (violation) => violation.impact === 'serious' || violation.impact === 'critical',
    );
    expect(faults.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(' ')).join(', ')}`)).toEqual([]);

    await inspector.getByRole('button', { name: 'Add a column' }).click();
    await inspector.getByRole('textbox', { name: 'Column 1' }).fill(COLUMN_TEXT);
    await expect(shown(page).locator('footer').getByText(COLUMN_TEXT)).toBeVisible({ timeout: 10_000 });

    await inspector.getByRole('button', { name: 'Save site-wide changes' }).click();
    await expect(inspector.getByText('Saved.')).toBeVisible();
    await expect(inspector.getByRole('button', { name: 'Save site-wide changes' })).toBeDisabled();

    await page.keyboard.press('Escape');
    await expect(inspector.getByLabel('Title')).toHaveValue('Site regions');

    await page.goto(`/s/${SITE_SLUG}/page/site-regions`);
    await expect(page.getByRole('contentinfo').getByText(COLUMN_TEXT)).toBeVisible();

    await page.goto(`/#/sites/${String(site)}/theme`);
    await expect(
      page.getByRole('group', { name: 'Footer' }).getByRole('textbox', { name: 'Column 1' }),
    ).toHaveText(COLUMN_TEXT);
  } finally {
    const restored = await page.request.patch(`/api/sites/${String(site)}`, {
      data: { 'o:theme_settings': before },
    });
    expect(restored.status(), await restored.text()).toBe(200);
  }
});
