/**
 * The Settings page's installation fields take effect where the admin has the
 * matching place: the title in the header and the page title, the page size
 * on a browse list, the time zone on every date, and a new item set starting
 * private. The full-text index box starts the index job and links to it.
 *
 * The run shares one database, so the spec puts the settings back as it found
 * them, whatever happens.
 */
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { numberField, record, stringField } from '../harness/json';

const FOUND = {
  installation_title: 'Romeka S',
  time_zone: 'UTC',
  pagination_per_page: 25,
  default_to_private_item_sets: false,
};

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.SETTINGS_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `settings-${name}.png`), fullPage: true });
  }
};

test('the installation settings save, and the header, a list, the dates and a new item set follow', async ({
  page,
  diagnostics,
}) => {
  // Three sets, so a page size of 2 needs a second page.
  for (const n of [1, 2, 3]) {
    const made = await page.request.post('/api/item_sets', {
      data: { 'dcterms:title': [{ type: 'literal', property_id: 1, '@value': `Page size set ${String(n)}` }] },
    });
    expect(made.status(), await made.text()).toBe(200);
  }
  try {
    await page.goto('/#/settings');
    await expect(page.getByText(/not persisted/)).toHaveCount(0);
    await expect(page.getByLabel('Tell me about new versions')).toHaveAccessibleDescription(
      'Stored only for now. Romeka does not check for new versions yet.',
    );
    await page.getByLabel('Installation title').fill('Gawan Museum');
    await page.getByLabel('Time zone').selectOption('Asia/Kolkata');
    await page.getByLabel('Results per page').fill('2');
    await page.getByLabel('Make new item sets private').check();
    await page.getByLabel('Rebuild the full-text search index').check();
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(page.getByText('Settings updated.')).toBeVisible();

    await expect(page.locator('.app-title')).toHaveText('Gawan Museum admin');
    await expect(page).toHaveTitle('Settings · Gawan Museum');
    const jobLink = page.getByRole('link', { name: 'Follow it on its job page.' });
    await expect(jobLink).toBeVisible();
    await expect(page.getByRole('status').filter({ hasText: /^The index rebuild started as job \d+\. Follow/ })).toBeVisible();
    const jobId = Number(((await jobLink.getAttribute('href')) ?? '').replace('#/jobs/', ''));
    expect(jobId).toBeGreaterThan(0);
    await expect(page.getByLabel('Rebuild the full-text search index')).not.toBeChecked();
    await shoot(page, 'saved');
    await expectClean(diagnostics, 'installation settings save');

    await page.reload();
    await expect(page.getByLabel('Installation title')).toHaveValue('Gawan Museum');
    await expect(page.getByLabel('Time zone')).toHaveValue('Asia/Kolkata');
    await expect(page.getByLabel('Results per page')).toHaveValue('2');
    await expect(page.getByLabel('Make new item sets private')).toBeChecked();
    await expect(page.locator('.app-title')).toHaveText('Gawan Museum admin');

    await page.goto('/#/item-sets');
    await expect(page.getByText(/^Page 1 of [2-9]\d* \(/)).toBeVisible();
    await expect(page.locator('table tbody tr')).toHaveCount(2);
    await expect(page).toHaveTitle('Item sets · Gawan Museum');

    const job = record(await (await page.request.get(`/api/jobs/${String(jobId)}`)).json(), 'job');
    expect(numberField(job, 'o:id', 'job')).toBe(jobId);
    const started = stringField(record(job['o:started'], 'o:started'), '@value', 'o:started');
    const [kolkata, utc] = await page.evaluate(
      (iso) =>
        ['Asia/Kolkata', 'UTC'].map((timeZone) =>
          new Intl.DateTimeFormat('en', { dateStyle: 'medium', timeStyle: 'short', timeZone }).format(new Date(iso)),
        ),
      started,
    );
    expect(kolkata).not.toBe(utc);
    await page.goto(`/#/jobs/${String(jobId)}`);
    await expect(page.getByText(kolkata ?? '').first()).toBeVisible();
    await expect(page.getByText(utc ?? '')).toHaveCount(0);

    await page.goto('/#/item-sets/new');
    await expect(page.getByLabel('Public', { exact: true })).not.toBeChecked();
    await shoot(page, 'new-item-set');
    await expectClean(diagnostics, 'installation settings applied');
  } finally {
    const restored = await page.request.patch('/api/settings', { data: FOUND });
    expect(restored.status(), await restored.text()).toBe(200);
  }
  // The admin reads the settings once per page load, so the restore shows after one.
  await page.goto('/#/item-sets/new');
  await page.reload();
  await expect(page.getByLabel('Public', { exact: true })).toBeChecked();
  await expect(page.locator('.app-title')).toHaveText('Romeka S admin');
});
