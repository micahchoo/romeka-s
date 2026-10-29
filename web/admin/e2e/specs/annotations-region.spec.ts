/**
 * The region editor in a real browser, against the real WAP routes: a rectangle
 * dragged on a picture, a polygon clicked and closed with Enter, the spring-loaded
 * Shift drag, the three-pixel drag that selects instead of drawing, and the
 * keyboard that moves a selected rectangle into the saved note.
 *
 * The picture is `fixtures/media-kinds/image.jpg`, which is 120 by 80 pixels with
 * EXIF Orientation 6, so the canvas its container reports is the upright 80 by
 * 120 (see the corpus README). A drag of a known fraction of the overlay is
 * expected on that canvas, which is what proves the overlay maps the derivative
 * to the upright original rather than to the file's unrotated header: 40% of the
 * height is 48 canvas pixels, not 32.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { isRecord, numberField, record } from '../harness/json';
import { settled } from '../harness/preview';
import { readHarness, REPO_ROOT } from '../harness/state';

/** The upright canvas of `image.jpg`, from the corpus README. */
const CANVAS = { width: 80, height: 120 };

interface Rect {
  readonly kind: 'rect';
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
}

interface Polygon {
  readonly kind: 'polygon';
  readonly points: readonly (readonly [number, number])[];
}

type Shape = Rect | Polygon;

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

/** Upload one corpus file as the only media of a new public item, its preview settled. */
const uploadItem = async (
  request: APIRequestContext,
  name: string,
  mediaType: string,
): Promise<number> => {
  const answer = await request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': name }],
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: { name, mimeType: mediaType, buffer: corpus(name) },
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  const media = firstMediaId(await answer.json(), name);
  // The drawing tools come with the picture's canvas, made with its preview.
  await settled(request, media);
  return media;
};

const firstMediaId = (item: unknown, what: string): number => {
  const media = record(item, what)['o:media'];
  if (!Array.isArray(media) || media.length === 0) {
    throw new Error(`${what}: the created item names no media`);
  }
  const first: unknown = media[0];
  if (!isRecord(first)) {
    throw new Error(`${what}: the media reference is not an object`);
  }
  return numberField(first, 'o:id', 'media');
};

/** Sign the browser in with the seeded administrator's password. */
const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const pair = (x: number, y: number): readonly [number, number] => [x, y];

/** The selector a stored annotation carries, as the server wrote it. */
const shapeOf = (annotation: unknown): Shape | null => {
  const target = record(annotation, 'annotation').target;
  if (!isRecord(target)) {
    return null;
  }
  const selector = target['selector'];
  if (!isRecord(selector)) {
    return null;
  }
  const value = typeof selector['value'] === 'string' ? selector['value'] : '';
  const box = /^xywh=(?:pixel:)?(\d+),(\d+),(\d+),(\d+)$/.exec(value);
  if (box !== null) {
    return {
      kind: 'rect',
      x: Number(box[1]),
      y: Number(box[2]),
      w: Number(box[3]),
      h: Number(box[4]),
    };
  }
  const points = [...value.matchAll(/(\d+),(\d+)/g)].map((match) =>
    pair(Number(match[1]), Number(match[2])),
  );
  return points.length === 0 ? null : { kind: 'polygon', points };
};

/** What the server holds for a media, read from its own page route. */
const shapesOf = async (request: APIRequestContext, media: number): Promise<readonly Shape[]> => {
  const answer = await request.get(`/annotations/media/${String(media)}/page`);
  expect(answer.status(), await answer.text()).toBe(200);
  const items = record(await answer.json(), 'page')['items'];
  if (!Array.isArray(items)) {
    throw new Error('the page names no items');
  }
  return items.flatMap((item) => {
    const shape = shapeOf(item);
    return shape === null ? [] : [shape];
  });
};

/** The canvas point under a fraction of the overlay's own box. */
const at = (
  box: { readonly x: number; readonly y: number; readonly width: number; readonly height: number },
  fx: number,
  fy: number,
): { readonly x: number; readonly y: number } => ({
  x: box.x + box.width * fx,
  y: box.y + box.height * fy,
});

