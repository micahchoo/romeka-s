/**
 * The public site the admin manages renders on the port's `/s` routes.
 *
 * The slug comes off the admin sites table, the way a user copies it, and the
 * page is then requested from the same origin the SPA is served from. The port
 * renders the theme's HTML there, so this is the one spec that does not run the
 * SPA at all.
 */
import type { Page } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';
import { png } from '../harness/png';
import { settled } from '../harness/preview';
import { readHarness } from '../harness/state';

test('a media page shows the whole file, never a 4:3 crop of it', async ({ page }) => {
  // The seeded items are not attached to the site, so the browse list is empty;
  // the item page still renders, and the whole first file opens its media page.
  const items = await page.request.get('/api/items?per_page=1');
  const listed: unknown = await items.json();
  if (!isArray(listed) || listed.length === 0) {
    throw new Error('/api/items returned no item');
  }
  const itemId = numberField(record(listed[0], 'item'), 'o:id', 'item');
  await page.goto(`/s/${SITE_SLUG}/item/${String(itemId)}`);
  // One file: it is the whole page, with no strip under it.
  await expect(page.locator('.media-embeds .media-render a')).toHaveCount(1);
  await expect(page.locator('.media-files')).toHaveCount(0);
  const file = page.locator('.media-embeds .media-render a').first();
  await expect(file).toBeVisible();
  await file.click();
  await expect(page).toHaveURL(/\/media\/\d+$/);
  // The upload has no title, so the heading and the document title name it by
  // its source file.
  await expect(page.getByRole('heading', { level: 2, name: 'sample.png' })).toBeVisible();
  await expect(page).toHaveTitle(/^sample\.png · /);
  await expect(page.locator('.media-position')).toHaveCount(0);

  const image = page.locator('.media-page .media-render img').first();
  await expect(image).toBeVisible();
  const shown = await image.evaluate((node) => {
    const computed = getComputedStyle(node);
    return {
      fit: computed.objectFit,
      height: node.getBoundingClientRect().height,
    };
  });
  // Contained rather than cover-cropped, and capped at 80% of the viewport so
  // a tall file fits the screen whole.
  expect(shown.fit).toBe('contain');
  expect(shown.height).toBeLessThanOrEqual((page.viewportSize()?.height ?? 720) * 0.8 + 1);
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(overflow).toBe(0);
});

test('the public site renders the page the admin shows', async ({ page }) => {
  await page.goto('/#/sites');
  const row = page.getByRole('row', { name: new RegExp(SITE_TITLE) });
  await expect(row).toBeVisible();
  await expect(row.getByRole('cell', { name: SITE_SLUG, exact: true })).toBeVisible();

  // The site index answers HTML for the origin the admin serves from.
  const index = await page.goto(`/s/${SITE_SLUG}`);
  expect(index?.status()).toBe(200);
  expect(index?.headers()['content-type']).toBe('text/html; charset=utf-8');

  // The welcome page renders its banner; its unwritten introduction reaches no visitor.
  const welcome = await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  expect(welcome?.status()).toBe(200);
  await expect(page.locator('.romeka-hero-heading')).toHaveText('Welcome');
  await expect(page.getByText('This is the first page of your site.')).toHaveCount(0);
  await expect(page.getByText('Powered by Romeka S')).toBeVisible();
});

/** An item with one uploaded 1600 by 1000 picture, created through the API, its preview made. */
const createPicture = async (page: Page): Promise<{ item: number; media: number }> => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'o:media': [{ 'o:ingester': 'upload', 'o:is_public': true, 'file_index': 0 }],
      }),
      file: { name: 'notes-picture.png', mimeType: 'image/png', buffer: png(1600, 1000) },
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const document = record(await created.json(), 'item');
  const files = document['o:media'];
  if (!isArray(files) || files.length === 0) {
    throw new Error('the created item returned no media');
  }
  const media = numberField(record(files[0], 'media'), 'o:id', 'media');
  // A note on a region needs the picture's canvas, which exists once the
  // derivative worker has made its preview.
  await settled(page.request, media);
  return { item: numberField(document, 'o:id', 'item'), media };
};

/** Write one image note through the Web Annotation Protocol, as the admin's key does. */
const addImageNote = async (
  page: Page,
  item: number,
  media: number,
  text: string,
  selector: string | null,
): Promise<number> => {
  const harness = readHarness();
  const canvas = `${harness.baseUrl}/iiif/3/item/${String(item)}/canvas/${String(media)}`;
  const target =
    selector === null
      ? canvas
      : {
          type: 'SpecificResource',
          source: { id: canvas, type: 'Canvas' },
          selector: {
            type: 'FragmentSelector',
            conformsTo: 'http://www.w3.org/TR/media-frags/',
            value: selector,
          },
        };
  const answer = await page.request.post(
    `${harness.baseUrl}/annotations/media/${String(media)}/?key_identity=${harness.keyIdentity}&key_credential=${harness.keyCredential}`,
    {
      headers: { 'content-type': 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"' },
      data: {
        '@context': 'http://www.w3.org/ns/anno.jsonld',
        type: 'Annotation',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: text, format: 'text/plain' },
        target,
      },
    },
  );
  expect(answer.status(), await answer.text()).toBe(201);
  const document: unknown = await answer.json();
  const iri = record(document, 'annotation')['id'];
  if (typeof iri !== 'string') {
    throw new Error('the annotation answered no id');
  }
  const id = Number(iri.split('/').pop());
  if (!Number.isInteger(id)) {
    throw new Error(`the annotation id is not a row id: ${iri}`);
  }
  return id;
};

