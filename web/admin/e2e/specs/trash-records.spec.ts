/**
 * The media page's and the set place's delete move the record to the trash, and Undo on the
 * list screen they open puts it back: a media with its file, served again; a set with its three
 * items and the site that shows it and pools its items. `TRASH_SCREENSHOTS` names a folder for
 * the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const SET = 'Trash e2e set';
const MEMBERS = ['Trash e2e member one', 'Trash e2e member two', 'Trash e2e member three'] as const;

/** A 1 by 1 PNG. */
const PNG_BASE64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.TRASH_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `trash-${name}.png`), fullPage: true });
  }
};

const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const getJson = async (request: APIRequestContext, path: string): Promise<unknown> => {
  const response = await request.get(path);
  expect(response.status(), `${path}: ${await response.text()}`).toBe(200);
  const body: unknown = await response.json();
  return body;
};

const titleId = async (request: APIRequestContext): Promise<number> => {
  const found = await getJson(request, '/api/properties?term=dcterms:title');
  if (!isArray(found) || found.length !== 1) {
    throw new Error('/api/properties?term=dcterms:title: expected one member');
  }
  return numberField(record(found[0], 'title'), 'o:id', 'title');
};

const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];

const created = async (request: APIRequestContext, path: string, body: object): Promise<number> => {
  const response = await request.post(path, { data: body });
  expect(response.status(), await response.text()).toBe(200);
  return numberField(record(await response.json(), path), 'o:id', path);
};

/** The ids of the set's items, ascending, as the caller's items list answers them. */
const membersOf = async (request: APIRequestContext, set: number): Promise<readonly number[]> => {
  const list = await getJson(request, `/api/items?item_set_id=${String(set)}&sort_by=id&sort_order=asc`);
  return isArray(list) ? list.map((entry) => numberField(record(entry, 'item'), 'o:id', 'item')) : [];
};

/** The sets a site shows, by id. */
const shownBy = async (request: APIRequestContext, site: number): Promise<readonly number[]> => {
  const doc = record(await getJson(request, `/api/sites/${String(site)}`), 'site');
  const entries = doc['o:site_item_set'];
  return isArray(entries)
    ? entries.map((entry) => numberField(record(record(entry, 'entry')['o:item_set'], 'set'), 'o:id', 'set'))
    : [];
};

test('a media deleted from its page goes to the trash with its file, and Undo serves the file again', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const request = page.request;
  const upload = await request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': literal(await titleId(request), 'Trash e2e scan holder'),
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: { name: 'trash-scan.png', mimeType: 'image/png', buffer: Buffer.from(PNG_BASE64, 'base64') },
    },
  });
  expect(upload.status(), await upload.text()).toBe(200);
  const holder = record(await upload.json(), 'item');
  const mediaList = holder['o:media'];
  if (!isArray(mediaList) || mediaList.length !== 1) {
    throw new Error('the upload made no media');
  }
  const media = numberField(record(mediaList[0], 'media'), 'o:id', 'media');
  const original = record(await getJson(request, `/api/media/${String(media)}`), 'media')['o:original_url'];
  if (typeof original !== 'string') {
    throw new Error('the media has no original');
  }
  expect((await request.get(original)).status(), 'the file is served').toBe(200);

  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);
  await page.getByRole('button', { name: 'Delete this media' }).click();
  const confirm = page.getByRole('region', { name: 'Move to the trash' });
  await expect(confirm.getByText('Move trash-scan.png to the trash?')).toBeVisible();
  await expect(confirm.getByText(/^Its file goes with it: /)).toBeVisible();
  await expect(confirm.getByText(/Undo on the Media screen puts it back on its item with its file\./)).toBeVisible();
  await expect(confirm.getByRole('button', { name: 'Move to the trash' })).toBeFocused();
  await diagnostics.idle();
  await expectAxeBudget(page, 'media trash confirm');
  await shoot(page, 'media-confirm');

  await confirm.getByRole('button', { name: 'Move to the trash' }).click();
  await expect(page).toHaveURL(/#\/media$/);
  const notice = page.locator('.trashed-notice');
  await expect(notice.getByText('Deleted trash-scan.png.')).toBeVisible();
  expect((await request.get(`/api/media/${String(media)}`)).status(), 'the media is gone').toBe(404);
  expect((await request.get(original)).status(), 'the file is no longer served').toBe(404);
  await diagnostics.idle();
  await expectAxeBudget(page, 'media trashed notice');
  await shoot(page, 'media-notice');

  await notice.getByRole('button', { name: 'Undo' }).click();
  await expect(notice.getByRole('link', { name: 'trash-scan.png is back.' })).toHaveAttribute('href', `#/media/${String(media)}`);
  const back = record(await getJson(request, `/api/media/${String(media)}`), 'media');
  expect(record(back['o:item'], 'item')['o:id']).toBe(numberField(holder, 'o:id', 'item'));
  const served = await request.get(original);
  expect(served.status(), 'the file is served again').toBe(200);
  expect(served.headers()['content-type']).toBe('image/png');
  await diagnostics.idle();
  await shoot(page, 'media-back');
  await expectClean(diagnostics, 'media trash');
});

