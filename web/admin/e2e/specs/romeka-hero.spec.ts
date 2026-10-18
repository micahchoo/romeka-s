/**
 * The Romeka hero end to end: the picker inserts the upstream carrier, the
 * settings form edits it, a save stores the plain rendering, and the public
 * page renders the banner.
 */
import { expect, test } from '../fixtures';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

const HEADING = 'E2E hero heading';
const SUBHEADING = 'A banner from the Romeka block';

test('the hero block saves and the public page renders the banner', async ({ page }) => {
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  await page.getByRole('link', { name: 'Welcome' }).click();

  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  const picker = page.getByRole('dialog', { name: 'Choose a block layout' });
  await picker.getByRole('button', { name: 'Hero banner' }).click();

  // The card carries the Romeka block's label, not the carrier layout's.
  await expect(page.locator('.canvas-card').filter({ hasText: 'Hero banner' })).toHaveCount(1);

  // The shell selects the new block, so its settings form is the inspector.
  await page.getByRole('textbox', { name: 'Heading', exact: true }).fill(HEADING);
  await page
    .getByRole('textbox', { name: 'Subheading', exact: true })
    .fill(SUBHEADING);

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  // The reload reads the saved page back, carrier and generated html included.
  await page.reload();
  await expect(
    page.locator('.canvas-card').filter({ hasText: 'Hero banner' }),
  ).toHaveCount(1);

  const response = await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  expect(response?.status()).toBe(200);
  const hero = page.locator('.romeka-hero');
  await expect(hero).toHaveCount(1);
  await expect(hero.locator('.romeka-hero-heading')).toHaveText(HEADING);
  await expect(hero.locator('.romeka-hero-subheading')).toHaveText(SUBHEADING);
});
