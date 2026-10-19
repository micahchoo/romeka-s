/**
 * The lessons against the real server: a lesson shows once, comes back on its
 * "?", stays folded after a reload and in a second browser, and the "Show
 * lessons" switch hides every one.
 *
 * Lessons belong to a signed-in user, whose settings hold what they have seen,
 * so each test makes its own user, gives it a password and signs in with it.
 * The harness's API key alone has no user to remember anything for.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';

const PASSWORD = 'e2e-lessons-password';

interface Account {
  readonly id: number;
  readonly email: string;
}

const json = async (request: APIRequestContext, path: string, data: unknown): Promise<unknown> => {
  const response = await request.post(path, { data });
  expect(response.ok(), `${path}: ${await response.text()}`).toBe(true);
  return response.status() === 204 ? null : response.json();
};

const member = (value: unknown, key: string): unknown =>
  typeof value === 'object' && value !== null && key in value
    ? Object.entries(value).find(([name]) => name === key)?.[1]
    : undefined;

/** A global admin with a password, made through the API as the harness key. */
const newAccount = async (request: APIRequestContext, name: string): Promise<Account> => {
  const email = `${name}-${String(Date.now())}@example.com`;
  const created = await json(request, '/api/users', {
    'o:name': name,
    'o:email': email,
    'o:role': 'global_admin',
    'o:is_active': true,
  });
  const id = member(created, 'o:id');
  if (typeof id !== 'number') {
    throw new Error('the user create answered no o:id');
  }
  const link = await json(request, `/api/users/${String(id)}/password-link`, {});
  await json(request, '/api/password', { key: member(link, 'key'), password: PASSWORD });
  return { id, email };
};

/** Sign the page's browser in, so the session cookie rides every request. */
const signIn = async (page: Page, account: Account, baseURL: string): Promise<void> => {
  const response = await page.request.post('/api/login', {
    data: { email: account.email, password: PASSWORD },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

test('a lesson shows once, comes back on its question mark, and stays folded after a reload', async ({
  page,
  browser,
  baseURL,
  diagnostics,
}) => {
  const account = await newAccount(page.request, 'lessons-once');
  await signIn(page, account, baseURL);

  await page.goto('/#/standards');
  const lesson = page.getByRole('note', { name: 'Lesson: Linked data' });
  await expect(lesson).toBeVisible();
  await expect(lesson).toContainText('http://purl.org/dc/terms/title');
  await expect(lesson.getByRole('link', { name: 'Read about linked data' })).toHaveAttribute(
    'href',
    'https://www.w3.org/DesignIssues/LinkedData.html',
  );

  await expectAxeBudget(page, 'Standards with a lesson open');

  await lesson.getByRole('button', { name: 'Got it' }).click();
  await expect(lesson).toBeHidden();
  const recall = page.getByRole('button', { name: 'Show the lesson: Linked data' });
  await expect(recall).toBeFocused();
  await recall.click();
  await expect(lesson).toBeVisible();
  await expectClean(diagnostics, 'first visit');

  await page.reload();
  await expect(recall).toBeVisible();
  await expect(lesson).toBeHidden();
  await expectClean(diagnostics, 'reload');

  // A second browser shares nothing with the first but the server.
  const elsewhere = await browser.newContext({ baseURL });
  const second = await elsewhere.newPage();
  await signIn(second, account, baseURL);
  await second.goto('/#/standards');
  await expect(second.getByRole('button', { name: 'Show the lesson: Linked data' })).toBeVisible();
  await expect(second.getByRole('note', { name: 'Lesson: Linked data' })).toBeHidden();
  await elsewhere.close();
});

test('the Show lessons switch hides every lesson and every question mark', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const account = await newAccount(page.request, 'lessons-off');
  await signIn(page, account, baseURL);

  await page.goto(`/#/users/${String(account.id)}`);
  const lessonsSwitch = page.getByLabel('Show lessons');
  await expect(lessonsSwitch).toBeChecked();
  await lessonsSwitch.uncheck();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  await page.goto('/#/standards');
  await expect(page.getByText('The shared vocabularies your fields')).toBeVisible();
  // The lessons wait for the session; once it is known, none may show.
  await expect(page.getByText(/^Signed in as/u)).toBeVisible();
  await expect(page.getByRole('note', { name: /^Lesson:/u })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /^Show the lesson/u })).toHaveCount(0);
  await expectClean(diagnostics, 'switched off');

  await page.reload();
  await expect(page.getByText('The shared vocabularies your fields')).toBeVisible();
  await expect(page.getByText(/^Signed in as/u)).toBeVisible();
  await expect(page.getByRole('button', { name: /^Show the lesson/u })).toHaveCount(0);

  // Back on, the lesson was never seen, so it opens.
  await page.goto(`/#/users/${String(account.id)}`);
  await page.getByLabel('Show lessons').check();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await page.goto('/#/standards');
  await expect(page.getByRole('note', { name: 'Lesson: Linked data' })).toBeVisible();
  await expectClean(diagnostics, 'switched back on');
});
