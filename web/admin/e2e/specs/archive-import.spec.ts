/**
 * The archive import against the real server and a real job process. The
 * harness starts `romeka serve` with `ROMEKA_ARCHIVE_ROOT` on the fixture
 * folder in `harness/archive.ts`, so the Import page is on, and each run or
 * undo is a `romeka job run` child the server starts.
 *
 * The first test drives the seven steps as a user would, imports, finds the
 * items and their thumbnails, and undoes the import. The second shows that a
 * later import closes the earlier one's undo window. Every name a test makes
 * carries the test's own tag, so a repeat in one run makes new rows.
 */
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import type { APIRequestContext, Locator, Page } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { ARCHIVE_FOLDER, CSV_FILE, CSV_TITLES, LETTERS, PHOTOS } from '../harness/archive';
import { isArray, isRecord } from '../harness/json';
import { readHarness } from '../harness/state';

/** A job is a child process; a run of four items ends in a few seconds. */
const JOB_TIMEOUT_MS = 20_000;

/** Letters and digits only, so the new field's term is `local:` plus the label in camel case. */
const runTag = (): string => `${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;

const originals = (): number => {
  try {
    return readdirSync(join(readHarness().storeRoot, 'original')).length;
  } catch {
    return 0;
  }
};

const getJson = async (request: APIRequestContext, path: string): Promise<unknown> => {
  const response = await request.get(path);
  expect(response.status(), path).toBe(200);
  const body: unknown = await response.json();
  return body;
};

/** The titles of the items an API search finds. */
const itemTitles = async (request: APIRequestContext, query: string): Promise<readonly string[]> => {
  const found = await getJson(request, `/api/items?${query}`);
  if (!isArray(found)) {
    throw new Error('/api/items: expected an array');
  }
  return found.flatMap((item) => (isRecord(item) && typeof item['o:title'] === 'string' ? [item['o:title']] : []));
};

/**
 * Record every text a status line shows from now on, and whether a progress
 * bar was on the page. A four-item job can end inside one poll, faster than an
 * assertion retries, so the Run step's progress view is proved from this
 * record rather than by waiting for it.
 */
const watchStatuses = async (page: Page): Promise<void> => {
  await page.evaluate(() => {
    const seen: string[] = [];
    Reflect.set(window, '__importStatuses', seen);
    new MutationObserver(() => {
      const texts = [...document.querySelectorAll('[role=status]')].map((element) => element.textContent);
      if (document.querySelector('progress') !== null) {
        texts.push('<progress>');
      }
      for (const text of texts) {
        if (!seen.includes(text)) {
          seen.push(text);
        }
      }
    }).observe(document.body, { subtree: true, childList: true, characterData: true });
  });
};

const seenStatuses = async (page: Page): Promise<readonly string[]> => {
  const seen: unknown = await page.evaluate((): unknown => Reflect.get(window, '__importStatuses'));
  return isArray(seen) ? seen.flatMap((text) => (typeof text === 'string' ? [text] : [])) : [];
};

const idsOf = (value: unknown): readonly number[] =>
  isArray(value) ? value.flatMap((entry) => (isRecord(entry) && typeof entry['o:id'] === 'number' ? [entry['o:id']] : [])) : [];

/**
 * Delete what a permanent import made, through the API as any client would:
 * the items in its sets (with their media files), then the sets. Untitled
 * public items left behind change what later specs find on the Items page.
 */
const deleteImported = async (request: APIRequestContext, id: number): Promise<void> => {
  const detail = await getJson(request, `/api/romeka/imports/${String(id)}`);
  const summary = isRecord(detail) ? detail.summary : null;
  const sets = isRecord(summary) && isArray(summary.itemSetIds) ? summary.itemSetIds : [];
  for (const set of sets) {
    if (typeof set !== 'number') {
      continue;
    }
    for (const item of idsOf(await getJson(request, `/api/items?item_set_id=${String(set)}&per_page=100`))) {
      expect((await request.delete(`/api/items/${String(item)}`)).ok(), `delete item ${String(item)}`).toBe(true);
    }
    expect((await request.delete(`/api/item_sets/${String(set)}`)).ok(), `delete item set ${String(set)}`).toBe(true);
  }
};

const step = (page: Page, number: number, name: string): Locator =>
  page.getByRole('navigation', { name: 'Import steps' }).getByRole('link', { name: `${String(number)}. ${name}` });

const importId = (page: Page): number => {
  const match = /#\/import\/(\d+)\//.exec(page.url());
  if (match?.[1] === undefined) {
    throw new Error(`not on an import: ${page.url()}`);
  }
  return Number(match[1]);
};

/** Import in the nav, then the fixture folder, then a new draft of it. */
const startDraft = async (page: Page): Promise<number> => {
  await page.goto('/#/items');
  await page.getByRole('navigation').getByRole('link', { name: 'Import', exact: true }).click();
  await expect(page).toHaveURL(/#\/import$/);
  await page.getByRole('button', { name: new RegExp(`^${ARCHIVE_FOLDER}/`) }).click();
  await page.getByRole('button', { name: `Import ${ARCHIVE_FOLDER}` }).click();
  await expect(page).toHaveURL(/#\/import\/\d+\/folder$/);
  return importId(page);
};

const checkRow = (page: Page, name: string): Locator =>
  page.getByRole('table', { name: 'What this import will do' }).getByRole('row', { name: new RegExp(`^${name}`) });

/** Check, then Import, then the Run step until it reports done. */
const runFromCheck = async (page: Page, items: number): Promise<void> => {
  await step(page, 6, 'Check').click();
  const start = page.getByRole('button', { name: `Import items: ${String(items)}` });
  await expect(start).toBeEnabled();
  await start.click();
  await expect(page).toHaveURL(/#\/import\/\d+\/run$/);
  await expect(page.getByRole('status').filter({ hasText: /^Done\./ })).toBeVisible({ timeout: JOB_TIMEOUT_MS });
};

test('an archive folder is imported through the seven steps and undone', async ({ page, diagnostics }) => {
  test.setTimeout(120_000);
  const tag = runTag();
  const setName = `Photographs ${tag}`;
  const typeName = `Photograph ${tag}`;
  const fieldName = `Notes ${tag}`;
  const editedTitle = `${CSV_TITLES.LT_001 ?? ''}, 1952`;
  const storedBefore = originals();

  await startDraft(page);

  // 1. Folder: the draft starts from the first file, its whole name the identifier.
  await expect(page.getByText(`${LETTERS}/LT_001_back.jpg`, { exact: true }).first()).toBeVisible();
  const parts = page.getByRole('group', { name: 'Parts of the example file name' });
  await parts.getByRole('button', { name: /^back/ }).click();
  await page.getByRole('group', { name: '“back” is' }).getByRole('button', { name: 'Part', exact: true }).click();
  await expect(page.getByText('Files that match: 3 of 7. Items they make: 2.')).toBeVisible();
  await expect(page.getByText('Some files have no part.')).toBeVisible();
  await page.getByRole('button', { name: 'Make it optional' }).click();
  await expect(page.getByText('The part is optional.')).toBeVisible();
  await expect(page.getByText('Files that match: 5 of 7. Items they make: 4.')).toBeVisible();
  await expect(page.getByText('One item has 2 files')).toHaveCount(0);
  await expectClean(diagnostics, 'folder step');

  // 2. Item sets: one set per top folder; the photographs' set is renamed.
  await step(page, 2, 'Item sets').click();
  await page.getByRole('textbox', { name: `Name for ${PHOTOS}` }).fill(setName);
  await expect(page.getByRole('textbox', { name: `Name for ${LETTERS}` })).toHaveValue(LETTERS);
  const photosRow = page.getByRole('row').filter({ has: page.getByRole('textbox', { name: `Name for ${PHOTOS}` }) });
  await expect(photosRow).toContainText('2 of 2');
  await expect(photosRow.getByText('New item set')).toBeVisible();

  // 3. Item types: a new Photograph type for the renamed set.
  await step(page, 3, 'Item types').click();
  await page.getByRole('combobox', { name: `Item type for ${setName}` }).selectOption({ label: 'New item type…' });
  await page.getByRole('group', { name: 'What kind of thing is this?' }).getByRole('button', { name: 'Photograph' }).click();
  await page.getByRole('textbox', { name: 'Name of the item type' }).fill(typeName);
  await page.getByRole('button', { name: 'Create item type' }).click();
  await expect(page.getByRole('combobox', { name: `Item type for ${setName}` })).toHaveValue(`n${typeName}`);
  await expectClean(diagnostics, 'sets and types');

  // 4. Sources: the spreadsheet, read the way the server detects it, joined by identifier.
  await step(page, 4, 'Sources').click();
  await page.getByRole('radio', { name: CSV_FILE }).check();
  await expect(page.getByRole('radio', { name: 'Semicolon (detected)' })).toBeChecked();
  await expect(page.getByRole('radio', { name: 'Line 2 (detected)' })).toBeChecked();
  await page.getByRole('combobox', { name: 'Match rows to items by the column' }).selectOption('identifier');
  await expect(
    page.getByText('Rows that match an item: 4. Rows that match no file: 0. Items with no row: 0.'),
  ).toBeVisible();
  await expectClean(diagnostics, 'sources step');

  // 5. Grid: map the two spreadsheet columns, use two bulk fixes, edit one cell.
  await step(page, 5, 'Grid').click();
  const grid = page.locator('.import-grid table');
  await grid.getByRole('columnheader', { name: /^title/ }).getByRole('button', { name: 'Map…' }).click();
  await page.locator('.import-panel').getByRole('button', { name: 'Title', exact: true }).click();
  await grid.getByRole('columnheader', { name: /^notes/ }).getByRole('button', { name: 'Map…' }).click();
  await page.getByRole('textbox', { name: 'Or make a new field for this archive only:' }).fill(fieldName);
  await page.getByRole('button', { name: 'Make field' }).click();
  await expect(grid.getByRole('columnheader', { name: new RegExp(`^${fieldName}`) })).toBeVisible();

  const kinds = page.locator('.import-kinds');
  const encoding = kinds.getByRole('listitem').filter({ hasText: 'looks like text read with the wrong encoding' });
  await expect(encoding).toContainText('Rows: 1');
  await encoding.getByRole('button', { name: 'Read as Windows-1252' }).click();
  await expect(grid.getByRole('button', { name: 'Café visit' })).toBeVisible();

  const empty = kinds.getByRole('listitem').filter({ hasText: 'is empty, but it is required' });
  await expect(empty).toContainText('Rows: 1');
  await empty.getByRole('button', { name: 'Fill them' }).click();
  await page.getByRole('textbox', { name: /^Value\./ }).fill('Untitled {identifier}');
  await page.getByRole('button', { name: 'Fill the empty cells: 1' }).click();
  await expect(grid.getByRole('button', { name: 'Untitled PH_102' })).toBeVisible();

  await grid.getByRole('button', { name: CSV_TITLES.LT_001 ?? '' }).click();
  await page.getByRole('textbox', { name: /^LT_001 · / }).fill(editedTitle);
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(grid.getByRole('button', { name: editedTitle })).toBeVisible();
  await expect(page.getByText('Rows: 4. Rows with problems: 0.')).toBeVisible();
  await expectClean(diagnostics, 'grid step');

  // 6. Check: the dry run counts what the run will write.
  await step(page, 6, 'Check').click();
  await expect(checkRow(page, 'Items to create')).toContainText('4');
  await expect(checkRow(page, 'Files to store')).toContainText('5');
  await expect(checkRow(page, 'New item sets')).toContainText(`2${LETTERS}, ${setName}`);
  await expect(checkRow(page, 'New item types')).toContainText(`1${typeName}`);
  await expect(checkRow(page, 'New fields')).toContainText(`1${fieldName}`);
  await expect(checkRow(page, 'Files that match no item')).toContainText('1');
  await expect(page.getByText(`${LETTERS}/readme.txt`)).toBeVisible();

  // 7. Run: progress while the job works, then done.
  await watchStatuses(page);
  await page.getByRole('button', { name: 'Import items: 4' }).click();
  await expect(page).toHaveURL(/#\/import\/\d+\/run$/);
  const done = page.getByRole('status').filter({ hasText: /^Done\./ });
  await expect(done).toBeVisible({ timeout: JOB_TIMEOUT_MS });
  const statuses = await seenStatuses(page);
  const importing = statuses.findIndex((text) => text.startsWith('Importing. Items done:'));
  expect(importing, statuses.join(' | ')).toBeGreaterThanOrEqual(0);
  expect(statuses, statuses.join(' | ')).toContain('<progress>');
  expect(statuses.findIndex((text) => text.startsWith('Done.')), statuses.join(' | ')).toBeGreaterThan(importing);
  await expect(done).toContainText('Items: 4. Files: 5.');
  await expect(done).toContainText('Rows left out: 0.');
  await expect(page.getByText('You can undo this import until the next import starts.')).toBeVisible();
  expect(originals()).toBe(storedBefore + 5);
  await expectClean(diagnostics, 'run');

  // The Run step links the job, and the Jobs page shows the job's log.
  await page.getByRole('link', { name: /^Job #\d+ and its log$/ }).click();
  await expect(page).toHaveURL(/#\/jobs\/\d+$/);
  await expect(page.locator('pre.job-log')).toContainText('imported 4 items and 5 files; 0 rows left out, 0 failed');
  await page.goBack();
  await expect(done).toBeVisible();

  // The items are on the Items page, and the TIFF master's thumbnail is served as a JPEG.
  await page.getByRole('navigation').getByRole('link', { name: 'Items', exact: true }).click();
  for (const title of [editedTitle, CSV_TITLES.LT_002 ?? '', CSV_TITLES.PH_101 ?? '', 'Untitled PH_102']) {
    await expect(page.getByRole('link', { name: title, exact: true })).toBeVisible();
  }
  await page.getByRole('link', { name: CSV_TITLES.PH_101 ?? '', exact: true }).click();
  const thumbnail = page.locator('img.media-card-image').first();
  await expect(thumbnail).toBeVisible();
  const src = (await thumbnail.getAttribute('src')) ?? '';
  expect(src.startsWith(`${readHarness().baseUrl}/files/`), src).toBe(true);
  const served = await page.request.get(src);
  expect(served.status()).toBe(200);
  expect(served.headers()['content-type']).toBe('image/jpeg');
  await expect.poll(() => thumbnail.evaluate((image: HTMLImageElement) => image.naturalWidth)).toBeGreaterThan(0);
  await page.getByRole('navigation').getByRole('link', { name: 'Items', exact: true }).click();
  await page.getByRole('link', { name: editedTitle, exact: true }).click();
  await expect(page.locator('img.media-card-image')).toHaveCount(2);
  await expectClean(diagnostics, 'imported items');

  // Back on Import, the banner offers the undo; it asks first, then the job deletes.
  await page.getByRole('navigation').getByRole('link', { name: 'Import', exact: true }).click();
  await expect(page.getByText(`Your last import (${ARCHIVE_FOLDER}, items: 4) can still be undone.`)).toBeVisible();
  await page.getByRole('button', { name: 'Undo it' }).click();
  const undo = page.getByRole('group', { name: `Undo the import of ${ARCHIVE_FOLDER}` });
  await expect(undo).toContainText(`The new item sets, if nothing else was added to them: ${LETTERS}, ${setName}.`);
  await expect(undo).toContainText(`The new item types, if no other item uses them: ${typeName}.`);
  await undo.getByRole('button', { name: 'Delete the imported items (4)' }).click();
  await expect(page).toHaveURL(/#\/import\/\d+\/run$/);
  const undone = page.getByRole('status').filter({ hasText: /^Undone\./ });
  await expect(undone).toBeVisible({ timeout: JOB_TIMEOUT_MS });
  await expect(undone).toContainText('Deleted items: 4. Files: 5. Item sets: 2. Item types: 1. Fields: 1.');
  await expectClean(diagnostics, 'undo');

  expect(originals()).toBe(storedBefore);
  for (const title of [editedTitle, CSV_TITLES.PH_101 ?? '', 'Untitled PH_102']) {
    expect(await itemTitles(page.request, `fulltext_search=${encodeURIComponent(title)}`)).not.toContain(title);
  }
  await page.getByRole('navigation').getByRole('link', { name: 'Items', exact: true }).click();
  await expect(page.getByRole('link', { name: CSV_TITLES.PH_101 ?? '', exact: true })).toHaveCount(0);
  await page.getByRole('navigation').getByRole('link', { name: 'Import', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Undo it' })).toHaveCount(0);
  await expectClean(diagnostics, 'after undo');
});

test('a later import makes the earlier one permanent', async ({ page, diagnostics }) => {
  test.setTimeout(120_000);
  const tag = runTag();

  /** A draft with the starting marks: each letter file one item, its set renamed for this test. */
  const importLetters = async (setName: string): Promise<number> => {
    const id = await startDraft(page);
    await expect(page.getByText('Files that match: 3 of 7. Items they make: 3.')).toBeVisible();
    await step(page, 2, 'Item sets').click();
    await page.getByRole('textbox', { name: `Name for ${LETTERS}` }).fill(setName);
    await page.getByRole('checkbox', { name: `Use ${PHOTOS}` }).uncheck();
    await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible();
    return id;
  };

  const first = await importLetters(`Letters first ${tag}`);
  await runFromCheck(page, 3);
  await expect(page.getByRole('button', { name: 'Undo this import' })).toBeVisible();

  // The second import groups front and back into one item, so its identifiers (LT_001, LT_002,
  // PH_101, PH_102) differ from the first import's whole file names and it makes four items.
  await startDraft(page);
  const parts = page.getByRole('group', { name: 'Parts of the example file name' });
  await parts.getByRole('button', { name: /^back/ }).click();
  await page.getByRole('group', { name: '“back” is' }).getByRole('button', { name: 'Part', exact: true }).click();
  await page.getByRole('button', { name: 'Make it optional' }).click();
  await expect(page.getByText('Files that match: 5 of 7. Items they make: 4.')).toBeVisible();
  await step(page, 2, 'Item sets').click();
  await page.getByRole('textbox', { name: `Name for ${LETTERS}` }).fill(`Letters second ${tag}`);
  await page.getByRole('textbox', { name: `Name for ${PHOTOS}` }).fill(`Photos second ${tag}`);
  await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible();
  await step(page, 6, 'Check').click();
  await expect(checkRow(page, 'Items that already exist and are skipped')).toContainText('0');
  await expect(
    page.getByText(
      `Starting this import makes the last import (${ARCHIVE_FOLDER}, items: 3) permanent. Until then you can undo it.`,
    ),
  ).toBeVisible();
  await runFromCheck(page, 4);
  await expectClean(diagnostics, 'two imports');

  await page.goto(`/#/import/${String(first)}/run`);
  await expect(page.getByRole('status').filter({ hasText: /^Done\./ })).toBeVisible();
  await expect(page.getByText('A later import started, so this import is permanent.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Undo this import' })).toHaveCount(0);
  const refused = await page.request.post(`/api/romeka/imports/${String(first)}/undo`);
  expect(refused.status()).toBe(409);
  await expectClean(diagnostics, 'first import is permanent');

  // The second import still holds the window; undo it so the run leaves only the permanent one.
  await page.getByRole('navigation').getByRole('link', { name: 'Import', exact: true }).click();
  await page.getByRole('button', { name: 'Undo it' }).click();
  await page.getByRole('button', { name: 'Delete the imported items (4)' }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Undone\./ })).toBeVisible({ timeout: JOB_TIMEOUT_MS });
  await expectClean(diagnostics, 'second import undone');

  const storedBefore = originals();
  await deleteImported(page.request, first);
  expect(originals()).toBe(storedBefore - 3);
});
