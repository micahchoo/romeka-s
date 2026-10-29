/**
 * The page editor's live surface against the real server: the preview render
 * carries the block markers, and the admin outlines, selects and inserts
 * through two script-free frames. A re-render keeps the reader's place, and
 * the swap waits for the new document's images (`load`).
 */
import type { Page } from '@playwright/test';
import { expect, test } from '../fixtures';
import AxeBuilder from '@axe-core/playwright';
import { AXE_TAGS } from '../harness/axe';
import { readHarness } from '../harness/state';
import { SITE_SLUG } from '../harness/seed';

const SLOW_IMAGE = '/live-canvas-slow.png';
const IMAGE_DELAY_MS = 1500;
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGM4ISf3Hx9mGBkKAFNGgMHUn6b2AAAAAElFTkSuQmCC',
  'base64',
);

const section = (n: number): string =>
  `<h2>Section ${String(n)} heading</h2>${'<p>A paragraph long enough to give this section some height on the page.</p>'.repeat(4)}`;

/** Nine blocks: two sections, a group of two, a section with a slow image, three more. */
const BLOCKS = [
  { 'o:layout': 'html', 'o:data': { html: section(1) } },
  { 'o:layout': 'html', 'o:data': { html: section(2) } },
  { 'o:layout': 'blockGroup', 'o:data': { span: '2' } },
  { 'o:layout': 'html', 'o:data': { html: '<p>Member A text</p>' } },
  { 'o:layout': 'html', 'o:data': { html: '<p>Member B text</p>' } },
  {
    'o:layout': 'html',
    'o:data': { html: `${section(3)}<p><img src="${SLOW_IMAGE}" width="320" height="200" alt="slow"></p>` },
  },
  { 'o:layout': 'html', 'o:data': { html: section(4) } },
  { 'o:layout': 'html', 'o:data': { html: section(5) } },
  { 'o:layout': 'html', 'o:data': { html: section(6) } },
];

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

/** A fresh page through the API, so the spec owns its blocks. Returns the editor hash. */
const createPage = async (page: Page, slug: string): Promise<string> => {
  const sites: unknown = await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json();
  const site = Array.isArray(sites) ? idOf(sites[0]) : idOf(sites);
  const created = await page.request.post('/api/site_pages', {
    data: { 'o:site': { 'o:id': site }, 'o:title': 'Live canvas', 'o:slug': slug, 'o:is_public': true, 'o:block': BLOCKS },
  });
  expect(created.status(), await created.text()).toBe(200);
  return `/#/sites/${String(site)}/pages/${String(idOf(await created.json()))}`;
};

const shown = (page: Page) => page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');

/** The inspector's own heading, not a heading inside the block's HTML. */
const inspectorHeading = (page: Page) =>
  page.getByRole('region', { name: 'Block inspector' }).locator('.block-inspector > h2');

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
  await page.route(`**${SLOW_IMAGE}`, async (route) => {
    await new Promise((resolve) => setTimeout(resolve, IMAGE_DELAY_MS));
    await route.fulfill({ body: PNG, contentType: 'image/png', headers: { 'cache-control': 'no-store' } });
  });
});