const overlayBox = async (page: Page): Promise<{ x: number; y: number; width: number; height: number }> => {
  const overlay = page.locator('.region-overlay');
  await expect(overlay).toHaveCount(1);
  // The figure is taller than the panel's scroll box, so a fraction of it is
  // only on screen once the page is scrolled to its top.
  await overlay.scrollIntoViewIfNeeded();
  const box = await overlay.boundingBox();
  if (box === null) {
    throw new Error('the overlay has no box');
  }
  return box;
};

/** Drag from one fraction of the overlay to another, in screen pixels. */
const dragOnOverlay = async (
  page: Page,
  from: { readonly x: number; readonly y: number },
  to: { readonly x: number; readonly y: number },
): Promise<void> => {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await page.mouse.up();
};

/** Draw the rectangle this spec expects, and save it with a note. */
const drawRectangle = async (page: Page, text: string): Promise<void> => {
  await page.getByRole('button', { name: 'Rectangle' }).click();
  const box = await overlayBox(page);
  await dragOnOverlay(page, at(box, 0.25, 0.05), at(box, 0.75, 0.3));
  const form = page.locator('.note-form');
  await expect(form).toBeVisible();
  await form.getByLabel('Note').fill(text);
  await form.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-entries')).toContainText(text);
};

test('a rectangle on a turned image is stored on the upright canvas', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'image.jpg', 'image/jpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  // A shape completes in one gesture and hands the tool back.
  await drawRectangle(page, 'A printed stamp.');
  await expect(page.getByRole('button', { name: 'Move' })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.region-draft')).toHaveCount(0);

  const shapes = await shapesOf(page.request, media);
  expect(shapes).toHaveLength(1);
  const shape = shapes[0];
  if (shape?.kind !== 'rect') {
    throw new Error('the saved shape is not a rectangle');
  }
  // 25% to 75% of 80 wide is 20 to 60; 5% to 30% of 120 tall is 6 to 36.
  expect(shape.x).toBeGreaterThanOrEqual(19);
  expect(shape.x).toBeLessThanOrEqual(21);
  expect(shape.w).toBeGreaterThanOrEqual(39);
  expect(shape.w).toBeLessThanOrEqual(41);
  expect(shape.y).toBeGreaterThanOrEqual(5);
  expect(shape.y).toBeLessThanOrEqual(7);
  expect(shape.h).toBeGreaterThanOrEqual(29);
  expect(shape.h).toBeLessThanOrEqual(31);

  // The entry carries the shape's number, and the overlay draws both.
  await expect(page.locator('.note-number')).toHaveText('1');
  await expect(page.locator('.region-shapes [data-number="1"] .region-shape')).toHaveCount(1);
  const drawn = page.locator('.region-shapes [data-number="1"] rect.region-shape');
  await expect(drawn).toHaveAttribute('x', String(shape.x));
  await expect(drawn).toHaveAttribute('height', String(shape.h));
  await expectClean(diagnostics, 'a drawn rectangle');
});

test('a drag shorter than four pixels selects instead of drawing', async ({ page, baseURL }) => {
  const media = await uploadItem(page.request, 'image.jpg', 'image/jpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  await page.getByRole('button', { name: 'Rectangle' }).click();
  const box = await overlayBox(page);
  const from = at(box, 0.5, 0.2);
  await dragOnOverlay(page, from, { x: from.x + 3, y: from.y + 1 });

  await expect(page.locator('.note-form')).toHaveCount(0);
  await expect(page.locator('.region-draft')).toHaveCount(0);
  await expect(page.locator('.media-notes-entries')).toHaveCount(0);
});

test('Shift and drag on Move draws a rectangle without choosing a tool', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'image.jpg', 'image/jpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  const box = await overlayBox(page);
  await page.keyboard.down('Shift');
  await dragOnOverlay(page, at(box, 0.1, 0.1), at(box, 0.4, 0.3));
  await page.keyboard.up('Shift');

  await expect(page.locator('.region-draft')).toHaveCount(1);
  await expect(page.locator('.note-form')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Move' })).toHaveAttribute('aria-pressed', 'true');
  await expectClean(diagnostics, 'a spring-loaded rectangle');
});

test('a click on a shape opens its entry, and the keyboard moves it into the save', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'image.jpg', 'image/jpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);
  await drawRectangle(page, 'A stamp in the corner.');

  // Click the middle of the shape: 20 to 60 wide and 6 to 36 tall.
  const box = await overlayBox(page);
  const centre = at(box, 40 / CANVAS.width, 21 / CANVAS.height);
  await page.mouse.click(centre.x, centre.y);
  await expect(page.locator('.note-entry.is-selected')).toHaveCount(1);
  await expect(page.locator('.note-open')).toHaveAttribute('aria-expanded', 'true');
  await expect(page.locator('.region-shapes g.is-selected .region-shape')).toHaveCount(1);

  const overlay = page.locator('.region-overlay');
  await overlay.focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('.region-shapes g.is-selected .region-shape')).toHaveAttribute('x', '21');

  await page.locator('.note-form').getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-status')).toHaveText('Saved.');

  const shapes = await shapesOf(page.request, media);
  const shape = shapes[0];
  if (shape?.kind !== 'rect') {
    throw new Error('the saved shape is not a rectangle');
  }
  expect(shape.x).toBe(21);
  expect(shape.w).toBe(40);
  await expectClean(diagnostics, 'a nudged rectangle');
});

