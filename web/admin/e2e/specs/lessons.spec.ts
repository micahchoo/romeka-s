/**
 * The lessons against the real server: a lesson shows once, comes back on its
 * "Lesson" chip, stays folded after a reload and in a second browser, and the
 * "Show lessons" switch hides every one. The vocabulary lessons open at their
 * own moments (a merge prompt, a promotion preview, a draft publication, a
 * term's Links), one at a time.
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

test('a lesson shows once, comes back on its Lesson chip, and stays folded after a reload', async ({
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

test('the Show lessons switch hides every lesson and every Lesson chip', async ({
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

test('the merge and promote lessons open at their moments, one at a time', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  for (const subject of ['E2E lesson Kerry', 'E2E lesson Co. Kerry']) {
    await json(page.request, '/api/items', {
      'dcterms:title': [{ type: 'literal', property_id: 'auto', '@value': `Tagged ${subject}` }],
      'dcterms:subject': [{ type: 'literal', property_id: 'auto', '@value': subject }],
    });
  }
  const account = await newAccount(page.request, 'lessons-tags');
  await signIn(page, account, baseURL);

  await page.goto('/#/term-lists');
  const merge = page.getByRole('note', { name: 'Lesson: One spelling' });
  await expect(merge).toBeVisible();
  await expect(merge.getByRole('link', { name: 'Read about controlled vocabularies in SKOS' })).toBeVisible();
  await expectAxeBudget(page, 'Term lists with a lesson open');

  // The promotion preview comes while the merge lesson is open, so its lesson waits.
  const panel = page.getByRole('complementary', { name: 'Tags' });
  await panel.getByRole('checkbox', { name: /^E2E lesson Kerry / }).check();
  await panel.getByLabel('List name').fill('E2E lesson counties');
  await panel.getByRole('button', { name: 'Promote to list' }).click();
  const promote = page.getByRole('note', { name: 'Lesson: A controlled vocabulary' });
  await expect(page.getByRole('button', { name: 'Show the lesson: A controlled vocabulary' })).toBeVisible();
  await expect(promote).toBeHidden();
  await expect(page.getByRole('note', { name: /^Lesson:/u })).toHaveCount(1);

  await merge.getByRole('button', { name: 'Got it' }).click();
  await expect(merge).toBeHidden();
  await expect(promote).toBeVisible();
  await expect(promote).toContainText('Subject field');
  await expectClean(diagnostics, 'tags lessons');

  await page.reload();
  await expect(page.getByRole('button', { name: 'Show the lesson: One spelling' }).first()).toBeVisible();
  await expect(page.getByRole('note', { name: /^Lesson:/u })).toHaveCount(0);
});

test('the publish lesson opens on a draft and the links lesson waits for it', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const list = await json(page.request, '/api/custom_vocabs', {
    'o:label': 'E2E lesson towns',
    'o:lang': 'en',
    'o:terms': 'Tralee\nDingle',
  });
  const account = await newAccount(page.request, 'lessons-publish');
  await signIn(page, account, baseURL);

  await page.goto(`/#/term-lists/${String(member(list, 'o:id'))}`);
  const publish = page.getByRole('note', { name: 'Lesson: Addresses that last' });
  await expect(publish).toBeVisible();
  await expect(publish.getByRole('link', { name: 'Read about web addresses that do not change' })).toHaveAttribute(
    'href',
    'https://www.w3.org/Provider/Style/URI',
  );
  await expectAxeBudget(page, 'Publish panel with a lesson open');

  await page.getByLabel('Links of Tralee to an authority', { exact: true }).first().click();
  await expect(page.getByLabel('The same term elsewhere (exactMatch)').first()).toBeVisible();
  const links = page.getByRole('note', { name: 'Lesson: Links to an authority' });
  await expect(links).toBeHidden();

  await publish.getByRole('button', { name: 'Got it' }).click();
  await expect(links).toBeVisible();
  await expect(links).toContainText('skos:exactMatch');
  await links.getByRole('button', { name: 'Got it' }).click();
  await expectClean(diagnostics, 'publish lessons');

  // Once published, the list is no longer a draft, so the publish lesson has no moment.
  const region = page.getByRole('region', { name: 'Publish' });
  await region.getByLabel('Address name').fill('e2e-lesson-towns');
  await region.getByRole('button', { name: 'Publish', exact: true }).click();
  await expect(region.getByText('Published version 1.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Show the lesson: Addresses that last' })).toHaveCount(0);
});
