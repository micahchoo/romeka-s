/**
 * Advanced > Standards and the shell around it, driven in a real browser: the
 * phone layout's rows and nav strip, the routes upstream's vocabulary pages
 * used, the "?" definitions, and a vocabulary built on its own page.
 */
import { expect, expectClean, test } from '../fixtures';

const PHONE = { width: 390, height: 844 };

test('on a phone, a short page leaves the header and the nav strip at their own height', async ({
  page,
}) => {
  await page.setViewportSize(PHONE);
  await page.goto('/#/system');
  await expect(page.getByRole('heading', { name: 'System', level: 1 })).toBeVisible();

  for (const selector of ['.app-header', '.app-nav']) {
    const box = page.locator(selector);
    const height = await box.evaluate((element) => element.getBoundingClientRect().height);
    const content = await box.evaluate((element) => {
      const style = getComputedStyle(element);
      const children = [...element.children].map((child) => child.getBoundingClientRect());
      const top = Math.min(...children.map((rect) => rect.top));
      const bottom = Math.max(...children.map((rect) => rect.bottom));
      return (
        bottom -
        top +
        Number.parseFloat(style.paddingTop) +
        Number.parseFloat(style.paddingBottom) +
        Number.parseFloat(style.borderBottomWidth)
      );
    });
    expect(height, `${selector} is as tall as what it holds`).toBeLessThanOrEqual(content + 1);
  }
});

test('on a phone, the nav strip scrolls the active link into view', async ({ page }) => {
  await page.setViewportSize(PHONE);
  await page.goto('/#/system/query');
  const link = page.getByRole('navigation', { name: 'Admin pages' }).getByRole('link', {
    name: 'System',
  });
  await expect(link).toHaveAttribute('aria-current', 'page');
  await expect(link).toBeInViewport({ ratio: 1 });
});

test('an upstream vocabulary route lands on its Standards tab', async ({ page, diagnostics }) => {
  await page.goto('/#/properties');
  await expect(page.getByRole('heading', { name: 'Standards', level: 1 })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Properties' })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expect(page.getByRole('link', { name: 'Standards' })).toHaveAttribute(
    'aria-current',
    'page',
  );

  const define = page.getByRole('button', { name: 'What dcterms:title means' });
  await define.click();
  await expect(define).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByText('A name given to the resource.')).toBeVisible();
  await expectClean(diagnostics, 'properties tab');
});

test('a vocabulary is created and given a property and a class on its own page', async ({
  page,
  diagnostics,
}) => {
  const label = 'E2E built vocabulary';

  await page.goto('/#/standards');
  await page.getByRole('link', { name: 'Add vocabulary' }).click();
  await page.locator('#vocabulary-namespace-uri').fill('http://example.org/built/');
  await page.locator('#vocabulary-prefix').fill('e2ebuilt');
  await page.locator('#vocabulary-label').fill(label);
  await page.getByRole('button', { name: 'Add vocabulary' }).click();
  await expect(page).toHaveURL(/#\/standards\/vocabularies\/\d+$/);
  const vocabularyUrl = page.url();

  await page.getByRole('button', { name: 'Add property', expanded: false }).click();
  await page.locator('#member-properties-local-name').fill('shelfMark');
  await page.locator('#member-properties-label').fill('Shelf mark');
  await page.locator('#member-properties-local-name').press('Enter');
  await expect(page.getByRole('link', { name: 'Shelf mark' })).toBeVisible();

  await page.getByRole('button', { name: 'Add resource class', expanded: false }).click();
  await page.locator('#member-resource_classes-local-name').fill('Ledger');
  await page.locator('#member-resource_classes-label').fill('Ledger');
  await page.locator('#member-resource_classes-local-name').press('Enter');
  await expect(page.getByRole('link', { name: 'Ledger' })).toBeVisible();
  await expect(page).toHaveURL(vocabularyUrl);
  await expectClean(diagnostics, 'members added in place');

  // The members were written, not only drawn: a fresh read lists both.
  await page.reload();
  await expect(page.getByText('e2ebuilt:shelfMark')).toBeVisible();
  await expect(page.getByText('e2ebuilt:Ledger')).toBeVisible();

  await page.getByRole('button', { name: 'Delete this vocabulary' }).click();
  await page.getByRole('button', { name: `Delete ${label}` }).click();
  await expect(page).toHaveURL(/#\/standards\/vocabularies$/);
  await expectClean(diagnostics, 'built vocabulary delete');
});
