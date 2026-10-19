/**
 * The accessibility pass. axe runs once per page the suite reaches, and the
 * budget is zero serious and zero critical violations. The moderate and minor
 * counts print on every run so a regression is visible in the log.
 *
 * The admin pages come from the `AdminPageId` union, so a new page is covered
 * with no edit here. The detail and form surfaces, the sign-in form and the
 * public site pages are named one by one. Each check navigates the real app in
 * a real browser and scans the rendered document.
 */
import { expect, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { ASSET_SPARE_NAME, SITE_SLUG } from '../harness/seed';
import { adminPageIds } from '../harness/table';

for (const id of adminPageIds()) {
  test(`admin page #/${id} meets the axe budget`, async ({ page, diagnostics }) => {
    await page.goto(`/#/${id}`);
    await expect(page.locator('main h1')).toBeVisible();
    await diagnostics.idle();
    await expectAxeBudget(page, `admin #/${id}`);
  });
}

test('the admin detail and form surfaces meet the axe budget', async ({ page, diagnostics }) => {
  // The item show page and its edit panel, which is the app's largest form.
  await page.goto('/#/items');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin item show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.getByRole('textbox', { name: 'Title', exact: true })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin item edit');

  await page.goto('/#/items/new');
  await expect(page.getByRole('textbox', { name: 'Title', exact: true })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin item create');

  // The item set edit form.
  await page.goto('/#/item-sets');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/item-sets\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin item set edit');

  // The media show page and its edit panel.
  await page.goto('/#/media');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/media\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin media show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.locator('#media-alt-text')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin media edit');

  // The user edit form.
  await page.goto('/#/users');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/users\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin user edit');

  // The site hub.
  await page.goto('/#/sites');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin site hub');

  // The vocabulary family: a vocabulary with members (both Dublin Core
  // vocabularies are permanent and carry no edit control), a property, a
  // resource class, a resource template, and the create form of each.
  await page.goto('/#/vocabularies');
  await page.getByRole('link', { name: 'Bibliographic Ontology' }).click();
  await expect(page).toHaveURL(/#\/vocabularies\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin vocabulary show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.locator('#vocabulary-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin vocabulary edit');

  await page.goto('/#/vocabularies/new');
  await expect(page.locator('#vocabulary-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin vocabulary create');

  await page.goto('/#/properties');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/properties\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin property show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.locator('#member-properties-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin property edit');

  await page.goto('/#/properties/new');
  await expect(page.locator('#member-properties-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin property create');

  await page.goto('/#/resource-classes');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/resource-classes\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin resource class show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.locator('#member-resource_classes-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin resource class edit');

  await page.goto('/#/resource-classes/new');
  await expect(page.locator('#member-resource_classes-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin resource class create');

  await page.goto('/#/resource-templates');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/resource-templates\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin resource template show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.locator('#template-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin resource template edit');

  await page.goto('/#/resource-templates/new');
  await expect(page.locator('#template-label')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin resource template create');

  // The asset and the job detail views, on the spare rows the write spec does
  // not delete.
  await page.goto('/#/assets');
  await page.getByRole('link', { name: ASSET_SPARE_NAME }).click();
  await expect(page).toHaveURL(/#\/assets\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin asset show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.locator('#asset-name')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin asset edit');

  await page.goto('/#/jobs');
  await page.locator('tbody a').first().click();
  await expect(page).toHaveURL(/#\/jobs\/\d+$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin job show');

  await page.locator('.edit-toggle').first().click();
  await expect(page.getByRole('button', { name: 'Update', exact: true })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin job update');
});

test('the sign-in forms meet the axe budget', async ({ page, diagnostics }) => {
  await page.goto('/#/items');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.getByRole('form', { name: 'Sign in' })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin sign in');

  // The key form, which is folded away until it is asked for.
  await page.getByText('Use an API key').click();
  await expect(page.getByRole('form', { name: 'Sign in with an API key' })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin sign in with a key');
});

test('the set-password, forgot and invite surfaces meet the axe budget', async ({
  page,
  diagnostics,
}) => {
  // The set-password page, reached the way a mail link reaches it.
  await page.goto('/#/password');
  await expect(page.getByRole('heading', { name: 'Set a password' })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin password link without a key');

  await page.goto('/#/forgot');
  await expect(page.getByRole('heading', { name: 'Reset your password' })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin forgot password');

  // The invite form and the link panel it produces.
  await page.goto('/#/users/new');
  await page.getByLabel('Display name').fill('Axe Invited User');
  await page.getByLabel('Email').fill('axe-invited@example.com');
  await page.getByLabel('Role').selectOption('author');
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin user invite');

  await page.getByRole('button', { name: 'Invite this user' }).click();
  await expect(page.getByLabel('Password link for Axe Invited User')).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'admin password link panel');
});

test('the public site pages meet the axe budget', async ({ page }) => {
  // The site index, the welcome page the port builds on create, the item
  // browse, the page browse and the two search surfaces. Between them they are
  // the layout, the page view, the resource list and the search views.
  const index = await page.goto(`/s/${SITE_SLUG}`);
  expect(index?.status()).toBe(200);
  await expectAxeBudget(page, `public /s/${SITE_SLUG}`);

  const welcome = await page.goto(`/s/${SITE_SLUG}/page/welcome`);
  expect(welcome?.status()).toBe(200);
  await expect(page.getByText('Welcome to your new site.')).toBeVisible();
  await expectAxeBudget(page, 'public welcome page');

  const browse = await page.goto(`/s/${SITE_SLUG}/item`);
  expect(browse?.status()).toBe(200);
  await expectAxeBudget(page, 'public item browse');

  const pages = await page.goto(`/s/${SITE_SLUG}/page`);
  expect(pages?.status()).toBe(200);
  await expectAxeBudget(page, 'public page browse');

  const advanced = await page.goto(`/s/${SITE_SLUG}/item/search`);
  expect(advanced?.status()).toBe(200);
  await expectAxeBudget(page, 'public advanced search');

  const results = await page.goto(`/s/${SITE_SLUG}/index/search?fulltext_search=item`);
  expect(results?.status()).toBe(200);
  await expectAxeBudget(page, 'public search results');
});
