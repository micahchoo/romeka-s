/**
 * "Check stored files" end to end, through the real job runner: one seeded original is changed on
 * disk, the administrator starts a check from the Jobs screen, the job page states "1 differs.",
 * that link opens the Media list of that one file, and the dashboard line names the problem.
 * The file's bytes are put back afterwards, so later specs read the seeded original.
 * `FIXITY_SCREENSHOTS` names a folder for the pictures.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, record, stringField } from '../harness/json';
import { readHarness } from '../harness/state';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.FIXITY_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `fixity-${name}.png`), fullPage: true });
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

/** The stored path of the first media that has an original. */
const firstOriginal = async (page: Page): Promise<string> => {
  const response = await page.request.get('/api/media?sort_by=id&sort_order=asc&per_page=50');
  expect(response.status()).toBe(200);
  const media: unknown = await response.json();
  if (!isArray(media)) {
    throw new Error('/api/media did not answer a list');
  }
  for (const entry of media) {
    const row = record(entry, 'media');
    if (typeof row['o:filename'] === 'string' && row['o:original_url'] !== null) {
      return join(readHarness().storeRoot, 'original', stringField(row, 'o:filename', 'media'));
    }
  }
  throw new Error('no seeded media has an original');
};

test('a check finds the changed file, its count opens that file, and the dashboard names it', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  await signIn(page, baseURL);
  const path = await firstOriginal(page);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from('a changed byte')]));
  try {
    await page.goto('/#/jobs');
    const start = page.getByRole('button', { name: 'Check stored files' });
    await expect(start).toBeVisible();
    await shoot(page, 'jobs');
    await start.click();
    await expect(page).toHaveURL(/#\/jobs\/\d+$/);
    const differs = page.getByRole('link', { name: '1 differs.' });
    await expect(differs).toBeVisible({ timeout: 20_000 });
    await expect(page.getByRole('link', { name: /files? checked on/ })).toBeVisible();
    await expectClean(diagnostics, 'job page');
    await expectAxeBudget(page, 'fixity result');
    await shoot(page, 'result');

    await differs.click();
    await expect(page).toHaveURL(/#\/media\?fixity=differs&fixityJob=\d+$/);
    await expect(page.getByText(/^Files that differ in check #\d+\./)).toBeVisible();
    await expect(page.locator('tbody tr')).toHaveCount(1);
    await expectClean(diagnostics, 'media list');
    await shoot(page, 'media-list');

    await page.goto('/#/dashboard');
    await expect(page.getByRole('link', { name: /^Files last checked .+: 1 problem$/ })).toBeVisible();
    await expectClean(diagnostics, 'dashboard');
    await shoot(page, 'dashboard');
  } finally {
    writeFileSync(path, original);
  }
});
