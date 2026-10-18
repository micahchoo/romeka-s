/**
 * The site Theme page: a curator changes a token, sees it in the live preview,
 * saves it, and the public page renders it.
 *
 * The preview is the sandboxed iframe the page editor also renders, and the
 * public page is the real `/s/<slug>` route, so this spec covers the token
 * write, the preview override and the stored render end to end.
 *
 * It does not assert a clean console: the harness proxy serves the SPA's own
 * `dist/assets`, while the theme's `/assets/css/style.css` is a port route the
 * proxy does not forward, so the public page's stylesheet is a 404 here.
 */
import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

/** The default brand the Rust token schema publishes. */
const DEFAULT_BRAND = '#6aa9d6';
/** A light brand that clears AA against the default text colour. */
const NEW_BRAND = '#abcdef';

test('the theme page previews, saves and renders a brand colour', async ({ page }) => {
  // The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
  // not key, so the browser holds the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );

  await page.goto('/#/sites');
  await page.getByRole('row', { name: new RegExp(SITE_TITLE) }).getByRole('link').first().click();
  const match = /#(\/sites\/\d+)$/.exec(page.url());
  if (match?.[1] === undefined) {
    throw new Error(`no site detail hash in ${page.url()}`);
  }
  await page.goto(`/#${match[1]}/theme`);

  const brand = page.locator('#theme-setting-brand_color');
  await expect(brand).toHaveValue(DEFAULT_BRAND);

  // The live preview renders the unsaved token in a sandboxed iframe.
  await brand.fill(NEW_BRAND);
  const frame = page.frameLocator('iframe[title="Preview of the page"]');
  await expect
    .poll(
      () =>
        frame
          .locator('html')
          .evaluate((element) => getComputedStyle(element).getPropertyValue('--color-brand').trim()),
      { timeout: 10_000 },
    )
    .toBe(NEW_BRAND);

  // Saving the token, then reading it back off the public page, proves the
  // write reached storage and the render reads it.
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  const rendered = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--color-brand').trim(),
  );
  expect(rendered).toBe(NEW_BRAND);
});
