/**
 * The site tabs as each site role sees them, signed in with a password in a
 * real browser: a Manager who does not own the site, a Creator and a Viewer.
 *
 * The harness proxy appends the administrator's key to every `/api` request,
 * and the server resolves a session cookie before the key, so a signed-in
 * browser acts as its user. The users, their passwords and their site roles
 * are written through the keyed `request` context, the administrator's.
 *
 * The run proves two things a stubbed test cannot: the admin reads the role the
 * server serves, and a Creator or a Viewer walking every tab sends no request
 * the server refuses.
 */
import type { APIRequestContext, Browser, Page } from '@playwright/test';

import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';

const PASSWORD = 'e2e-site-role-password';

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

/** A user with `role` and a password, made the way an administrator's invitation makes one. */
const makeUser = async (request: APIRequestContext, name: string, role: string): Promise<{ id: number; email: string }> => {
  const email = `${name.toLowerCase().replaceAll(' ', '.')}@roles.example.org`;
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

/**
 * The shell asks `GET /api/romeka/archive` to decide whether to show the Import
 * link, and a refusal is how it learns to hide the link for a non-administrator
 * (`src/app/shell.tsx`, `NavLink`). That answer belongs to the shell, not to a
 * site tab.
 */
const isShellProbe = (url: string): boolean => new URL(url).pathname === '/api/romeka/archive';

interface Signed {
  readonly page: Page;
  /** Every response at or above 400 the page saw, but the shell's Import probe. */
  readonly refused: string[];
  readonly close: () => Promise<void>;
}

const signIn = async (browser: Browser, email: string): Promise<Signed> => {
  const context = await browser.newContext({ baseURL: readHarness().baseUrl });
  const page = await context.newPage();
  const refused: string[] = [];
  page.on('response', (response) => {
    if (response.status() >= 400 && !isShellProbe(response.url())) {
      refused.push(`${String(response.status())} ${response.request().method()} ${response.url()}`);
    }
  });
  const answer = await page.request.post('/api/login', { data: { email, password: PASSWORD } });
  expect(answer.status(), await answer.text()).toBe(200);
  return { page, refused, close: () => context.close() };
};

const tabs = (page: Page): Promise<string[]> =>
  page.getByRole('navigation', { name: 'Site sections' }).getByRole('link').allTextContents();

test('each site role sees the controls it may use, and no tab sends a refused request', async ({
  browser,
  request,
}) => {
  test.setTimeout(90_000);
  const site = idOf(
    await json(
      await request.post('/api/sites', {
        data: { 'o:title': 'E2E roles site', 'o:slug': 'e2e-roles', 'o:theme': 'default', 'o:is_public': true },
      }),
    ),
  );
  const manager = await makeUser(request, 'Role Manager', 'researcher');
  const creator = await makeUser(request, 'Role Creator', 'author');
  const viewer = await makeUser(request, 'Role Viewer', 'editor');
  await json(
    await request.patch(`/api/sites/${String(site)}`, {
      data: {
        'o:site_permission': [
          { 'o:user': { 'o:id': manager.id }, 'o:role': 'admin' },
          { 'o:user': { 'o:id': creator.id }, 'o:role': 'editor' },
          { 'o:user': { 'o:id': viewer.id }, 'o:role': 'viewer' },
        ],
      },
    }),
  );
  const hub = `/#/sites/${String(site)}`;

  // A Creator: pages yes, everything that updates the site no.
  const asCreator = await signIn(browser, creator.email);
  const { page } = asCreator;
  await page.goto(hub);
  await expect(page.getByText('Only site managers can change the homepage and the menu.')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Add page' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Set as homepage' })).toHaveCount(0);
  expect(await tabs(page)).toEqual(['Pages', 'Navigation', 'Items']);
  await page.goto(`${hub}/navigation`);
  await expect(page.getByText('Only site managers can change the menu.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Add link' })).toHaveCount(0);
  await page.goto(`${hub}/resources`);
  await expect(page.getByText('Only site managers can add or remove items.')).toBeVisible();
  await expect(page.getByRole('switch')).toBeDisabled();
  for (const [tab, reason] of [
    ['users', 'Only site managers can change who works on this site.'],
    ['theme', 'Only site managers can change the theme.'],
    ['settings', 'Only site managers can change the site settings.'],
  ] as const) {
    await page.goto(`${hub}/${tab}`);
    await expect(page.getByText(reason)).toBeVisible();
  }
  // A Creator may still add a page, which is the write the hub offers.
  await page.goto(`${hub}/pages/new`);
  await expect(page.getByRole('heading', { name: 'Add page' })).toBeVisible();
  expect(asCreator.refused).toEqual([]);
  await asCreator.close();

  // A Viewer: reads the pages, adds and edits none.
  const asViewer = await signIn(browser, viewer.email);
  await asViewer.page.goto(hub);
  await expect(
    asViewer.page.getByText('You can view this site. Only its creators and managers can change it.'),
  ).toBeVisible();
  await expect(asViewer.page.getByRole('link', { name: 'Add page' })).toHaveCount(0);
  await expect(asViewer.page.getByRole('link', { name: 'Edit', exact: true })).toHaveCount(0);
  await asViewer.page.goto(`${hub}/resources`);
  await expect(asViewer.page.getByText(/Items on this site:/)).toBeVisible();
  expect(asViewer.refused).toEqual([]);
  await asViewer.close();

  // A Manager who does not own the site: every tab, and the reason in place of Delete.
  const asManager = await signIn(browser, manager.email);
  await asManager.page.goto(`${hub}/settings`);
  await expect(
    asManager.page.getByText('Only the site owner or an administrator can delete this site.'),
  ).toBeVisible();
  expect(await tabs(asManager.page)).toEqual([
    'Pages',
    'Navigation',
    'Items',
    'User permissions',
    'Theme',
    'Site settings',
  ]);
  await asManager.page.getByLabel('Name in the browser tab').fill('Managed by a manager');
  await asManager.page.getByRole('button', { name: 'Save' }).click();
  await expect(asManager.page.getByText('Saved.')).toBeVisible();
  expect(asManager.refused).toEqual([]);
  await asManager.close();
});
