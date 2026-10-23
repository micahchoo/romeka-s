/**
 * Import v2 against the real port: the Simple pass on a BHC_006-like folder
 * (the files, the Structure of its folders, one example marked on a folder,
 * the list round trip and the item type from that folder's node, the check,
 * the start, the progress, the end, then an undo that keeps the one
 * item edited since and undoes it anyway), an Advanced edit read in Simple, the
 * fields a chosen list's headers name, a column's field kept across a click on another folder, a column's field
 * saved from Advanced's Lists section, a folder's item type from a column, the pairs list round trip, a zip sent from this computer, and a delete the
 * server refuses until its count is typed.
 *
 * The folders are `e2e/harness/archive.ts`'s. Every import job is a real
 * `romeka job run` child of the port, so each wait below is for the job.
 */
import type { Download, Locator, Page } from '@playwright/test';
import { readFileSync } from 'node:fs';

import { expect, test } from '../fixtures';
import { ARTIFACTS, BHC, PAIRS, PAIRS_COUNT, PHOTOGRAPHS, pairsFile, uploadJpeg, zipOf } from '../harness/archive';
import { isArray, numberField, parseJson, record } from '../harness/json';

/**
 * The items Artifacts makes once its example is marked: three objects, their
 * front and back one item, and the register's two rows that name no file of
 * the folder (the photographs'), each an item of its own.
 */
const ARTIFACT_ITEMS = 5;

/** Artifacts' objects, the items with files: the rows of its list to fill in. */
const OBJECTS = 3;

/** The items BHC_006's Structure makes: Artifacts' and the two photographs; the note, which is no picture, is not imported. */
const BHC_ITEMS = ARTIFACT_ITEMS + 2;

/** A job takes a process start and a database write per record. */
const JOB_MS = 60_000;

test.describe.configure({ mode: 'serial', timeout: 240_000 });
test.use({ actionTimeout: 15_000 });

const api = async (page: Page, method: 'GET' | 'POST' | 'PATCH', path: string, body?: unknown): Promise<{ readonly status: number; readonly json: unknown }> => {
  const response = await page.request.fetch(path, { method, ...(body === undefined ? {} : { data: body }) });
  const text = await response.text();
  if (!(response.headers()['content-type'] ?? '').includes('json')) {
    throw new Error(`${method} ${path}: ${String(response.status())} ${text}`);
  }
  return { status: response.status(), json: text === '' ? null : parseJson(text) };
};

const importId = (page: Page): number => {
  const match = /#\/import\/(\d+)\//.exec(page.url());
  if (match?.[1] === undefined) {
    throw new Error(`not on an import: ${page.url()}`);
  }
  return Number(match[1]);
};

const next = async (page: Page): Promise<void> => {
  await page.getByRole('link', { name: 'Next', exact: true }).click();
};