test('the preview outlines, selects and inserts, and a re-render keeps the place', async ({ page, diagnostics }) => {
  await page.goto(await createPage(page, 'live-canvas'));

  // Both frames are script-free: the sandbox is exactly `allow-same-origin`.
  const frames = page.locator('iframe.live-frame-doc');
  await expect(frames).toHaveCount(2);
  for (const frame of await frames.all()) {
    expect(await frame.getAttribute('sandbox')).toBe('allow-same-origin');
  }
  await expect(shown(page).locator('[data-romeka-block]')).toHaveCount(BLOCKS.length);
  // The overlay's chips stay out of the tab order and the accessibility tree,
  // and the editor meets the zero serious and critical budget every page does.
  // Legacy mode, because the shared helper's frame pass cannot open its helper
  // page beside a sandboxed frame; the frames hold the public page, which the
  // accessibility spec scans on its own.
  await diagnostics.idle();
  const axe = await new AxeBuilder({ page })
    .withTags([...AXE_TAGS])
    .exclude('iframe')
    .setLegacyMode(true)
    .analyze();
  const faults = axe.violations.filter(
    (violation) => violation.impact === 'serious' || violation.impact === 'critical',
  );
  expect(faults.map((violation) => `${violation.id}: ${violation.nodes.map((node) => node.target.join(' ')).join(', ')}`)).toEqual([]);

  // Hovering a block outlines it with its label; nothing is requested.
  await shown(page).getByText('Section 2 heading').hover();
  await expect(page.locator('.live-outline--hover .live-label')).toHaveText('Text');

  // A link-free click selects the block, and the inspector shows its form.
  await shown(page).getByText('Section 2 heading').click();
  await expect(page.locator('.live-outline--selected')).toBeVisible();
  await expect(inspectorHeading(page)).toHaveText(`Text, block 2 of ${String(BLOCKS.length)}`);
  await expect(page.getByRole('textbox', { name: 'Text', exact: true })).toContainText('Section 2 heading');

  // A group member outlines on its own; the group outlines as a group.
  await shown(page).getByText('Member A text').hover();
  await expect(page.locator('.live-outline--hover .live-label')).toHaveText('Text');

  // Scroll the frame, select a block lower down, and edit it.
  const top = (name: string | RegExp) =>
    shown(page)
      .getByRole('heading', { name })
      .evaluate((element) => element.getBoundingClientRect().top);
  await shown(page)
    .locator('html')
    .evaluate((element) => {
      element.ownerDocument.defaultView?.scrollTo(0, 900);
    });
  await shown(page).getByText('Section 5 heading').click();
  await expect(inspectorHeading(page)).toHaveText(`Text, block 8 of ${String(BLOCKS.length)}`);
  const before = await top('Section 5 heading');

  const rendered = page.waitForResponse(
    (response) => response.url().includes(`/s/${SITE_SLUG}/preview`) && response.request().method() === 'POST',
  );
  // The caret starts at the head of the editor, so the words land before the heading text.
  await page.getByRole('textbox', { name: 'Text', exact: true }).pressSequentially('Edited ', { delay: 10 });
  await rendered;
  // The swap waits for the new document's `load`, which waits for its slow
  // image, so the reader is still on the old document for that time.
  const answered = Date.now();
  await expect(shown(page).getByText('Edited Section 5 heading')).toHaveCount(0);
  await expect(shown(page).getByText('Edited Section 5 heading')).toHaveCount(1, { timeout: 10_000 });
  expect(Date.now() - answered).toBeGreaterThan(IMAGE_DELAY_MS / 2);
  expect(Math.abs((await top('Edited Section 5 heading')) - before)).toBeLessThan(2);
  await expect(page.locator('.live-outline--selected .live-label')).toHaveText('Text');

  // The insert point before block 2 opens the picker at index 1.
  await shown(page).locator('html').evaluate((element) => {
    element.ownerDocument.defaultView?.scrollTo(0, 0);
  });
  await page.locator('.live-slot-button[data-insert-index="1"]').click();
  const picker = page.getByRole('dialog', { name: 'Insert a block or template' });
  await picker.getByRole('button', { name: 'Line break', exact: true }).click();
  await expect(inspectorHeading(page)).toHaveText(`Line break, block 2 of ${String(BLOCKS.length + 1)}`);
  await expect(page.getByRole('listitem', { name: /^Block 2: Line break/ })).toBeVisible();
  await expect(shown(page).locator('[data-romeka-block]')).toHaveCount(BLOCKS.length + 1);

  // The group's own insert point, at its bottom edge, appends to the group.
  await shown(page).getByText('Member B text').scrollIntoViewIfNeeded();
  await page.locator('.live-slot-button[data-insert-group]').click();
  await picker.getByRole('button', { name: 'Line break', exact: true }).click();
  await expect(inspectorHeading(page)).toHaveText(`Line break, block 7 of ${String(BLOCKS.length + 2)}`);
  await expect(page.getByRole('listitem', { name: /^Block 7: Line break/ })).toBeVisible();
});

test('Fit lays the page out at a desktop width scaled into the panel, and the outlines stay on their blocks', async ({ page }) => {
  await page.goto(await createPage(page, 'live-canvas-fit'));
  await expect(page.getByRole('button', { name: 'Fit', pressed: true })).toBeVisible();
  await expect(shown(page).locator('[data-romeka-block]')).toHaveCount(BLOCKS.length);

  // A panel narrower than a desktop would show the phone header; Fit lays out wider and scales.
  expect(await shown(page).locator('html').evaluate((element) => element.ownerDocument.defaultView?.innerWidth)).toBeGreaterThanOrEqual(1024);
  await expect(shown(page).locator('.nav-menu-toggle')).toBeHidden();
  const frame = await page.locator('iframe.live-frame-doc:not(.live-frame-doc--back)').boundingBox();
  const panel = await page.locator('.live-overlay').boundingBox();
  if (frame === null || panel === null) {
    throw new Error('expected the frame and the overlay on screen');
  }
  expect(frame.width).toBeLessThanOrEqual(panel.width + 1);

  // The hover outline covers the block's scaled box, so the overlay and the clicks share coordinates.
  const block = shown(page).locator('[data-romeka-block]', { hasText: 'Section 2 heading' }).last();
  await shown(page).getByText('Section 2 heading').hover();
  const drawn = await page.locator('.live-outline--hover').boundingBox();
  const target = await block.boundingBox();
  if (drawn === null || target === null) {
    throw new Error('expected the outline and its block on screen');
  }
  for (const side of ['x', 'y', 'width', 'height'] as const) {
    expect(Math.abs(drawn[side] - target[side]), side).toBeLessThan(2);
  }
  await shown(page).getByText('Section 2 heading').click();
  await expect(inspectorHeading(page)).toHaveText(`Text, block 2 of ${String(BLOCKS.length)}`);
});

test('a page set to a grid takes its column count on the outline and in the preview', async ({ page }) => {
  await page.goto(await createPage(page, 'live-canvas-grid'));
  const settings = page.getByRole('region', { name: 'Block inspector' });
  await expect(settings.getByRole('heading', { name: 'Page settings' })).toBeVisible();
  await expect(settings.getByLabel('Columns')).toHaveCount(0);

  await settings.getByLabel('Layout', { exact: true }).selectOption('grid');
  const columns = settings.getByRole('group', { name: 'Grid' }).getByLabel('Columns');
  await expect(columns).toHaveAccessibleDescription('How many columns the grid has.');
  await columns.fill('3');

  await expect(page.locator('.canvas-list.canvas-grid')).toHaveCSS(
    'grid-template-columns',
    /^(\S+ ){2}\S+$/,
  );
  await expect(shown(page).locator('.page-layout-grid.grid-template-columns-3')).toHaveCount(1, {
    timeout: 10_000,
  });

  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  const response = await page.goto(`/s/${SITE_SLUG}/page/live-canvas-grid`);
  expect(response?.status()).toBe(200);
  await expect(page.locator('.page-layout-grid.grid-template-columns-3')).toHaveCount(1);
});
