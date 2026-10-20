/**
 * Four upstream blocks with Romeka settings, end to end: a curator sets each
 * option in the block inspector, saves, and the public page draws it. The
 * spec makes its own site, items and pages, and deletes the site at the end,
 * so no other spec sees them.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { request as newRequest } from '@playwright/test';
import { expect, test } from '../fixtures';
import { ASSET_NAME } from '../harness/seed';
import { readHarness } from '../harness/state';

const SITE = 'Harbour Archive';
const SLUG = 'harbour-archive';
const CHARTS = ['Chart of the inner harbour', 'Chart of the river mouth', 'Chart of the east quay', 'Chart of the lighthouse'];
const SHOP = 'https://example.org/harbour-shop';

interface Fixture {
  readonly api: APIRequestContext;
  readonly site: number;
  readonly asset: number;
  readonly items: readonly number[];
}

let fixture: Fixture | null = null;

const setUp = (): Fixture => {
  if (fixture === null) {
    throw new Error('the fixture was not built');
  }
  return fixture;
};

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const listOf = (value: unknown): readonly unknown[] => (Array.isArray(value) ? value : []);

const post = async (api: APIRequestContext, path: string, data: unknown): Promise<number> => {
  const response = await api.post(path, { data });
  expect(response.status(), await response.text()).toBe(200);
  return idOf(await response.json());
};

const propertyId = async (api: APIRequestContext, term: string): Promise<number> =>
  idOf(listOf(await (await api.get(`/api/properties?term=${term}`)).json())[0]);

const literal = (property: number, value: string) => [{ type: 'literal', property_id: property, '@value': value }];

test.beforeAll(async () => {
  const api = await newRequest.newContext({ baseURL: readHarness().baseUrl });
  const [title, creator, date, identifier] = await Promise.all(
    ['dcterms:title', 'dcterms:creator', 'dcterms:date', 'dcterms:identifier'].map((term) => propertyId(api, term)),
  );
  if (title === undefined || creator === undefined || date === undefined || identifier === undefined) {
    throw new Error('a Dublin Core property is missing');
  }
  const site = await post(api, '/api/sites', {
    'o:title': SITE,
    'o:slug': SLUG,
    'o:theme': 'default',
    'o:is_public': true,
  });
  const items: number[] = [];
  for (const [index, chart] of CHARTS.entries()) {
    items.push(
      await post(api, '/api/items', {
        'o:is_public': true,
        'o:site': [{ 'o:id': site }],
        'dcterms:title': literal(title, chart),
        'dcterms:creator': literal(creator, 'Ada Byron'),
        'dcterms:date': literal(date, String(1820 + index * 10)),
        'dcterms:identifier': literal(identifier, `HC-${String(index + 1)}`),
      }),
    );
  }
  const assets = listOf(await (await api.get('/api/assets')).json());
  const named = assets.find(
    (entry) => typeof entry === 'object' && entry !== null && 'o:name' in entry && entry['o:name'] === ASSET_NAME,
  );
  fixture = { api, site, asset: idOf(named), items };
});

test.afterAll(async () => {
  const built = setUp();
  await built.api.delete(`/api/sites/${String(built.site)}`);
  await built.api.dispose();
});

test.beforeEach(async ({ page }) => {
  // The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
  // not key, so the browser holds the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

/** A page of the spec's site holding `blocks`; returns its id. */
const createPage = async (slug: string, title: string, blocks: readonly unknown[]): Promise<number> =>
  post(setUp().api, '/api/site_pages', {
    'o:site': { 'o:id': setUp().site },
    'o:title': title,
    'o:slug': slug,
    'o:is_public': true,
    'o:block': blocks,
  });

/** Opens a page in the editor and selects its block at `position`. */
const editBlock = async (page: Page, pageId: number, position: number, label: string) => {
  await page.goto(`/#/sites/${String(setUp().site)}/pages/${String(pageId)}`);
  await page.getByRole('listitem', { name: new RegExp(`^Block ${String(position)}: ${label}`) }).click();
  return page.getByRole('region', { name: 'Block inspector' });
};

