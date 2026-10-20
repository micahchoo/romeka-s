/**
 * The content blocks end to end: a curator inserts each block into a fresh
 * page, fills its form, saves, and a visitor sees it on the public page. Each
 * test makes its own items and page, so the specs never share state.
 */
import { expect, expectClean, test } from '../fixtures';
import { SITE_SLUG } from '../harness/seed';
import { createItem, createPage, holdApiKey, insertBlock, save } from '../contentBlocks';

test.beforeEach(async ({ page }) => {
  await holdApiKey(page);
});

test('a gallery of items opens its images in a lightbox', async ({ page, diagnostics }) => {
  const harbour = 'E2E gallery harbour';
  const fields = 'E2E gallery fields';
  await createItem(page, harbour, { colour: [70, 120, 190] });
  await createItem(page, fields, { colour: [190, 160, 60] });
  await page.goto(await createPage(page, 'gallery-block'));
  const form = await insertBlock(page, 'Image gallery');
  const picker = form.getByRole('listbox', { name: 'Add or remove items' });
  for (const title of [harbour, fields]) {
    await form.getByLabel('Search titles and values').fill(title);
    await picker.getByRole('option', { name: title, exact: true, selected: false }).click();
  }
  await form.getByLabel('Layout', { exact: true }).selectOption('masonry');
  await form.getByRole('spinbutton', { name: 'Columns', exact: true }).fill('2');

  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-gallery-masonry .romeka-gallery-image')).toHaveCount(2);
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/gallery-block`);
  expect(response?.status()).toBe(200);
  const gallery = page.locator('.romeka-gallery');
  await expect(gallery).toHaveClass(/romeka-gallery-masonry romeka-gallery-columns-2/);
  const links = gallery.locator('a.romeka-gallery-link');
  await expect(links).toHaveCount(2);
  await expect(links.first()).toHaveAttribute('href', /\/files\/(large|original)\//);
  await expect(gallery.locator('figcaption').first()).toHaveText(harbour);
  await expect
    .poll(() => links.first().locator('img').evaluate((image: HTMLImageElement) => image.naturalWidth))
    .toBeGreaterThan(0);

  await links.first().click();
  const lightbox = page.getByRole('dialog', { name: 'Image 1 of 2' });
  await expect(lightbox).toBeVisible();
  await expect(lightbox.locator('figcaption')).toHaveText(harbour);
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('dialog', { name: 'Image 2 of 2' })).toBeVisible();
  await expect(page.getByRole('dialog').locator('figcaption')).toHaveText(fields);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(links.nth(1)).toBeFocused();
  await expectClean(diagnostics, 'the public page with a gallery');
});

test('a featured item shows its image, chosen fields and a link', async ({ page, diagnostics }) => {
  const title = 'E2E featured lighthouse';
  await createItem(page, title, {
    colour: [200, 90, 60],
    values: { 'dcterms:date': '1902', 'dcterms:description': 'A lighthouse on the point.' },
  });
  await page.goto(await createPage(page, 'featured-block'));
  const form = await insertBlock(page, 'Featured item');
  await form.getByLabel('Search titles and values').fill(title);
  await form
    .getByRole('listbox', { name: 'Item', exact: true })
    .getByRole('option', { name: title, exact: true, selected: false })
    .click();
  await form.getByRole('button', { name: 'Add item' }).click();
  await form.getByLabel('Field', { exact: true }).selectOption('dcterms:date');
  await form.getByLabel('Image', { exact: true }).selectOption('right');

  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-featured-title')).toHaveText(title);
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/featured-block`);
  expect(response?.status()).toBe(200);
  const card = page.locator('.romeka-featured');
  await expect(card).toHaveClass(/romeka-featured-image-right/);
  await expect(card.locator('dt')).toHaveText('Date');
  await expect(card.locator('dd')).toHaveText('1902');
  await expect
    .poll(() => card.locator('img').evaluate((image: HTMLImageElement) => image.naturalWidth))
    .toBeGreaterThan(0);
  await card.getByRole('link', { name: 'View item' }).click();
  await expect(page).toHaveURL(/\/s\/[^/]+\/item\/\d+$/);
  await expect(page.getByRole('heading', { name: title }).first()).toBeVisible();
  await expectClean(diagnostics, 'the public page with a featured item');
});

