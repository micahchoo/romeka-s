/**
 * The dashboard's "Needs attention" list on the seeded data. An item with no set and no item type
 * is made, and an editor changes it, so every count is at least one for the administrator. Each
 * count opens the Items list whose total it is, under a chip that names the filter.
 *
 * Then a fixity check whose process died: its row stays `in_progress` with a pid no process has.
 * The dashboard reads it as running; "Check stored files" ends it as an error and starts a new
 * check. `DASHBOARD_SCREENSHOTS` names a folder for the pictures.
 */
import { DatabaseSync } from 'node:sqlite';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { ensureCli, runCli } from '../harness/cli';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.DASHBOARD_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `dashboard-${name}.png`), fullPage: true });
  }
};

const signIn = async (page: Page, baseURL: string, email: string, password: string): Promise<void> => {
  const response = await page.request.post('/api/login', {
    data: { email, password },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const titleProperty = async (page: Page): Promise<number> => {
  const response = await page.request.get('/api/properties?term=dcterms:title');
  const found: unknown = await response.json();
  if (!isArray(found) || found.length !== 1) {
    throw new Error('/api/properties?term=dcterms:title: expected one member');
  }
  return numberField(record(found[0], 'title'), 'o:id', 'title');
};

const titled = (property: number, text: string) => ({
  'dcterms:title': [{ type: 'literal', property_id: property, '@value': text }],
});

/** The number a "Needs attention" line states, after its colon. */
const countOf = async (page: Page, name: RegExp): Promise<number> => {
  const text = (await page.getByRole('listitem').filter({ hasText: name }).textContent()) ?? '';
  return Number(text.slice(text.lastIndexOf(':') + 1).replace(/[^\d]/g, ''));
};

/** The total the Items list states. */
const listedTotal = async (page: Page): Promise<number> => {
  const line = page.locator('.items-count');
  await expect(line).toHaveText(/^\d[\d,]* items? match/);
  return Number(((await line.textContent()) ?? '').replace(/[^\d]/g, ''));
};

test('each count opens the Items list whose total it is', async ({ page, baseURL, diagnostics, browser }) => {
  const harness = readHarness();
  const title = await titleProperty(page);
  const made = await page.request.post('/api/items', { data: { ...titled(title, 'Dashboard e2e loose letter'), 'o:is_public': true } });
  expect(made.status(), await made.text()).toBe(200);
  const id = numberField(record(await made.json(), 'item'), 'o:id', 'item');

  const editorEmail = `dashboard-editor-${String(Date.now())}@example.com`;
  runCli(
    ensureCli(),
    ['user', 'add', '-d', harness.dbPath, '--email', editorEmail, '--name', 'E2E Editor', '--password', 'e2e-password', '--role', 'editor'],
    harness.workDir,
  );
  const editor = await browser.newContext({ baseURL });
  try {
    const editorPage = await editor.newPage();
    await signIn(editorPage, baseURL, editorEmail, 'e2e-password');
    const changed = await editorPage.request.patch(`/api/items/${String(id)}`, {
      data: titled(title, 'Dashboard e2e loose letter, redated'),
      headers: { origin: new URL(baseURL).origin },
    });
    expect(changed.status(), await changed.text()).toBe(200);
  } finally {
    await editor.close();
  }

  await signIn(page, baseURL, harness.adminEmail, harness.adminPassword);
  await page.goto('/#/dashboard');
  const attention = page.getByRole('region', { name: 'Needs attention' });
  await expect(attention.getByRole('link', { name: /^Items in no item set: / })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'dashboard');
  await shoot(page, 'needs-attention');

  const lines = [
    { name: /^Items in no item set: /, url: /#\/items\?set=none$/, chip: 'In no item set' },
    { name: /^Items with no item type: /, url: /#\/items\?template=none$/, chip: 'No item type' },
    { name: /^Items others changed in the last 7 days: /, url: /#\/items\?changed=/, chip: /^Changed by others since / },
  ];
  for (const line of lines) {
    await page.goto('/#/dashboard');
    const link = attention.getByRole('link', { name: line.name });
    await expect(link).toBeVisible();
    const count = await countOf(page, line.name);
    expect(count).toBeGreaterThan(0);
    await link.click();
    await expect(page).toHaveURL(line.url);
    await expect(page.locator('.filter-chip').getByText(line.chip)).toBeVisible();
    expect(await listedTotal(page), String(line.name)).toBe(count);
    await expect(page.getByRole('link', { name: 'Dashboard e2e loose letter, redated' })).toBeVisible();
  }
  await shoot(page, 'changed-by-others');
  await expectClean(diagnostics, 'dashboard counts');
});

test('a check whose process died is ended, and a new check starts', async ({ page, baseURL, diagnostics }) => {
  const harness = readHarness();
  const database = new DatabaseSync(harness.dbPath);
  let stale: number;
  try {
    database.exec('PRAGMA busy_timeout = 5000');
    const started = new Date().toISOString().slice(0, 19).replace('T', ' ');
    // Past Linux's largest `pid_max` (2^22), so no process has it.
    const row = database
      .prepare(
        "INSERT INTO job (owner_id, pid, status, class, args, log, started, ended) VALUES (NULL, '4194305', 'in_progress', ?, '{}', NULL, ?, NULL)",
      )
      .run('Romeka\\Job\\FixityCheck', started);
    stale = Number(row.lastInsertRowid);
  } finally {
    database.close();
  }

  await signIn(page, baseURL, harness.adminEmail, harness.adminPassword);
  await page.goto('/#/dashboard');
  await expect(page.getByRole('link', { name: 'Checking stored files now.' })).toHaveAttribute('href', `#/jobs/${String(stale)}`);

  await page.goto('/#/jobs');
  await page.getByRole('button', { name: 'Check stored files' }).click();
  await expect(page).toHaveURL(/#\/jobs\/\d+$/);
  const opened = Number(new URL(page.url()).hash.split('/').pop());
  expect(opened).not.toBe(stale);
  await expect(page.getByRole('link', { name: /files? checked on/ })).toBeVisible({ timeout: 20_000 });

  const job = await page.request.get(`/api/jobs/${String(stale)}`);
  expect(record(await job.json(), 'job')['o:status']).toBe('error');
  await expectClean(diagnostics, 'new check');
  // The ended check has no result, which `GET /api/romeka/fixity/{job}` answers with a 404.
  await page.goto(`/#/jobs/${String(stale)}`);
  await expect(page.getByText("the job's process 4194305 ended before the job did")).toBeVisible();
  await shoot(page, 'stale-check-ended');
});
