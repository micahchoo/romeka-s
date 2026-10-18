/**
 * The one-time-link surfaces in a real browser: an administrator invites a user,
 * the invitation link opens the set-password page, and the key it carried works
 * exactly once.
 *
 * The link's shape is the server's, so the spec follows whatever the mint
 * answered rather than building a URL of its own. That is the point of driving it
 * here: the page has to read the key out of the URL the mail would have carried.
 */
import { expect, expectClean, test } from '../fixtures';

test('an invitation link sets a password once and then refuses', async ({ page, diagnostics }) => {
  await page.goto('/#/users/new');
  await page.getByLabel('Display name').fill('Invited User');
  await page.getByLabel('Email').fill('invited@example.com');
  await page.getByLabel('Role').selectOption('author');
  await page.getByRole('button', { name: 'Invite this user' }).click();

  const link = page.getByLabel('Password link for Invited User');
  await expect(link).toBeVisible();
  await expect(page.getByText('No mail was sent, so hand this link to the user.')).toBeVisible();
  const url = await link.inputValue();
  expect(url).toContain('#/password/');
  await expectClean(diagnostics, 'invite');

  await page.goto(url);
  await expect(page.getByRole('heading', { name: 'Set a password' })).toBeVisible();
  await page.getByLabel('New password', { exact: true }).fill('e2e-invited-password');
  await page.getByLabel('Confirm the new password').fill('e2e-invited-password');
  await page.getByRole('button', { name: 'Set the password' }).click();

  await expect(
    page.getByText('Your password is set. Sign in from the header to continue.'),
  ).toBeVisible();
  await expectClean(diagnostics, 'set password');

  // The key is one-time: the same link answers 404 the second time, and the page
  // renders the server's message.
  await page.reload();
  await page.getByLabel('New password', { exact: true }).fill('e2e-invited-password');
  await page.getByLabel('Confirm the new password').fill('e2e-invited-password');
  await page.getByRole('button', { name: 'Set the password' }).click();

  await expect(page.getByRole('alert')).toContainText('(HTTP 404)');
  diagnostics.reset();
});

test('a password link with no key says so and offers the request form', async ({ page }) => {
  await page.goto('/#/password');

  await expect(page.getByRole('alert')).toContainText('This link carries no password key.');
  await page.getByRole('link', { name: 'Reset your password' }).click();

  await expect(page.getByRole('heading', { name: 'Reset your password' })).toBeVisible();
  await page.getByLabel('Email').fill('nobody@example.com');
  await page.getByRole('button', { name: 'Send the reset link' }).click();

  await expect(page.getByText('Check your mail for the reset link.')).toBeVisible();
});