test('a set deleted from its place goes to the trash, and Undo brings back its items and its site', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const request = page.request;
  const title = await titleId(request);
  const set = await created(request, '/api/item_sets', { 'dcterms:title': literal(title, SET), 'o:is_public': true });
  const members: number[] = [];
  for (const name of MEMBERS) {
    members.push(
      await created(request, '/api/items', {
        'dcterms:title': literal(title, name),
        'o:item_set': [{ 'o:id': set }],
        'o:is_public': true,
      }),
    );
  }
  const site = await created(request, '/api/sites', {
    'o:slug': 'trash-e2e',
    'o:title': 'Trash e2e site',
    'o:theme': 'default',
    'o:site_item_set': [{ 'o:item_set': { 'o:id': set } }],
    'o:item_pool': { item_set_id: [set] },
  });
  expect(await membersOf(request, set)).toEqual(members);

  await signIn(page, baseURL);
  await page.goto(`/#/items?set=${String(set)}`);
  const place = page.locator('.set-card');
  await place.getByRole('button', { name: 'Delete this item set' }).click();
  const confirm = page.getByRole('region', { name: 'Move to the trash' });
  await expect(confirm.getByText(`Move ${SET} to the trash?`)).toBeVisible();
  await expect(confirm.getByText('The 3 items stay. They leave this set.')).toBeVisible();
  await expect(confirm.getByText('1 site shows this set. It stops showing it.')).toBeVisible();
  await expect(
    confirm.getByText(/Undo on the Item sets screen puts it back with its items and the sites that show it\./),
  ).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'set trash confirm');
  await shoot(page, 'set-confirm');

  await confirm.getByRole('button', { name: 'Move to the trash' }).click();
  await expect(page).toHaveURL(/#\/item-sets$/);
  const notice = page.locator('.trashed-notice');
  await expect(notice.getByText(`Deleted ${SET}.`)).toBeVisible();
  expect((await request.get(`/api/item_sets/${String(set)}`)).status(), 'the set is gone').toBe(404);
  expect(await membersOf(request, set), 'no item is in it').toEqual([]);
  expect(await shownBy(request, site), 'the site no longer shows it').toEqual([]);
  for (const id of members) {
    expect((await request.get(`/api/items/${String(id)}`)).status(), `item ${String(id)} stays`).toBe(200);
  }
  await diagnostics.idle();
  await expectAxeBudget(page, 'item sets trashed notice');
  await shoot(page, 'set-notice');

  await notice.getByRole('button', { name: 'Undo' }).click();
  await expect(notice.getByRole('link', { name: `${SET} is back.` })).toHaveAttribute('href', `#/items?set=${String(set)}`);
  await expect(page.getByRole('link', { name: SET, exact: true })).toBeVisible();
  expect(await membersOf(request, set), 'its items are back in it').toEqual(members);
  expect(await shownBy(request, site), 'the site shows it again').toEqual([set]);
  const pool = record(await getJson(request, `/api/sites/${String(site)}`), 'site')['o:item_pool'];
  expect(pool, "the site's item pool names it").toEqual({ item_set_id: [set] });
  await diagnostics.idle();
  await shoot(page, 'set-back');
  await expectClean(diagnostics, 'set trash');
});