test('a polygon closes with Enter and is stored with its corners', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'image.jpg', 'image/jpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  await page.getByRole('button', { name: 'Advanced' }).click();
  await page.getByRole('button', { name: 'Polygon' }).click();
  const box = await overlayBox(page);
  const corners = [
    { fx: 0.2, fy: 0.2 },
    { fx: 0.6, fy: 0.2 },
    { fx: 0.4, fy: 0.6 },
  ];
  for (const corner of corners) {
    const point = at(box, corner.fx, corner.fy);
    await page.mouse.click(point.x, point.y);
  }
  await page.locator('.region-overlay').focus();
  await page.keyboard.press('Enter');

  const form = page.locator('.note-form');
  await expect(form).toBeVisible();
  await form.getByLabel('Note').fill('A fold along the edge.');
  await form.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-entries')).toContainText('A fold along the edge.');

  const shapes = await shapesOf(page.request, media);
  const shape = shapes[0];
  if (shape?.kind !== 'polygon') {
    throw new Error('the saved shape is not a polygon');
  }
  expect(shape.points).toHaveLength(3);
  const [first, second, third] = shape.points;
  expect(first?.[0]).toBeGreaterThanOrEqual(15);
  expect(first?.[0]).toBeLessThanOrEqual(17);
  expect(first?.[1]).toBeGreaterThanOrEqual(23);
  expect(first?.[1]).toBeLessThanOrEqual(25);
  expect(second?.[0]).toBeGreaterThanOrEqual(47);
  expect(second?.[0]).toBeLessThanOrEqual(49);
  expect(third?.[1]).toBeGreaterThanOrEqual(71);
  expect(third?.[1]).toBeLessThanOrEqual(73);
  await expectClean(diagnostics, 'a drawn polygon');
});

test('a PDF has no region editor, because its surface is the whole file', async ({
  page,
  baseURL,
}) => {
  const media = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  await expect(page.getByRole('heading', { name: 'Notes', exact: true })).toBeVisible();
  await expect(page.locator('.region-editor, .region-overlay')).toHaveCount(0);
});

test('a reader sees the numbered shapes and holds no tool at all', async ({
  page,
  browser,
  baseURL,
}) => {
  const media = await uploadItem(page.request, 'image.jpg', 'image/jpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);
  await drawRectangle(page, 'A stamp only a reader sees.');

  const anonymous = await browser.newContext({ baseURL });
  const reader = await anonymous.newPage();
  await reader.goto(`/#/media/${String(media)}`);

  await expect(reader.locator('.region-overlay')).toHaveCount(1);
  await expect(reader.locator('.region-shapes [data-number="1"] .region-shape')).toHaveCount(1);
  await expect(reader.locator('.note-entry')).toHaveCount(1);
  // The tools are absent, not disabled: a reader's page holds no write control.
  await expect(reader.locator('.region-toolbar, .region-advanced, .region-hint')).toHaveCount(0);
  await expect(reader.locator('.region-overlay')).toHaveAttribute('role', 'presentation');
  await anonymous.close();
});
