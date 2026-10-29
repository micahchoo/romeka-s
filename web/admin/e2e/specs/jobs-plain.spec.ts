/**
 * Background work in plain words, against the real port: a job reads by its name in the list and
 * on its page, its page names the owner and folds the arguments, and Past imports offers Discard
 * on an import that added nothing, which removes it. `JOBS_SCREENSHOTS` names a folder for the
 * pictures.
 */
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record, stringField } from '../harness/json';
import { JOB_SPARE_CLASS } from '../harness/seed';
import { readHarness } from '../harness/state';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.JOBS_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `jobs-${name}.png`), fullPage: true });
  }
};

const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

/** The seeded job of `JOB_SPARE_CLASS`, which no other spec deletes. */
const spareJob = async (page: Page): Promise<number> => {
  const response = await page.request.get('/api/jobs?per_page=100');
  expect(response.status()).toBe(200);
  const jobs: unknown = await response.json();
  if (!isArray(jobs)) {
    throw new Error('/api/jobs did not answer a list');
  }
  for (const entry of jobs) {
    const job = record(entry, 'job');
    if (stringField(job, 'o:job_class', 'job') === JOB_SPARE_CLASS) {
      return numberField(job, 'o:id', 'job');
    }
  }
  throw new Error(`no seeded job of ${JOB_SPARE_CLASS}`);
};

test('a job reads by its name and its owner, with its arguments folded', async ({ page, baseURL, diagnostics }) => {
  await signIn(page, baseURL);
  const id = await spareJob(page);

  await page.goto('/#/jobs');
  const row = page.locator('tbody tr', { has: page.getByRole('link', { name: String(id), exact: true }) });
  await expect(row.getByRole('cell', { name: 'Batch edit', exact: true })).toBeVisible();
  await expect(page.getByText(JOB_SPARE_CLASS)).toHaveCount(0);
  await expectClean(diagnostics, 'jobs list');
  await shoot(page, 'list');

  await row.getByRole('link', { name: String(id), exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Batch edit', level: 2 })).toBeVisible();
  await expect(page.getByText(JOB_SPARE_CLASS)).toBeVisible();
  await expect(page.getByRole('link', { name: readHarness().adminName, exact: true })).toHaveAttribute(
    'href',
    /^#\/users\/\d+$/,
  );
  const fold = page.locator('details.job-args');
  await expect(fold).not.toHaveAttribute('open');
  await expect(fold.locator('pre')).toBeHidden();
  await expectClean(diagnostics, 'job page');
  await expectAxeBudget(page, 'job page');
  await shoot(page, 'detail');

  await fold.getByText('Arguments', { exact: true }).click();
  await expect(fold.locator('pre')).toContainText('"action": "index"');
  await shoot(page, 'detail-arguments');
});

test('Past imports discards an import that added nothing', async ({ page, baseURL, diagnostics }) => {
  await signIn(page, baseURL);
  const made = await page.request.post('/api/romeka/imports', {
    data: { sources: [{ kind: 'staging', id: 'up' }] },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(made.status(), await made.text()).toBe(201);
  const id = numberField(record(await made.json(), 'import'), 'id', 'import');

  await page.goto('/#/import');
  const row = page.locator('tbody tr', { has: page.locator(`a[href="#/import/${String(id)}"]`) });
  await expect(row.getByRole('cell', { name: 'Not added yet', exact: true })).toBeVisible();
  await row.getByRole('button', { name: 'Discard', exact: true }).click();
  const confirm = row.getByRole('group', { name: 'Discard' });
  await expect(confirm).toContainText('Nothing was added from this import.');
  await expectClean(diagnostics, 'discard confirm');
  await shoot(page, 'discard-confirm');

  await confirm.getByRole('button', { name: 'Discard this import' }).click();
  await expect(row).toHaveCount(0);
  const gone = await page.request.get(`/api/romeka/imports/${String(id)}`);
  expect(gone.status()).toBe(404);
  await expectClean(diagnostics, 'discarded');
  await shoot(page, 'discarded');
});
