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
  await expect(page.getByText('Romeka S admin')).toBeVisible();

  await page.getByLabel('Language').selectOption('es');

  await expect(page.getByText('Administración de Romeka S')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Elementos', exact: true })).toBeVisible();
  // The document language follows the switch, so a screen reader reads the
  // Spanish text as Spanish.
  await expect(page.locator('html')).toHaveAttribute('lang', 'es');
  const stored = await page.evaluate(() => window.localStorage.getItem('romeka.locale'));
  expect(stored).toBe('es');
  await expectClean(diagnostics, 'switch to Spanish');

  await page.reload();
  await expect(page.getByText('Administración de Romeka S')).toBeVisible();
  await expect(page.getByLabel('Idioma')).toHaveValue('es');
  await expect(page.locator('html')).toHaveAttribute('lang', 'es');
  await expectClean(diagnostics, 'reload in Spanish');

  await page.getByLabel('Idioma').selectOption('en');
  await expect(page.getByText('Romeka S admin')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  expect(await page.evaluate(() => window.localStorage.getItem('romeka.locale'))).toBe('en');
  await expectClean(diagnostics, 'switch back to English');
});

test('only the chosen language downloads, and a reload in Spanish never paints English', async ({
  page,
  diagnostics,
}) => {
  const scripts = new Set<string>();
  page.on('request', (request) => {
    if (request.resourceType() === 'script') {
      scripts.add(request.url());
    }
  });
  await page.goto('/');
  await expect(page.getByText('Romeka S admin')).toBeVisible();
  await diagnostics.idle();
  const english = new Set(scripts);

  await page.getByLabel('Language').selectOption('es');
  await expect(page.getByText('Administración de Romeka S')).toBeVisible();
  const spanishOnly = [...scripts].filter((url) => !english.has(url));
  // The switch fetched a script the English page never asked for: the Spanish catalogue.
  expect(spanishOnly).toHaveLength(1);

  // Mark the document with every title it ever shows, from before the first render.
  await page.addInitScript(() => {
    new MutationObserver(() => {
      for (const node of document.querySelectorAll('.app-title')) {
        document.documentElement.dataset[
          node.textContent === 'Romeka S admin' ? 'sawEnglishTitle' : 'sawTitle'
        ] = 'yes';
      }
    }).observe(document, { childList: true, subtree: true, characterData: true });
  });
  await page.reload();
  await expect(page.getByText('Administración de Romeka S')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-saw-title', 'yes');
  await expect(page.locator('html')).not.toHaveAttribute('data-saw-english-title', 'yes');
  await expectClean(diagnostics, 'reload in Spanish');
});
