/**
 * Finding against the real server. The header's jump-to box opens a record by its identifier,
 * by `#<id>`, and lists the two items that share a title. A saved list keeps the Items filters
 * and order under a name, shows under Items in the nav of a second browser, opens there, and is
 * renamed and deleted.
 *
 * Saved lists belong to a signed-in user, so each test makes its own user and signs in with it.
 * `FINDING_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';

const PASSWORD = 'e2e-finding-password';

interface Account {
  readonly email: string;
}

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.FINDING_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: join(folder, `finding-${name}.png`), fullPage: true });
  }
};

const post = async (request: APIRequestContext, path: string, data: unknown): Promise<unknown> => {
  const response = await request.post(path, { data });
  expect(response.ok(), `${path}: ${await response.text()}`).toBe(true);
  return response.status() === 204 ? null : response.json();
};

const idOf = (answer: unknown, label: string): number => numberField(record(answer, label), 'o:id', label);

/** A global admin with a password, made through the API as the harness key. */
const newAccount = async (request: APIRequestContext, name: string): Promise<Account> => {
  const email = `${name}-${String(Date.now())}@example.com`;
  const id = idOf(
    await post(request, '/api/users', { 'o:name': name, 'o:email': email, 'o:role': 'global_admin', 'o:is_active': true }),
    'user',
  );
  const link = record(await post(request, `/api/users/${String(id)}/password-link`, {}), 'link');
  await post(request, '/api/password', { key: link.key, password: PASSWORD });
  return { email };
};

const signIn = async (page: Page, account: Account, baseURL: string): Promise<void> => {
  const response = await page.request.post('/api/login', {
    data: { email: account.email, password: PASSWORD },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const propertyId = async (request: APIRequestContext, term: string): Promise<number> => {
  const answer: unknown = await (await request.get(`/api/properties?term=${term}`)).json();
  return idOf(isArray(answer) ? answer[0] : answer, term);
};

/** Items and a set to find, each title and identifier unique to this run. */
const seed = async (request: APIRequestContext, run: string) => {
  const [title, identifier] = [await propertyId(request, 'dcterms:title'), await propertyId(request, 'dcterms:identifier')];
  const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];
  const item = async (name: string, id: string | null): Promise<number> =>
    idOf(
      await post(request, '/api/items', {
        'dcterms:title': literal(title, name),
        ...(id === null ? {} : { 'dcterms:identifier': literal(identifier, id) }),
        'o:is_public': true,
      }),
      name,
    );
  return {
    ledger: await item(`Jump ${run} ledger`, `JE_${run}`),
    twins: [await item(`Jump ${run} twin letter`, null), await item(`Jump ${run} twin letter`, null)],
    set: idOf(await post(request, '/api/item_sets', { 'dcterms:title': literal(title, `Jump ${run} box`), 'o:is_public': true }), 'set'),
  };
};

test('the jump-to box opens an identifier, a #id, and lists two items of one title', async ({ page, baseURL, diagnostics }) => {
  const run = String(Date.now());
  const records = await seed(page.request, run);
  await signIn(page, await newAccount(page.request, 'finding-jump'), baseURL);
  await page.goto('/#/');
  const box = page.getByRole('searchbox', { name: 'Jump to a record' });
  await expect(box).toBeVisible();

  await page.keyboard.press('/');
  await expect(box).toBeFocused();
  await box.fill(`JE_${run}`);
  await box.press('Enter');
  await expect(page).toHaveURL(new RegExp(`#/items/${String(records.ledger)}$`));
  await expect(page.getByRole('heading', { name: `Jump ${run} ledger`, level: 2 })).toBeVisible();
  await expect(box).toHaveValue('');

  await box.fill(`#${String(records.set)}`);
  await box.press('Enter');
  await expect(page).toHaveURL(new RegExp(`#/items\\?set=${String(records.set)}$`));

  await box.fill(`Jump ${run} twin letter`);
  await box.press('Enter');
  const answer = page.getByRole('list', { name: `2 records match “Jump ${run} twin letter”` });
  await expect(answer.getByRole('link')).toHaveCount(2);
  await diagnostics.idle();
  await expectAxeBudget(page, 'jump-to answer with two matches');
  await shoot(page, 'jump-two-matches');
  await answer.getByRole('link').first().click();
  await expect(page).toHaveURL(new RegExp(`#/items/(${records.twins.map(String).join('|')})$`));
  await expect(answer).toBeHidden();

  await box.fill('zz-no-such-record');
  await box.press('Enter');
  await expect(page.getByText('No record that you can see matches “zz-no-such-record”.')).toBeVisible();
  await box.press('Escape');
  await expect(box).toHaveValue('');
  await expect(page.getByText(/No record that you can see/)).toBeHidden();
  await expectClean(diagnostics, 'jump-to');
});

test('a saved list follows the user to another browser, opens there, and is renamed and deleted', async ({
  page,
  browser,
  baseURL,
  diagnostics,
}) => {
  const run = String(Date.now());
  await seed(page.request, run);
  const account = await newAccount(page.request, 'finding-lists');
  await signIn(page, account, baseURL);
  await page.goto(`/#/items?q=${encodeURIComponent(`Jump ${run}`)}&sort=title`);
  const rows = page.locator('.items-results tbody tr');
  await expect(rows).toHaveCount(3);

  await page.getByRole('button', { name: 'Save this list' }).click();
  await page.getByRole('textbox', { name: 'Name' }).fill(`Letters ${run}`);
  await shoot(page, 'save-form');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText(`Saved list: Letters ${run}`)).toBeVisible();
  const nav = page.getByRole('navigation', { name: 'Admin pages' });
  await expect(nav.getByRole('list', { name: 'Saved lists' }).getByRole('link', { name: `Letters ${run}` })).toHaveAttribute(
    'aria-current',
    'true',
  );
  await expectClean(diagnostics, 'save');

  // A second browser shares nothing with the first but the server.
  const elsewhere = await browser.newContext({ baseURL });
  const second = await elsewhere.newPage();
  await signIn(second, account, baseURL);
  await second.goto('/#/');
  const secondNav = second.getByRole('navigation', { name: 'Admin pages' });
  await secondNav.getByRole('link', { name: `Letters ${run}` }).click();
  await expect(second).toHaveURL(/sort=title/);
  await expect(second.locator('.items-results tbody tr')).toHaveCount(3);
  await expect(second.locator('.items-results tbody tr').first()).toContainText(`Jump ${run} ledger`);
  await expect(second.getByText(`Saved list: Letters ${run}`)).toBeVisible();
  await expectAxeBudget(second, 'items on a saved list');
  await shoot(second, 'saved-list-open');

  await second.getByRole('button', { name: 'Rename saved list' }).click();
  await second.getByRole('textbox', { name: 'Name' }).fill(`Renamed ${run}`);
  await second.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(second.getByText(`Saved list: Renamed ${run}`)).toBeVisible();
  await expect(secondNav.getByRole('link', { name: `Renamed ${run}` })).toBeVisible();

  await second.getByRole('button', { name: 'Delete saved list' }).click();
  await expect(second.getByText(`Deleted the saved list Renamed ${run}.`)).toBeVisible();
  await expect(secondNav.getByRole('list', { name: 'Saved lists' })).toBeHidden();
  await expect(second.getByRole('button', { name: 'Save this list' })).toBeVisible();
  await elsewhere.close();
});
