/**
 * The locale switch in a real browser: it re-renders the shell in the chosen
 * language, stores the choice, and the next load reads it back.
 */
import { expect, expectClean, test } from '../fixtures';

test('the locale switch re-renders the shell and survives a reload', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/');
  await expect(page.getByText('Omeka S admin')).toBeVisible();

  await page.getByLabel('Language').selectOption('es');

  await expect(page.getByText('Administración de Omeka S')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Elementos', exact: true })).toBeVisible();
  // The document language follows the switch, so a screen reader reads the
  // Spanish text as Spanish.
  await expect(page.locator('html')).toHaveAttribute('lang', 'es');
  const stored = await page.evaluate(() => window.localStorage.getItem('omeka.locale'));
  expect(stored).toBe('es');
  await expectClean(diagnostics, 'switch to Spanish');

  await page.reload();
  await expect(page.getByText('Administración de Omeka S')).toBeVisible();
  await expect(page.getByLabel('Idioma')).toHaveValue('es');
  await expect(page.locator('html')).toHaveAttribute('lang', 'es');
  await expectClean(diagnostics, 'reload in Spanish');

  await page.getByLabel('Idioma').selectOption('en');
  await expect(page.getByText('Omeka S admin')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  expect(await page.evaluate(() => window.localStorage.getItem('omeka.locale'))).toBe('en');
  await expectClean(diagnostics, 'switch back to English');
});
