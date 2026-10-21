/**
 * The global settings page: the media alt text property is one text field,
 * stored through `PATCH /api/settings` and read back on the next load.
 *
 * Upstream's `SettingForm` has the field in its editing group; the port's SPA
 * carries only the keys its page edits, and this is the newest of them.
 */
import { expect, expectClean, test } from '../fixtures';

test('the media alt text property saves, reads back, and empties again', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/settings');
  const field = page.getByLabel('Media alt text property');
  await expect(field).toHaveValue('');
  await expect(field).toHaveAccessibleDescription(/first value describes a media/);

  await field.fill('dcterms:title');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Settings updated.')).toBeVisible();

  await page.reload();
  await expect(page.getByLabel('Media alt text property')).toHaveValue('dcterms:title');
  await expectClean(diagnostics, 'global settings save');

  // Leave the setting the other specs share as it was found.
  await page.getByLabel('Media alt text property').fill('');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Settings updated.')).toBeVisible();
  await page.reload();
  await expect(page.getByLabel('Media alt text property')).toHaveValue('');
  await expectClean(diagnostics, 'global settings clear');
});
