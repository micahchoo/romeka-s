/**
 * Who the admin offers what, against the real port: your own account hides the
 * controls the server refuses you, the role select says what each role may do,
 * a user's page lists their site roles, and the nav offers each page by role.
 *
 * The users and the site role are written through the keyed `request` context,
 * the administrator's; a second browser signs in with a password as the other
 * user. `ACCESS_SCREENSHOTS` names a folder for the pictures.
 */
import type { APIRequestContext, Page } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { readHarness } from '../harness/state';

const PASSWORD = 'e2e-access-password';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.ACCESS_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: `${folder}/access-${name}.png`, fullPage: true });
  }
};

const json = async (response: Awaited<ReturnType<APIRequestContext['get']>>): Promise<unknown> => {
  expect(response.status(), await response.text()).toBeLessThan(300);
  return response.json();
};

const idOf = (document: unknown): number => {
  if (typeof document === 'object' && document !== null && 'o:id' in document && typeof document['o:id'] === 'number') {
    return document['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(document)}`);
};

const makeUser = async (request: APIRequestContext, name: string, role: string): Promise<{ id: number; email: string }> => {
  const email = `${name.toLowerCase().replaceAll(' ', '.')}@access.example.org`;
  const id = idOf(
    await json(
      await request.post('/api/users', {
        data: { 'o:name': name, 'o:email': email, 'o:role': role, 'o:is_active': true },
      }),
    ),
  );
  const link = await json(await request.post(`/api/users/${String(id)}/password-link`, { data: {} }));
  const key = typeof link === 'object' && link !== null && 'key' in link ? String(link.key) : '';
  const set = await request.post('/api/password', { data: { key, password: PASSWORD } });
  expect(set.status(), await set.text()).toBeLessThan(300);
  return { id, email };
};

test('your own account hides Role, Active and Delete and says why', async ({ page, diagnostics }) => {
  const me = idOf(await json(await page.request.get('/api/me')));
  await page.goto(`/#/users/${String(me)}`);

  await expect(page.getByText('Your role is Global Administrator.')).toBeVisible();
  await expect(
    page.getByText("You can't change your own role or deactivate yourself. Another administrator can."),
  ).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Role' })).toHaveCount(0);
  await expect(page.getByRole('checkbox', { name: 'Active' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Delete this user' })).toHaveCount(0);
  await shoot(page, 'own-account');

  // Saving your own account still works: the server sees the role and switch unchanged.
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]').filter({ hasText: 'Saved.' })).toBeVisible();
  await expectClean(diagnostics, 'own account');
});

test("another user's page says what the chosen role may do and lists their site roles", async ({
  page,
  request,
  diagnostics,
}) => {
  const user = await makeUser(request, 'Access Reader', 'researcher');
  const site = idOf(
    await json(
      await request.post('/api/sites', {
        data: { 'o:title': 'E2E access site', 'o:slug': 'e2e-access', 'o:theme': 'default', 'o:is_public': true },
      }),
    ),
  );
  await json(
    await request.patch(`/api/sites/${String(site)}`, {
      data: { 'o:site_permission': [{ 'o:user': { 'o:id': user.id }, 'o:role': 'editor' }] },
    }),
  );

  await page.goto(`/#/users/${String(user.id)}`);
  const role = page.getByRole('combobox', { name: 'Role' });
  await expect(role).toHaveAccessibleDescription('May read public records. May change only their own account.');
  await role.selectOption('editor');
  await expect(role).toHaveAccessibleDescription(
    'May add, change and delete every item, item set, media and item type, see private records and make sites.',
  );
  await expect(page.getByRole('checkbox', { name: 'Active' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Delete this user' })).toBeVisible();

  const sites = page.getByRole('region', { name: 'Sites' });
  await expect(sites.getByRole('row').nth(1)).toHaveText('E2E access siteCreator');
  await shoot(page, 'user-page');
  await sites.getByRole('link', { name: 'E2E access site' }).click();
  await expect(page).toHaveURL(new RegExp(`#/sites/${String(site)}/users$`, 'u'));
  await expectClean(diagnostics, 'user page');
});

test('the nav offers a researcher only the pages their role may use', async ({ browser, request }) => {
  const researcher = await makeUser(request, 'Access Nav Researcher', 'researcher');
  const context = await browser.newContext({ baseURL: readHarness().baseUrl, storageState: { cookies: [], origins: [] } });
  const page = await context.newPage();
  const answer = await page.request.post('/api/login', { data: { email: researcher.email, password: PASSWORD } });
  expect(answer.status(), await answer.text()).toBe(200);
  await page.goto('/#/items');
  const nav = page.getByRole('navigation', { name: 'Admin pages' });
  await expect(nav.getByRole('link', { name: 'Items' })).toBeVisible();
  for (const name of ['Import', 'Settings', 'Jobs', 'Modules']) {
    await expect(nav.getByRole('link', { name, exact: true })).toHaveCount(0);
  }
  await expect(nav.getByRole('link', { name: 'Users' })).toBeVisible();
  await shoot(page, 'researcher-nav');
  await context.close();
});