test('the media page numbers a note on the picture and lists it below', async ({ page, diagnostics }) => {
  const { item, media } = await createPicture(page);
  const whole = await addImageNote(page, item, media, 'A whole-file note', null);
  const region = await addImageNote(page, item, media, 'A region note', 'xywh=pixel:400,250,400,250');
  const response = await page.goto(`/s/${SITE_SLUG}/media/${String(media)}`);
  expect(response?.status()).toBe(200);

  await expect(page.locator('.annotation-heading')).toContainText('notes');
  const wholeEntry = page.locator(`#note-${String(whole)}`);
  const regionEntry = page.locator(`#note-${String(region)}`);
  await expect(wholeEntry).toContainText('A whole-file note');
  await expect(wholeEntry.locator('.note-place')).toHaveText('Whole image');
  await expect(regionEntry.locator('.note-place')).toHaveText('Region');
  // A whole-file note comes before every region.
  const order = await page.evaluate(
    (ids: string[]) => {
      const first = ids[0] === undefined ? null : document.getElementById(ids[0]);
      const second = ids[1] === undefined ? null : document.getElementById(ids[1]);
      return first !== null && second !== null ? first.compareDocumentPosition(second) : 0;
    },
    [`note-${String(whole)}`, `note-${String(region)}`],
  );
  // `DOCUMENT_POSITION_FOLLOWING`: the region's entry comes after the whole one.
  expect(order & 4).toBe(4);

  // One marker on the region, positioned by the server: 400/1600 and 250/1000.
  const marker = page.locator(`.note-marker[href="#note-${String(region)}"]`);
  await expect(marker).toHaveCount(1);
  await expect(marker).toHaveAttribute('style', 'left:25.00%;top:25.00%');
  await expect(marker).toHaveAttribute('aria-hidden', 'true');
  await expect(page.locator('.annotation-overlay')).toHaveAttribute('viewBox', '0 0 1600 1000');
  // The SVG holds shapes, never a focusable link.
  await expect(page.locator('.annotation-overlay a')).toHaveCount(0);

  // Hovering an entry marks its region.
  await regionEntry.hover();
  await expect(page.locator(`#region-${String(region)}`)).toHaveClass(/is-active/);
  await expect(regionEntry).toHaveClass(/is-active/);

  await expectAxeBudget(page, 'the media page with notes');

  // The marker lands on the pixels it names, at a phone width and a desktop
  // width: its centre is the region's corner, from the picture's own box.
  const evidence = process.env.EVIDENCE_DIR;
  for (const width of [1280, 375]) {
    await page.setViewportSize({ width, height: 900 });
    const geometry = await page.evaluate((id: string) => {
      const image = document.querySelector('.annotated-frame img');
      const placed = document.querySelector(`.note-marker[href="#note-${id}"]`);
      if (image === null || placed === null) {
        return null;
      }
      const box = image.getBoundingClientRect();
      const marker = placed.getBoundingClientRect();
      return {
        left: marker.left + marker.width / 2 - (box.left + box.width / 4),
        top: marker.top + marker.height / 2 - (box.top + box.height / 4),
      };
    }, String(region));
    expect(geometry, `the marker and the picture at ${String(width)}px`).not.toBeNull();
    expect(Math.abs(geometry?.left ?? 99), `left at ${String(width)}px`).toBeLessThanOrEqual(2);
    expect(Math.abs(geometry?.top ?? 99), `top at ${String(width)}px`).toBeLessThanOrEqual(2);
    if (evidence !== undefined && evidence !== '') {
      await page.screenshot({ path: `${evidence}/public-notes-${String(width)}.png`, fullPage: true });
    }
  }
  await page.setViewportSize({ width: 1280, height: 900 });

  // The toggle hides the shapes without a reload.
  await page.getByLabel('Show notes on the picture').uncheck();
  await expect(page.locator('.annotation-overlay')).toBeHidden();
  await expect(marker).toBeHidden();

  await expectClean(diagnostics, 'the media page with notes');
});

test('without a script a marker still opens its note', async ({ page }) => {
  const { item, media } = await createPicture(page);
  const note = await addImageNote(page, item, media, 'A scriptless note', 'xywh=pixel:400,250,400,250');
  const browser = page.context().browser();
  if (browser === null) {
    throw new Error('the spec needs a browser');
  }
  const harness = readHarness();
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const plain = await context.newPage();
    await plain.goto(`${harness.baseUrl}/s/${SITE_SLUG}/media/${String(media)}`);
    const entry = plain.locator(`#note-${String(note)}`);
    await expect(entry).toContainText('A scriptless note');
    await expect(plain.locator('.notes-toggle')).toBeVisible();
    await plain.locator(`.note-marker[href="#note-${String(note)}"]`).click();
    await expect(plain).toHaveURL(new RegExp(`#note-${String(note)}$`));
    expect(await entry.evaluate((element) => element.matches(':target'))).toBe(true);
  } finally {
    await context.close();
  }
});