test('a button block links two buttons in the chosen style', async ({ page, diagnostics }) => {
  await page.goto(await createPage(page, 'button-block'));
  const form = await insertBlock(page, 'Button');
  await form.getByLabel('Button text', { exact: true }).fill('Browse the collection');
  const first = form.getByRole('group', { name: 'Button link', exact: true });
  await first.getByRole('radio', { name: 'A web address' }).check();
  await first.getByRole('textbox', { name: 'A web address' }).fill('/s/e2e-site/item');
  await form.getByLabel('Second button text').fill('Read the guide');
  const second = form.getByRole('group', { name: 'Second button link' });
  await second.getByRole('radio', { name: 'A web address' }).check();
  await second.getByRole('textbox', { name: 'A web address' }).fill('https://example.org/guide');
  await form.getByLabel('Alignment', { exact: true }).selectOption('center');
  await form.getByLabel('Size', { exact: true }).selectOption('large');
  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-button')).toHaveCount(2);
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/button-block`);
  expect(response?.status()).toBe(200);
  const row = page.locator('.romeka-buttons');
  await expect(row).toHaveClass(/romeka-buttons-align-center/);
  await expect(row.getByRole('link', { name: 'Read the guide' })).toHaveAttribute(
    'href',
    'https://example.org/guide',
  );
  await row.getByRole('link', { name: 'Browse the collection' }).click();
  await expect(page).toHaveURL(/\/s\/e2e-site\/item$/);
  await expectClean(diagnostics, 'the public page with a button block');
});

test('a quote shows the quotation, who said it and its source', async ({ page, diagnostics }) => {
  const source = 'E2E quote ship log';
  await createItem(page, source);
  await page.goto(await createPage(page, 'quote-block'));
  const form = await insertBlock(page, 'Quote');
  await form.getByRole('textbox', { name: 'Quotation' }).fill('The sea was calm at noon.');
  await form.getByLabel('Said by').fill('The first mate');
  await form.getByLabel('Search titles and values').fill(source);
  await form
    .getByRole('listbox', { name: 'Source item' })
    .getByRole('option', { name: source, exact: true, selected: false })
    .click();
  await form.getByLabel('Style', { exact: true }).selectOption('band');
  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-quote-band')).toContainText('The sea was calm at noon.');
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/quote-block`);
  expect(response?.status()).toBe(200);
  const quote = page.locator('figure.romeka-quote-band');
  await expect(quote.locator('blockquote')).toHaveText('The sea was calm at noon.');
  await expect(quote.locator('figcaption')).toHaveText(`The first mate, ${source}`);
  await quote.getByRole('link', { name: source }).click();
  await expect(page).toHaveURL(/\/item\/\d+$/);
  await expectClean(diagnostics, 'the public page with a quote');
});

test('an accordion opens each answer from its heading', async ({ page, diagnostics }) => {
  await page.goto(await createPage(page, 'accordion-block'));
  const form = await insertBlock(page, 'Accordion');
  for (const [question, answer] of [
    ['When is the reading room open?', 'Tuesday to Saturday, ten to five.'],
    ['Can I bring a camera?', 'Yes, without a flash.'],
  ] as const) {
    await form.getByRole('button', { name: 'Add item' }).click();
    await form.getByLabel('Question or heading').last().fill(question);
    await form.getByRole('textbox', { name: 'Answer' }).last().fill(answer);
  }
  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-accordion-item')).toHaveCount(2);
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/accordion-block`);
  expect(response?.status()).toBe(200);
  const answer = page.getByText('Yes, without a flash.');
  await expect(answer).toBeHidden();
  await page.getByText('Can I bring a camera?').click();
  await expect(answer).toBeVisible();
  await expect(page.getByText('Tuesday to Saturday, ten to five.')).toBeHidden();
  await expectClean(diagnostics, 'the public page with an accordion');
});

test('a timeline places items in date order and lists the undated after', async ({ page, diagnostics }) => {
  const items = [
    ['E2E timeline opening', '1894-06-30'],
    ['E2E timeline survey', '1886?'],
    ['E2E timeline sketch', 'some spring'],
  ] as const;
  for (const [title, date] of items) {
    await createItem(page, title, { values: { 'dcterms:date': date } });
  }
  await page.goto(await createPage(page, 'timeline-block'));
  const form = await insertBlock(page, 'Timeline');
  const picker = form.getByRole('listbox', { name: 'Items', exact: true });
  for (const [title] of items) {
    await form.getByLabel('Search titles and values').fill(title);
    await picker.getByRole('option', { name: title, exact: true, selected: false }).click();
  }
  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-timeline-entry')).toHaveCount(2);
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/timeline-block`);
  expect(response?.status()).toBe(200);
  await expect(page.locator('.romeka-timeline-title')).toHaveText([
    'E2E timeline survey',
    'E2E timeline opening',
  ]);
  await expect(page.locator('.romeka-timeline-date')).toHaveText(['1886?', '1894-06-30']);
  await expect(page.locator('.romeka-timeline-undated')).toHaveText('E2E timeline sketch some spring');
  await expectClean(diagnostics, 'the public page with a timeline');
});

test('a search box searches the site items from the page', async ({ page, diagnostics }) => {
  const title = 'E2E search box quillfeather';
  await createItem(page, title, { onSite: true });
  await page.goto(await createPage(page, 'search-block'));
  const form = await insertBlock(page, 'Search box');
  await form.getByLabel('Placeholder').fill('Search the logs');
  await form.getByLabel('Search in').selectOption('items');
  const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
  await expect(preview.locator('.romeka-search-input')).toHaveAttribute('placeholder', 'Search the logs');
  await save(page);

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/search-block`);
  expect(response?.status()).toBe(200);
  const box = page.locator('form.romeka-search');
  await box.getByRole('searchbox').fill('quillfeather');
  await box.getByRole('button', { name: 'Search' }).click();
  await expect(page).toHaveURL(/\/s\/e2e-site\/item\?fulltext_search=quillfeather$/);
  await expect(page.getByRole('link', { name: title }).first()).toBeVisible();
  await expectClean(diagnostics, 'a search from the search box');
});