const save = async (page: Page): Promise<void> => {
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
};

test('a browse preview drawn as a three-column carousel steps with Next', async ({ page }) => {
  const id = await createPage('recent', 'Recent charts', [
    { 'o:layout': 'browsePreview', 'o:data': { resource_type: 'items', query: 'sort_by=dcterms:date&sort_order=asc', heading: 'Recent charts', limit: 12, components: ['resource-heading', 'thumbnail'] } },
  ]);
  const form = await editBlock(page, id, 1, 'Browse preview');
  await form.getByLabel('Card style').selectOption('carousel');
  await form.getByLabel('Columns', { exact: true }).selectOption('3');
  const details = form.getByRole('group', { name: 'Details under the name' });
  await details.getByRole('checkbox', { name: 'Creator' }).uncheck();
  await save(page);

  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto(`/s/${SLUG}/page/recent`);
  const carousel = page.getByRole('region', { name: 'Recent charts' });
  const track = carousel.getByRole('list');
  const cards = track.getByRole('listitem');
  await expect(cards).toHaveCount(CHARTS.length);
  // The stored sort puts the oldest chart first.
  await expect(cards.first()).toContainText(CHARTS[0] ?? '');
  await expect(cards.first()).toContainText('1820');
  await expect(cards.first()).not.toContainText('Ada Byron');
  const width = async (index: number): Promise<number> =>
    (await cards.nth(index).boundingBox())?.width ?? 0;
  const trackWidth = (await track.boundingBox())?.width ?? 0;
  expect(await width(0)).toBeGreaterThan(trackWidth / 3 - 30);
  expect(await width(0)).toBeLessThan(trackWidth / 3 + 1);
  await expect(carousel.getByRole('link', { name: 'Prev' })).toHaveAttribute('aria-disabled', 'true');
  await carousel.getByRole('link', { name: 'Next' }).click();
  await expect.poll(() => track.evaluate((element) => element.scrollLeft)).toBeGreaterThan(100);
  await expect(page).toHaveURL(new RegExp(`/s/${SLUG}/page/recent$`));

  await page.setViewportSize({ width: 390, height: 844 });
  const phoneTrack = (await track.boundingBox())?.width ?? 0;
  expect(await width(0)).toBeGreaterThan(phoneTrack * 0.6);
});

test('an asset grid links an image to a web address and keeps its shape', async ({ page }) => {
  const { asset } = setUp();
  const id = await createPage('shop', 'Shop', [
    {
      'o:layout': 'asset',
      'o:data': [
        { id: asset, page: '', alt_link_title: 'Visit the shop', caption: 'Prints and maps' },
        { id: asset, page: '', alt_link_title: '', caption: 'The quay' },
      ],
    },
  ]);
  const form = await editBlock(page, id, 1, 'Asset');
  const first = form.locator('.repeat-row').first();
  await first.getByRole('radio', { name: 'A web address' }).check();
  await first.getByRole('textbox', { name: 'A web address' }).fill(SHOP);
  await expect(form.getByLabel('Columns', { exact: true })).toHaveCount(0);
  await form.getByLabel('Arrange images').selectOption('grid');
  await form.getByLabel('Columns', { exact: true }).selectOption('2');
  await form.getByLabel('Caption alignment').selectOption('center');
  await save(page);

  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto(`/s/${SLUG}/page/shop`);
  const grid = page.locator('.assets.assets-grid.assets-columns-2');
  await expect(grid.locator('.asset')).toHaveCount(2);
  await expect(page.getByRole('link', { name: 'Visit the shop' })).toHaveAttribute('href', SHOP);
  const image = grid.locator('img').first();
  await expect(image).toHaveCSS('aspect-ratio', 'auto');
  const box = await image.boundingBox();
  const ratio = await image.evaluate((element) =>
    element instanceof HTMLImageElement ? element.naturalWidth / element.naturalHeight : 0,
  );
  expect(Math.abs((box?.width ?? 0) / (box?.height ?? 1) - ratio)).toBeLessThan(0.05);
  const [left, right] = await grid.locator('.asset').evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect().left),
  );
  expect((right ?? 0) - (left ?? 0)).toBeGreaterThan(200);
  await expect(grid.locator('.caption').first()).toHaveCSS('text-align', 'center');
});

