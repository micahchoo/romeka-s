/**
 * A built-in FOAF label, as `romeka install` stores it, in Spanish on the
 * admin and on a Spanish public site. The translation is keyed by term and
 * applies only while the stored label is still the built-in English, so this
 * fails when the shipped English is not what the install stores.
 *
 * The spec makes its own Spanish site and item, so the seeded site the other
 * specs read stays as seeded.
 */
import type { APIRequestContext } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { isArray, numberField, record, stringField } from '../harness/json';
import { TITLE_TERM } from '../harness/seed';
import { unsetAdminLocale } from '../harness/session';

const SLUG = 'e2e-spanish-labels';
const MBOX = 'archive@example.org';

const propertyOf = async (request: APIRequestContext, term: string): Promise<Record<string, unknown>> => {
  const found: unknown = await (await request.get(`/api/properties?term=${term}`)).json();
  return record(isArray(found) ? found[0] : null, term);
};


// The switch to Spanish is also the shared administrator's `locale` setting.
test.afterEach(async ({ page }) => {
  await unsetAdminLocale(page.request);
});

test('foaf:mbox shows as "buzón personal" on the admin and on a Spanish site', async ({ page, diagnostics }) => {
  const mbox = await propertyOf(page.request, 'foaf:mbox');
  expect(stringField(mbox, 'o:label', 'foaf:mbox')).toBe('personal mailbox');
  const title = await propertyOf(page.request, TITLE_TERM);

  const site = await page.request.post('/api/sites', {
    data: {
      'o:title': 'E2E Spanish labels',
      'o:slug': SLUG,
      'o:theme': 'default',
      'o:is_public': true,
      'o:site_settings': { locale: 'es' },
    },
  });
  expect(site.status(), await site.text()).toBe(200);
  const siteId = numberField(record(await site.json(), 'site'), 'o:id', 'site');
  const item = await page.request.post('/api/items', {
    data: {
      'o:is_public': true,
      'o:site': [{ 'o:id': siteId }],
      [TITLE_TERM]: [{ type: 'literal', property_id: numberField(title, 'o:id', TITLE_TERM), '@value': 'E2E mailbox' }],
      'foaf:mbox': [{ type: 'literal', property_id: numberField(mbox, 'o:id', 'foaf:mbox'), '@value': MBOX }],
    },
  });
  expect(item.status(), await item.text()).toBe(200);
  const itemId = numberField(record(await item.json(), 'item'), 'o:id', 'item');

  await page.goto(`/#/items/${String(itemId)}`);
  await page.getByLabel('Language').selectOption('es');
  await expect(page.getByText(MBOX)).toBeVisible();
  await expect(page.getByText('buzón personal', { exact: true })).toBeVisible();
  await expect(page.getByText('personal mailbox', { exact: true })).toHaveCount(0);
  await expectClean(diagnostics, 'admin item in Spanish');

  const answer = await page.goto(`/s/${SLUG}/item/${String(itemId)}`);
  expect(answer?.status()).toBe(200);
  await expect(page.locator('dt', { hasText: /^buzón personal$/ })).toBeVisible();
  await expect(page.locator('dt', { hasText: /^personal mailbox$/ })).toHaveCount(0);
  await expectClean(diagnostics, 'public item on a Spanish site');
});
