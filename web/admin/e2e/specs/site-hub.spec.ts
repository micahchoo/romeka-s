/**
 * The site hub, its frame and the navigation tree, against the real server.
 *
 * Each test makes its own site through the Add site form, so the seeded site
 * the other specs read stays as seeded. A reload after each write proves the
 * write reached storage, and the public menu proves what the tree saved.
 */
import type { Locator, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';

/** Make a site through the Add site form; answers the hub hash, e.g. `/sites/4`. */
const createSite = async (page: Page, title: string, slug: string): Promise<string> => {
  await page.goto('/#/sites/new');
  await page.getByLabel('Title').fill(title);
  await expect(page.getByLabel('Slug')).toHaveValue(slug);
  await page.getByRole('button', { name: 'Add site' }).click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  const match = /#(\/sites\/\d+)$/.exec(page.url());
  if (match?.[1] === undefined) {
    throw new Error(`no site hub hash in ${page.url()}`);
  }
  return match[1];
};

const pageRow = (page: Page, title: string): Locator =>
  page.getByRole('table').getByRole('row', { name: new RegExp(title) });

const linkRow = (page: Page, label: string): Locator =>
  page.locator('.nav-tree-row', { has: page.getByRole('button', { name: `Move ${label}`, exact: true }) });

test('a new site lands on its hub, and the page actions and settings persist', async ({
  page,
  diagnostics,
}) => {
  const hub = await createSite(page, 'Hub Site One', 'hub-site-one');

  const crumbs = page.getByRole('navigation', { name: 'Breadcrumb' });
  await expect(crumbs.getByText('Hub Site One')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('heading', { level: 2, name: 'Hub Site One' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'View site' })).toHaveAttribute(
    'href',
    /\/s\/hub-site-one$/,
  );
  await expect(
    page.getByRole('navigation', { name: 'Site sections' }).getByRole('link', { name: 'Pages' }),
  ).toHaveAttribute('aria-current', 'page');

  // The welcome page the create builds is not in the default menu, so the site
  // has no homepage until it is added.
  const welcome = pageRow(page, 'Welcome');
  await expect(welcome).not.toContainText('Homepage');
  await welcome.getByRole('button', { name: 'Add to navigation' }).click();
  await expect(welcome).toContainText('In navigation');
  await expect(welcome).toContainText('Homepage');

  // A second page, through the editor; the frame names it once it exists.
  await page.getByRole('link', { name: 'Add page' }).click();
  await page.getByRole('button', { name: 'Start blank' }).click();
  await page.getByLabel('Title').fill('About');
  await page.getByRole('button', { name: 'Add page' }).click();
  await expect(page).toHaveURL(/#\/sites\/\d+\/pages\/\d+$/);
  await expect(crumbs.getByText('About')).toHaveAttribute('aria-current', 'page');
  await crumbs.getByRole('link', { name: 'Hub Site One' }).click();

  await pageRow(page, 'About').getByRole('button', { name: 'Set as homepage' }).click();
  await expect(pageRow(page, 'About')).toContainText('Homepage');
  await page.reload();
  await expect(pageRow(page, 'About')).toContainText('Homepage');
  await expect(pageRow(page, 'Welcome')).not.toContainText('Homepage');
  await expect(pageRow(page, 'Welcome')).toContainText('In navigation');

  // A settings save renames the site and keeps both pages.
  diagnostics.reset();
  await page.getByRole('link', { name: 'Site settings', exact: true }).click();
  await page.getByLabel('Summary').fill('A hub for the e2e run.');
  await page.getByLabel('Title', { exact: true }).fill('Hub Site Renamed');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await expect(crumbs.getByRole('link', { name: 'Hub Site Renamed' })).toBeVisible();
  await expectAxeBudget(page, 'admin site settings');
  await page.goto(`/#${hub}`);
  await expect(page.getByRole('table').getByRole('row')).toHaveCount(3);
  await expect(page.getByText('A hub for the e2e run.')).toBeVisible();
  await expectClean(diagnostics, 'site hub');
});

test('the navigation tree reorders by drag and by keyboard, and the public menu follows', async ({
  page,
  diagnostics,
}) => {
  const hub = await createSite(page, 'Nav Site Two', 'nav-site-two');
  await page.goto(`/#${hub}/navigation`);

  // A page link and a URL link join the default Browse link.
  await page.getByRole('button', { name: 'Add link' }).click();
  await page.getByLabel('Page', { exact: true }).selectOption({ label: 'Welcome' });
  await page.getByRole('button', { name: 'Add link' }).click();
  await page.getByLabel('Link type', { exact: true }).selectOption('url');
  await page.getByLabel('URL', { exact: true }).fill('https://example.org/docs');
  await page.getByLabel('Label', { exact: true }).fill('Docs');
  await expect(page.getByText('Unsaved changes')).toBeVisible();

  // Docs dropped on the middle of the Browse row nests under it.
  const browse = linkRow(page, 'Browse');
  const box = await browse.boundingBox();
  if (box === null) {
    throw new Error('the Browse row has no box');
  }
  await linkRow(page, 'Docs').dragTo(browse, {
    targetPosition: { x: box.width / 2, y: box.height / 2 },
  });
  const preview = page.getByRole('complementary', { name: 'Menu preview' });
  await expect(preview.locator('li li')).toHaveText(['Docs']);

  // Welcome moves to the front by keyboard: Space picks it up, Up moves it.
  const handle = page.getByRole('button', { name: 'Move Welcome', exact: true });
  await handle.focus();
  await page.keyboard.press('Space');
  await page.keyboard.press('ArrowUp');
  await expect(page.getByText('Welcome: position 1 of 2, level 1.')).toBeVisible();
  await page.keyboard.press('Space');
  await expect(preview.locator('.menu-preview-bar > ul > li > .menu-preview-link')).toHaveText([
    'Welcome',
    'Browse',
  ]);
  await expectAxeBudget(page, 'admin site navigation');

  diagnostics.reset();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await expect(page.getByText('Unsaved changes')).toHaveCount(0);
  await page.reload();
  await expect(page.locator('.nav-tree-label')).toHaveText(['Welcome', 'Browse', 'Docs']);
  const browseItem = page.locator('.nav-tree-item', { has: linkRow(page, 'Browse') });
  await expect(browseItem.locator('.nav-tree .nav-tree-label')).toHaveText(['Docs']);
  await expectClean(diagnostics, 'navigation save');

  // The public menu renders the saved order and nesting.
  await page.goto('/s/nav-site-two/page/welcome');
  const menu = page.locator('#top-nav');
  await expect(menu.locator(':scope > ul > li > a')).toHaveText(['Welcome', 'Browse']);
  await expect(menu.locator('li li > a')).toHaveText(['Docs']);
  await expect(menu.locator('li li > a')).toHaveAttribute('href', 'https://example.org/docs');
});

test('a dirty navigation tree asks before a tab click leaves it', async ({ page }) => {
  const hub = await createSite(page, 'Guard Site Three', 'guard-site-three');
  await page.goto(`/#${hub}/navigation`);
  await page.getByRole('button', { name: 'Add link' }).click();
  await expect(page.getByText('Unsaved changes')).toBeVisible();
  const dialogs: string[] = [];
  page.on('dialog', (dialog) => {
    dialogs.push(dialog.type());
    void dialog.dismiss();
  });
  await page
    .getByRole('navigation', { name: 'Site sections' })
    .getByRole('link', { name: 'Pages' })
    .click();
  await expect(page.getByText('Unsaved changes')).toBeVisible({ timeout: 2_000 });
  expect(dialogs).toEqual(['confirm']);
});
