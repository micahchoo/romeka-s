/**
 * The Items browse's item type chips against the real server. A run-unique item type with three
 * items reads "<name> 3"; pressing it lists those three, puts `template=<id>` in the URL, names
 * the type in each row's Item type column and in a filter chip, and the walk from a row counts
 * "1 of 3". Each chip's count is the Items list total for that type. The Class select waits behind
 * More filters, and the Class column shows exactly when the server says some item has a class.
 *
 * `ITEMTYPE_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.ITEMTYPE_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: join(folder, `item-type-chips-${name}.png`), fullPage: true });
  }
};

const post = async (request: APIRequestContext, path: string, data: unknown): Promise<number> => {
  const response = await request.post(path, { data });
  expect(response.ok(), `${path}: ${await response.text()}`).toBe(true);
  return numberField(record(await response.json(), path), 'o:id', path);
};

const total = async (request: APIRequestContext, query: string): Promise<number> => {
  const response = await request.get(`/api/items?${query}&per_page=1`);
  expect(response.ok(), await response.text()).toBe(true);
  return Number(response.headers()['omeka-s-total-results']);
};

/** The class counts route: each class some item has, with its item count. */
const classCounts = async (request: APIRequestContext) => {
  const answer = record(await (await request.get('/api/romeka/classes/counts')).json(), 'class counts');
  const classes = answer.classes;
  return (isArray(classes) ? classes : []).map((entry, at) => {
    const row = record(entry, `classes[${String(at)}]`);
    return { id: numberField(row, 'id', 'class'), items: numberField(row, 'count', 'class') };
  });
};

/** The chip counts route, as `{type id: items}`, `no_item_type` and `with_class`. */
const counts = async (request: APIRequestContext) => {
  const answer = record(await (await request.get('/api/romeka/item-types/counts')).json(), 'counts');
  const types = answer.item_types;
  return {
    items: new Map(
      (isArray(types) ? types : []).map((entry) => [
        numberField(record(entry, 'type'), 'id', 'type'),
        numberField(record(entry, 'type'), 'items', 'type'),
      ]),
    ),
    noItemType: numberField(answer, 'no_item_type', 'counts'),
    withClass: numberField(answer, 'with_class', 'counts'),
  };
};

test('a chip lists the items of its type, keeps it in the URL, and the walk counts them', async ({ page, diagnostics }) => {
  const run = String(Date.now());
  const name = `Chips ${run}`;
  const type = await post(page.request, '/api/resource_templates', { 'o:label': name });
  for (const n of [1, 2, 3]) {
    await post(page.request, '/api/items', {
      'o:resource_template': { 'o:id': type },
      'dcterms:title': [{ type: 'literal', property_id: 'auto', '@value': `${name} item ${String(n)}` }],
      'o:is_public': true,
    });
  }

  await page.goto('/#/items');
  const chips = page.getByRole('radiogroup', { name: 'Item type' });
  const chip = chips.getByRole('radio', { name: `${name} 3`, exact: true });
  await expect(chip).toBeVisible();
  await expect(chip).toHaveAttribute('title', '3 items of this type in all');

  // Every chip's count is its list's total, for the same caller.
  const served = await counts(page.request);
  for (const [id, items] of served.items) {
    expect(items, `type ${String(id)}`).toBe(await total(page.request, `resource_template_id=${String(id)}`));
  }
  expect(served.noItemType).toBe(await total(page.request, 'romeka_no_resource_template=1'));
  expect(served.withClass).toBe(await total(page.request, 'romeka_has_resource_class=1'));
  const untyped = chips.getByRole('radio', { name: `No item type ${String(served.noItemType)}`, exact: true });
  await (served.noItemType > 0 ? expect(untyped).toBeVisible() : expect(untyped).toHaveCount(0));

  // The Class select is behind More filters; the Class column follows the server's answer.
  await expect(page.getByRole('combobox', { name: 'Class' })).toBeHidden();
  const classHeader = page.getByRole('columnheader', { name: 'Class', exact: true });
  await (served.withClass > 0 ? expect(classHeader).toBeVisible() : expect(classHeader).toHaveCount(0));
  await expect(page.getByRole('columnheader', { name: 'Item type', exact: true })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'items browse with item type chips');
  await shoot(page, 'rest');

  // Arrow keys move between chips; a press chooses.
  await chips.getByRole('radio').first().focus();
  await page.keyboard.press('ArrowRight');
  await expect(chips.getByRole('radio').nth(1)).toBeFocused();

  await chip.click();
  await expect(page).toHaveURL(new RegExp(`template=${String(type)}(&|$)`));
  await expect(chip).toHaveAttribute('aria-checked', 'true');
  await expect(chip).toHaveAccessibleName(`${name} 3`);
  await expect(page.getByText(`Item type: ${name}`, { exact: true })).toBeVisible();
  const rows = page.locator('.items-results tbody tr');
  await expect(rows).toHaveCount(3);
  for (const row of await rows.all()) {
    await expect(row.getByRole('button', { name, exact: true })).toBeVisible();
  }
  await diagnostics.idle();
  await shoot(page, 'chosen');

  await rows.first().getByRole('link').click();
  await expect(page.getByText(/^1 of 3 in /)).toBeVisible();
  await page.goBack();

  await page.getByRole('button', { name: 'More filters' }).click();
  const classSelect = page.getByRole('combobox', { name: 'Class' });
  await expect(classSelect).toBeVisible();
  // The Class select offers only the classes some item has, each with the total its filter lists.
  const used = await classCounts(page.request);
  await expect(classSelect.getByRole('option')).toHaveCount(used.length + 1);
  for (const { id, items } of used) {
    const option = classSelect.locator(`option[value="${String(id)}"]`);
    await expect(option).toHaveText(items === 1 ? / · 1 item$/ : new RegExp(` · ${items.toLocaleString('en')} items$`));
    expect(items, `class ${String(id)}`).toBe(await total(page.request, `resource_class_id=${String(id)}`));
  }
  await expect(page.getByRole('combobox', { name: 'Item type' })).toHaveCount(0);
  await shoot(page, 'more-filters');

  await chips.getByRole('radio', { name: `${name} 3`, exact: true }).click();
  await expect(page).not.toHaveURL(/template=/);
  await expect(page.getByText(`Item type: ${name}`, { exact: true })).toHaveCount(0);
  await expectClean(diagnostics, 'item type chips');
});
