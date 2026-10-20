/**
 * What the content-block specs share: items and pages made through the API, and
 * the editor steps that insert a block and save the page.
 */
import { deflateSync, crc32 } from 'node:zlib';
import type { Page } from '@playwright/test';
import { expect } from './fixtures';
import { SITE_SLUG } from './harness/seed';
import { readHarness } from './harness/state';

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const first = (value: unknown): unknown => (Array.isArray(value) ? value[0] : value);

/** A `width` by `height` PNG in one colour, with a darker band at its foot. */
const png = (width: number, height: number, colour: readonly [number, number, number]): Buffer => {
  const rows: Buffer[] = [];
  for (let y = 0; y < height; y += 1) {
    const row = Buffer.alloc(1 + width * 3);
    const shade = y > height * 0.75 ? 0.5 : 1;
    for (let x = 0; x < width; x += 1) {
      row.set(
        colour.map((channel) => Math.round(channel * shade)),
        1 + x * 3,
      );
    }
    rows.push(row);
  }
  const chunk = (kind: string, body: Buffer): Buffer => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(body.length);
    const typed = Buffer.concat([Buffer.from(kind, 'ascii'), body]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc32(typed));
    return Buffer.concat([length, typed, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
};

const propertyId = async (page: Page, term: string): Promise<number> =>
  idOf(first(await (await page.request.get(`/api/properties?term=${term}`)).json()));

/** A public item with a title, the given literal values and, with a colour,
 * one uploaded public image. */
export const createItem = async (
  page: Page,
  title: string,
  options: {
    readonly colour?: readonly [number, number, number];
    readonly values?: Readonly<Record<string, string>>;
    /** Whether the item joins the seeded site, so the site's browse finds it. */
    readonly onSite?: boolean;
  } = {},
): Promise<number> => {
  const body: Record<string, unknown> = { 'o:is_public': true };
  if (options.onSite === true) {
    const site = idOf(first(await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json()));
    body['o:site'] = [{ 'o:id': site }];
  }
  const literals: Readonly<Record<string, string>> = { 'dcterms:title': title, ...options.values };
  for (const [term, value] of Object.entries(literals)) {
    body[term] = [{ type: 'literal', property_id: await propertyId(page, term), '@value': value }];
  }
  if (options.colour === undefined) {
    const created = await page.request.post('/api/items', { data: body });
    expect(created.status(), await created.text()).toBe(200);
    return idOf(await created.json());
  }
  body['o:media'] = [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }];
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify(body),
      file: { name: `${title}.png`, mimeType: 'image/png', buffer: png(900, 600, options.colour) },
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return idOf(await created.json());
};

/** A fresh public page with one HTML block; returns its editor route. */
export const createPage = async (page: Page, slug: string): Promise<string> => {
  const site = idOf(first(await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json()));
  const created = await page.request.post('/api/site_pages', {
    data: {
      'o:site': { 'o:id': site },
      'o:title': slug,
      'o:slug': slug,
      'o:is_public': true,
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Above the block</p>' } }],
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return `/#/sites/${String(site)}/pages/${String(idOf(await created.json()))}`;
};

/** Insert the block named `label` at the end of the page and return its form. */
export const insertBlock = async (page: Page, label: string) => {
  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  await page
    .getByRole('dialog', { name: 'Insert a block or template' })
    .getByRole('button', { name: label, exact: true })
    .click();
  return page.getByRole('region', { name: 'Block inspector' });
};

export const save = async (page: Page): Promise<void> => {
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
};

export const holdApiKey = async (page: Page): Promise<void> => {
  // The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
  // not key, so the browser holds the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
};