/** Start a draft over one folder of the archive root, from the start page, which opens it on Structure. */
const startOn = async (page: Page, folder: string): Promise<number> => {
  await page.goto('/#/import');
  await page.getByRole('button', { name: folder, exact: true }).click();
  await page.getByRole('button', { name: `Use the folder ${folder}` }).click();
  await expect(page).toHaveURL(/#\/import\/\d+\/simple\/structure$/);
  return importId(page);
};

const saved = async (page: Page): Promise<void> => {
  await expect(page.getByRole('status').filter({ hasText: /^Saved$/ })).toBeVisible({ timeout: 10_000 });
};

/** A count the screen's one line of facts states, such as "Items 6": its label, then the number. */
const fact = (page: Page, label: string, count: number): Locator => page.getByText(new RegExp(`^${label}\\s+${String(count)}$`, 'u'));

/** Structure's suggestion used as it is: the plan then holds each folder with the list the proposer chose for it. */
const useStructure = async (page: Page): Promise<void> => {
  await page.getByRole('button', { name: 'Use this structure' }).click();
  await expect(page.getByText(/^In use\. Your changes: \d+\.$/u)).toBeVisible();
  await saved(page);
};

const downloaded = async (download: Download): Promise<string> => {
  const path = await download.path();
  return readFileSync(path, 'utf8');
};

const itemByTitle = async (page: Page, title: string): Promise<number> => {
  const query = new URLSearchParams({
    'property[0][property]': 'dcterms:title',
    'property[0][type]': 'eq',
    'property[0][text]': title,
  });
  const { json } = await api(page, 'GET', `/api/items?${query.toString()}`);
  if (!isArray(json) || json[0] === undefined) {
    throw new Error(`no item titled ${title}`);
  }
  return numberField(record(json[0], title), 'o:id', title);
};

/** The headers of the columns Artifacts' item rule for `slot` reads, in the stored plan. */
const itemRuleHeaders = async (page: Page, id: number, slot: string): Promise<readonly unknown[]> => {
  const plan = record(record((await api(page, 'GET', `/api/romeka/imports/${String(id)}`)).json, 'import')['plan'], 'plan');
  const levels = record(plan['structure'], 'structure')['levels'];
  const artifacts = (isArray(levels) ? levels : []).map((level) => record(level, 'level')).find((level) => level['id'] === ARTIFACTS);
  const rule = record(record(record(artifacts?.['slots'], 'slots')['item'], 'item')[slot], slot);
  const sources = isArray(rule['sources']) ? rule['sources'] : [];
  return sources.map((source) => record(record(source, 'source')['column'], 'column')['header']);
};

test('the Simple pass adds the folder, and undo keeps the item edited since until asked', async ({ page }) => {
  const id = await startOn(page, BHC);

  // One example, marked on Artifacts: front and back are parts of one object, and the folder is written by hand.
  await page.getByRole('option', { name: ARTIFACTS }).click();
  await page.getByRole('button', { name: 'Mark an example' }).click();
  await page.getByRole('searchbox', { name: 'Find a file name' }).fill('_front');
  await page.getByRole('button', { name: 'Find' }).click();
  await page.getByRole('radio', { name: 'BHC_006_AF_001_front.jpg' }).click();
  const parts = page.getByRole('group', { name: 'Parts of the name BHC_006_AF_001_front.jpg' });
  await parts.getByRole('button', { name: 'front' }).click();
  const back = page.getByRole('checkbox', { name: /^back / });
  if (!(await back.isChecked())) {
    await back.check();
  }
  await expect(page.getByRole('status').filter({ hasText: `Written this way: ${String(ARTIFACT_ITEMS)} items.` })).toBeVisible();
  await page.getByRole('button', { name: 'Use this', exact: true }).click();
  await expect(page.getByRole('option', { name: ARTIFACTS })).toContainText('written by you');
  await saved(page);

  // The register's titles need no click: the list the proposer chose puts its Title column in Title.
  await page.getByRole('button', { name: 'Where each column goes' }).click();
  await expect(page.getByRole('combobox', { name: 'Where the column Title goes' })).toHaveValue('field:dcterms:title');
  await page.getByRole('button', { name: 'Where each column goes' }).click();

  // The folder's list round trip, from its node.
  await page.getByRole('button', { name: 'Fill in a list yourself' }).click();
  const trip = page.getByRole('region', { name: 'Fill in a list yourself' });
  const [template] = await Promise.all([
    page.waitForEvent('download'),
    trip.getByRole('link', { name: /^Download the list \(/ }).click(),
  ]);
  const rows = (await downloaded(template)).replace(/^\uFEFF/, '').trim().split(/\r?\n/);
  expect(rows[0]).toMatch(/^Identifier,Files/);
  expect(rows.length).toBe(OBJECTS + 1);
  const header = `${rows[0] ?? ''},Notes [dcterms:description]`;
  const filled = [header, ...rows.slice(1).map((row) => `${row},Seen on the shelf`)].join('\r\n');
  await trip.getByLabel('Choose the file').setInputFiles({ name: 'filled.csv', mimeType: 'text/csv', buffer: Buffer.from(filled) });
  await expect(page.getByText(`Read ${String(OBJECTS)} rows from filled.csv.`)).toBeVisible();
  await expect(page.getByText(`${String(OBJECTS)} belong to items of this import.`)).toBeVisible();

  // Where the folder's item type comes from, on the same node; then the check's suggested answers.
  await expect(page.getByRole('combobox', { name: 'The item type comes from' })).toBeVisible();
  await page.getByRole('list', { name: 'Steps' }).getByRole('link', { name: 'Check' }).click();
  await expect(fact(page, 'Items to add', BHC_ITEMS)).toBeVisible();
  // The register's two rows that name no file and the photographs with no row: the suggested answers keep both.
  await page.getByRole('button', { name: /^Use the (?:\d+ suggested answers|1 suggested answer)$/u }).click();
  await expect(fact(page, 'Questions open', 0)).toBeVisible();
  await saved(page);
  await next(page);

  // Add: Simple's summary over the run screen's start, then the progress and the end.
  await expect(page).toHaveURL(new RegExp(`#/import/${String(id)}/simple/add$`));
  await expect(page.getByRole('definition').filter({ hasText: /^Add new items/ })).toBeVisible();
  const add = page.getByRole('button', { name: `Add ${String(BHC_ITEMS)} items to Omeka` });
  await expect(add).toBeEnabled();
  await add.click();
  await expect(page.getByText('The import is done.')).toBeVisible({ timeout: JOB_MS });

  const lamp = await itemByTitle(page, 'Brass lamp');
  const edited = await api(page, 'PATCH', `/api/items/${String(lamp)}`, { 'o:is_public': true });
  expect(edited.status).toBe(200);

  // Undo keeps the edited item, its two files and the folder's item set that holds it; then they are undone anyway.
  await page.getByRole('button', { name: 'Undo this import...' }).click();
  await expect(page.getByText(/^4 are kept: since the import/)).toBeVisible();
  await expect(page.getByRole('link', { name: `Item ${String(lamp)}` })).toBeVisible();
  await page.getByRole('button', { name: 'Undo the import' }).click();
  await expect(page.getByText('Partly undone.')).toBeVisible({ timeout: JOB_MS });
  await expect(page.getByText(/^4 were kept/)).toBeVisible();
  expect((await api(page, 'GET', `/api/items/${String(lamp)}`)).status).toBe(200);
  await page.getByRole('checkbox', { name: 'Undo all 4 anyway' }).check();
  await expect(page.getByText(/^Undoing 4 anyway also undoes/)).toBeVisible();
  await page.getByRole('button', { name: 'Undo the import' }).click();
  await expect(page.getByText('Undone.', { exact: true })).toBeVisible({ timeout: JOB_MS });
  expect((await api(page, 'GET', `/api/items/${String(lamp)}`)).status).toBe(404);
});

test('an answer given in Advanced shows in Simple', async ({ page }) => {
  const id = await startOn(page, BHC);
  await page.getByRole('link', { name: 'Advanced', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`#/import/${String(id)}/advanced/`));
  // The old Files address opens Structure at the first folder, its Items section open.
  await page.goto(`/#/import/${String(id)}/advanced/files`);
  const regex = page.getByLabel('Regex', { exact: true });
  await regex.fill('^(?:[^/]*/)*(?<identifier>BHC_006_[A-Z]+_\\d+).*$');
  await page.getByRole('button', { name: 'Use this pattern' }).click();
  await saved(page);
  // A typed pattern writes the folder by hand, and Simple's Structure says so.
  await page.getByRole('link', { name: 'Simple', exact: true }).click();
  await expect(page.getByRole('option', { name: 'Artifacts' })).toContainText('5 items');
  await expect(page.getByRole('option', { name: 'Artifacts' })).toContainText('written by you');
});

test('a list Structure’s suggestion chooses gives each column whose header is a field’s label that field', async ({ page }) => {
  const id = await startOn(page, BHC);
  await useStructure(page);
  expect(await itemRuleHeaders(page, id, 'dcterms:title')).toEqual(['Title']);
  expect(await itemRuleHeaders(page, id, 'dcterms:date')).toEqual(['Date']);
});

test('a column given its field on a folder’s node survives a click on another folder', async ({ page }) => {
  const id = await startOn(page, BHC);
  await page.goto(`/#/import/${String(id)}/simple/structure`);
  await page.getByRole('option', { name: ARTIFACTS }).click();
  await page.getByRole('button', { name: 'Where each column goes' }).click();
  const date = page.getByRole('combobox', { name: 'Where the column Date goes' });
  await expect(date.getByRole('option', { name: 'Date Submitted' })).toBeAttached();
  await date.selectOption('field:dcterms:dateSubmitted');
  await saved(page);

  // A click on another folder saves the structure the proposer sends back: the column's rule is in it.
  await page.getByRole('option', { name: PHOTOGRAPHS }).click();
  await page.getByRole('button', { name: /^Leave out/ }).click();
  await expect(page.getByRole('option', { name: PHOTOGRAPHS })).toContainText('you left it out');
  await saved(page);
  expect(await itemRuleHeaders(page, id, 'dcterms:dateSubmitted')).toEqual(['Date']);

  await page.getByRole('option', { name: ARTIFACTS }).click();
  await page.getByRole('button', { name: 'Where each column goes' }).click();
  await expect(page.getByRole('combobox', { name: 'Where the column Date goes' })).toHaveValue('field:dcterms:dateSubmitted');
});

test('a column given another field in Advanced’s Lists section is saved', async ({ page }) => {
  const id = await startOn(page, BHC);
  await page.goto(`/#/import/${String(id)}/advanced/structure`);
  await page.getByRole('option', { name: ARTIFACTS }).click();
  await page.getByRole('button', { name: 'Lists', exact: true }).click();
  const date = page.getByRole('combobox', { name: 'Field for Date' });
  await expect(date.getByRole('option', { name: 'Date Submitted' })).toBeAttached();
  await date.selectOption('dcterms:dateSubmitted');
  await saved(page);
  expect(await itemRuleHeaders(page, id, 'dcterms:dateSubmitted')).toEqual(['Date']);
});

test('a folder’s item type from a column of its list is saved, and its values are counted', async ({ page }) => {
  const id = await startOn(page, BHC);
  await page.goto(`/#/import/${String(id)}/simple/structure`);
  await page.getByRole('option', { name: ARTIFACTS }).click();
  const from = page.getByRole('combobox', { name: 'The item type comes from' });
  await expect(from.getByRole('option', { name: 'Column “Title”' })).toBeAttached();
  await from.selectOption({ label: 'Column “Title”' });
  await saved(page);
  await expect(page.getByRole('region', { name: 'Item type for each value' }).getByRole('combobox', { name: /^Item type for / }).first()).toBeVisible();
  expect(await itemRuleHeaders(page, id, 'template')).toEqual(['Title']);
});

test('the pairs list pairs every row of a register that names no file', async ({ page }) => {
  const id = await startOn(page, PAIRS);
  await useStructure(page);
  await page.goto(`/#/import/${String(id)}/simple/pair`);
  const card = page.getByRole('region', { name: `${String(PAIRS_COUNT)} rows have no file: pair them in a spreadsheet` });
  await expect(card).toBeVisible();
  const [list] = await Promise.all([
    page.waitForEvent('download'),
    card.getByRole('link', { name: `Download the ${String(PAIRS_COUNT)} rows (.csv)` }).click(),
  ]);
  const lines = (await downloaded(list)).replace(/^\uFEFF/, '').trim().split(/\r?\n/);
  expect(lines).toHaveLength(PAIRS_COUNT + 1);
  const columns = (lines[0] ?? '').split(',');
  const files = columns.indexOf('Files');
  expect(files).toBeGreaterThanOrEqual(0);
  const filled = [
    lines[0],
    ...lines.slice(1).map((line, at) => {
      const cells = line.split(',');
      cells[files] = pairsFile(at + 1);
      return cells.join(',');
    }),
  ].join('\r\n');
  await card.getByLabel('Choose the file').setInputFiles({ name: 'pairs.csv', mimeType: 'text/csv', buffer: Buffer.from(filled) });
  await expect(page.getByText(`Paired ${String(PAIRS_COUNT)} rows.`)).toBeVisible();
  await expect(page.getByRole('heading', { name: `Pairs you made (${String(PAIRS_COUNT)})` })).toBeVisible();
  const { json } = await api(page, 'GET', `/api/romeka/imports/${String(id)}`);
  const decisions = record(record(json, 'import')['plan'], 'plan')['decisions'];
  expect(isArray(decisions) ? decisions.filter((one) => record(one, 'decision')['kind'] === 'pair').length : 0).toBe(PAIRS_COUNT);
});

test('a zip from this computer is unpacked into the import and counted', async ({ page }) => {
  await page.goto('/#/import');
  await page.getByRole('button', { name: 'Start with files from this computer' }).click();
  await expect(page).toHaveURL(/#\/import\/\d+\/simple\/files$/);
  const zip = zipOf({ 'box/ZP_001_front.jpg': uploadJpeg('box/ZP_001_front.jpg'), 'box/ZP_002_front.jpg': uploadJpeg('box/ZP_002_front.jpg') });
  await page.locator('input[type="file"][multiple]').setInputFiles({ name: 'box.zip', mimeType: 'application/zip', buffer: zip });
  await page.getByText('Sent: 1', { exact: true }).click({ timeout: JOB_MS });
  await expect(page.getByText('Unpacked. Its files are added.')).toBeVisible({ timeout: JOB_MS });
  await expect(fact(page, 'Files', 2)).toBeVisible({ timeout: JOB_MS });
});

/** A draft over BHC_006 with its register read and joined, and its titles in Title, as Structure's suggestion leaves it. */
const listed = async (page: Page): Promise<number> => {
  const id = await startOn(page, BHC);
  await useStructure(page);
  await expect
    .poll(async () => {
      const { json } = await api(page, 'GET', `/api/romeka/imports/${String(id)}`);
      const levels = record(record(record(json, 'import')['plan'], 'plan')['structure'], 'structure')['levels'];
      const lists = (isArray(levels) ? levels : []).map((level) => record(level, 'level')['lists']);
      return lists.reduce<number>((sum, one) => sum + (isArray(one) ? one.length : 0), 0);
    }, { timeout: 15_000 })
    .toBe(1);
  return id;
};

test('a delete is refused until its count is typed', async ({ page }) => {
  // The items to delete: one import of the folder, added.
  const first = await listed(page);
  await page.goto(`/#/import/${String(first)}/advanced/run`);
  await page.getByRole('button', { name: 'Run the import' }).click();
  await expect(page.getByText('The import is done.')).toBeVisible({ timeout: JOB_MS });

  // A delete matches by Omeka id only (it takes no field rules), so a second
  // import names two of them by file names that are their ids.
  const doomed = [await itemByTitle(page, 'Brass lamp'), await itemByTitle(page, 'Tin box')];
  await page.goto('/#/import');
  await page.getByRole('button', { name: 'Start with files from this computer' }).click();
  await expect(page).toHaveURL(/#\/import\/\d+\/simple\/files$/);
  const id = importId(page);
  await page
    .locator('input[type="file"][multiple]')
    .setInputFiles(doomed.map((one) => ({ name: `${String(one)}.jpg`, mimeType: 'image/jpeg', buffer: uploadJpeg(String(one)) })));
  await expect(fact(page, 'Files', 2)).toBeVisible({ timeout: JOB_MS });
  await page.goto(`/#/import/${String(id)}/advanced/existing`);
  await page.getByRole('combobox', { name: 'Match key for Items' }).selectOption('internalId');
  await page.getByRole('combobox', { name: 'Scope for Items' }).selectOption('all');
  await saved(page);
  await page.getByRole('radio', { name: /^Delete\./ }).check();
  const lock = page.getByText(/^Type \d+ to choose delete$/);
  const count = /\d+/.exec((await lock.textContent()) ?? '')?.[0] ?? '';
  expect(Number(count)).toBeGreaterThan(0);
  await page.getByLabel(`Type ${count} to choose delete`).fill(count);
  await page.getByRole('button', { name: /^Choose delete: \d+ records$/ }).click();
  await saved(page);

  const check = record(record((await api(page, 'POST', `/api/romeka/imports/${String(id)}/preview`, { include: ['check'] })).json, 'preview')['check'], 'check');
  expect(check['deletes']).toBeGreaterThan(0);
  const refused = await api(page, 'POST', `/api/romeka/imports/${String(id)}/run`, { digest: check['digest'] });
  expect(refused.status).toBe(409);
  expect(record(refused.json, 'refusal')['code']).toBe('confirm');

  await page.goto(`/#/import/${String(id)}/advanced/run`);
  const go = page.getByRole('button', { name: 'Delete them from Omeka' });
  await expect(go).toBeDisabled();
  await page.getByLabel(`Type ${String(check['deletes'])} to confirm`).fill(String(check['deletes']));
  await expect(go).toBeEnabled();
  await go.click();
  await expect(page.getByText('The import is done.')).toBeVisible({ timeout: JOB_MS });
  for (const one of doomed) {
    expect((await api(page, 'GET', `/api/items/${String(one)}`)).status).toBe(404);
  }

  // The specs after this one share the database: the rest of the first import goes too.
  await page.goto(`/#/import/${String(first)}/advanced/run`);
  await page.getByRole('button', { name: 'Undo this import...' }).click();
  await page.getByRole('button', { name: 'Undo the import' }).click();
  await expect(page.getByText('Undone.', { exact: true })).toBeVisible({ timeout: JOB_MS });
});