test('a list of pages drawn as cards shows each page’s image and first paragraph', async ({ page }) => {
  const { asset } = setUp();
  await createPage('history', 'Harbour history', [
    { 'o:layout': 'asset', 'o:data': [{ id: asset, page: '', alt_link_title: '', caption: '' }] },
    { 'o:layout': 'html', 'o:data': { html: '<h2>History</h2><p>The harbour opened in 1820 and grew with the river trade.</p>' } },
  ]);
  await createPage('visit', 'Visiting', [
    { 'o:layout': 'html', 'o:data': { html: '<p>Open every day from nine.</p>' } },
  ]);
  const pages = listOf(await (await setUp().api.get(`/api/site_pages?site_id=${String(setUp().site)}`)).json());
  const pageId = (slug: string): number =>
    idOf(pages.find((entry) => typeof entry === 'object' && entry !== null && 'o:slug' in entry && entry['o:slug'] === slug));
  const pagelist = JSON.stringify([
    { text: 'Harbour history', data: { id: pageId('history') } },
    { text: 'Visiting', data: { id: pageId('visit') } },
  ]);
  const id = await createPage('contents', 'Contents', [{ 'o:layout': 'listOfPages', 'o:data': { pagelist } }]);
  const form = await editBlock(page, id, 1, 'List of pages');
  await form.getByLabel('Show pages as').selectOption('cards');
  await form.getByLabel('Columns', { exact: true }).selectOption('2');
  await save(page);

  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto(`/s/${SLUG}/page/contents`);
  const cards = page.locator('.page-card');
  await expect(cards).toHaveCount(2);
  const history = page.getByRole('link', { name: /^Harbour history/ });
  await expect(history).toContainText('The harbour opened in 1820 and grew with the river trade.');
  await expect(history.locator('img')).toHaveCount(1);
  await expect(page.getByRole('link', { name: /^Visiting/ })).toContainText('Open every day from nine.');
  const [left, right] = await cards.evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect().left),
  );
  expect((right ?? 0) - (left ?? 0)).toBeGreaterThan(300);
});

test('an item with metadata as a compact card prints the chosen field and a citation', async ({ page }) => {
  const item = setUp().items[1] ?? 0;
  const id = await createPage('featured', 'Featured chart', [
    {
      'o:layout': 'itemWithMetadata',
      'o:data': {},
      'o:attachment': [{ 'o:item': { 'o:id': item }, 'o:media': null, 'o:caption': '' }],
    },
  ]);
  const form = await editBlock(page, id, 1, 'Item with metadata');
  const fields = form.getByRole('group', { name: 'Fields to show' });
  await fields.getByLabel('Add a field').selectOption('dcterms:date');
  await expect(fields.getByText('Date (dcterms:date)')).toBeVisible();
  await form.getByLabel('Show items as').selectOption('card');
  await save(page);

  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto(`/s/${SLUG}/page/featured`);
  const card = page.locator('.item-card');
  await expect(card.getByRole('heading', { name: CHARTS[1] ?? '' })).toBeVisible();
  await expect(card.locator('.values dt')).toHaveText(['Date']);
  await expect(card.locator('.item-card-citation')).toHaveText(
    `Ada Byron. ${CHARTS[1] ?? ''}. 1830. HC-2. ${SITE}.`,
  );
  await expect(card.locator('.item-card-citation').getByRole('link', { name: SITE })).toHaveAttribute(
    'href',
    new RegExp(`/s/${SLUG}/item/${String(item)}$`),
  );
});
