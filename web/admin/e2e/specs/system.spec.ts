/**
 * Admin > Advanced > System: the four pages it replaced open it, the nav names it once, and each
 * tab works on the seeded install. The dashboard's site names open their site.
 * `SYSTEM_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { ITEM_TITLES, SITE_TITLE } from '../harness/seed';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.SYSTEM_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `system-${name}.png`), fullPage: true });
  }
};

test('each old page opens its System tab, and the nav names System once, under Advanced', async ({
  page,
  diagnostics,
}) => {
  const tabs = page.getByRole('navigation', { name: 'System' });
  const landings: readonly (readonly [string, string])[] = [
    ['system-info', 'System information'],
    ['columns', 'System information'],
    ['modules', 'Modules'],
    ['query', 'Search query'],
  ];
  for (const [route, tab] of landings) {
    await page.goto(`/#/${route}`);
    await expect(page.getByRole('heading', { name: 'System', level: 1 }), route).toBeVisible();
    await expect(tabs.getByRole('link', { name: tab }), route).toHaveAttribute('aria-current', 'page');
  }

  const nav = page.getByRole('navigation', { name: 'Admin pages' });
  const advanced = nav.locator('.nav-group').filter({ hasText: 'Admin' }).locator('.nav-sub');
  await expect(advanced.getByRole('heading', { name: 'Advanced' })).toBeVisible();
  await expect(advanced.getByRole('link', { name: 'System' })).toHaveAttribute('href', '#/system');
  for (const gone of ['System information', 'Modules', 'Search query', 'Browse columns']) {
    await expect(nav.getByRole('link', { name: gone })).toHaveCount(0);
  }

  await nav.getByRole('link', { name: 'System' }).click();
  await expect(page).toHaveURL(/#\/system$/);
  await expect(page.getByRole('cell', { name: 'items', exact: false }).first()).toBeVisible();
  await shoot(page, 'about');

  await tabs.getByRole('link', { name: 'Modules' }).click();
  await expect(page).toHaveURL(/#\/system\/modules$/);
  await expect(page.locator('main table')).toBeVisible();
  await shoot(page, 'modules');

  await tabs.getByRole('link', { name: 'Search query' }).click();
  await page.locator('#query-search').fill('E2E item');
  await page.getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByRole('cell', { name: ITEM_TITLES[0] ?? 'E2E item 1', exact: true })).toBeVisible();
  await shoot(page, 'query');
  await expectClean(diagnostics, 'system tabs');
});

test('a site name on the dashboard opens that site', async ({ page, diagnostics }) => {
  await page.goto('/#/dashboard');
  const site = page.getByRole('link', { name: SITE_TITLE, exact: true });
  await expect(site).toHaveAttribute('href', /^#\/sites\/\d+$/);
  await shoot(page, 'dashboard');
  await site.click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  await expect(page.getByRole('heading', { name: SITE_TITLE })).toBeVisible();
  await expectClean(diagnostics, 'dashboard site link');
});
