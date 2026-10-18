/**
 * The data every spec starts from.
 *
 * `globalSetup` writes it through the harness origin, so the seeding run also
 * proves the origin forwards `/api` with the minted key. The specs read the same
 * constants to find their rows, and the item set is created here because the CLI
 * cannot create one.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';

import { runCli } from './cli';
import { isArray, numberField, parseJson, record, stringField } from './json';

export const ITEM_SET_TITLE = 'E2E item set';
export const ITEM_TITLES: readonly string[] = ['E2E item 1', 'E2E item 2', 'E2E item 3'];
export const SITE_TITLE = 'E2E site';
export const SITE_SLUG = 'e2e-site';
export const MEDIA_FILE = 'sample.png';
/** `SiteAdapter::hydrate` builds this page on create; the public spec visits it. */
export const WELCOME_SLUG = 'welcome';
/** The property every seeded title uses. */
export const TITLE_TERM = 'dcterms:title';

/** An asset the write spec edits and deletes. No endpoint creates one. */
export const ASSET_NAME = 'E2E asset';
const ASSET_STORAGE_ID = 'e2easset';
const ASSET_EXTENSION = 'png';
/** A second asset, so the accessibility pass always has a row to open. */
export const ASSET_SPARE_NAME = 'E2E spare asset';
const ASSET_SPARE_STORAGE_ID = 'e2eassetspare';
/** A job the write spec updates and deletes. The dispatcher writes a real one. */
export const JOB_CLASS = 'Omeka\\Job\\IndexFulltextSearch';
export const JOB_ARGS = { action: 'index' };
/** A second job, for the same reason as the spare asset. */
export const JOB_SPARE_CLASS = 'Omeka\\Job\\BatchUpdate';

/**
 * An 8 by 8 opaque PNG. The store then holds bytes a browser decodes, which is
 * what the media spec checks.
 */
const PNG_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGM4ISf3Hx9mGBkKAFNGgMHUn6b2AAAAAElFTkSuQmCC';

/** The media directory `omeka import media` reads. */
export const mediaDir = (workDir: string): string => join(workDir, 'media');
/** The file store root, which the harness origin serves under `/files`. */
export const storeRoot = (workDir: string): string => join(workDir, 'files');

const apiGet = async (baseUrl: string, path: string): Promise<unknown> => {
  const response = await fetch(`${baseUrl}${path}`, { headers: { accept: 'application/json' } });
  if (!response.ok) {
    throw new Error(`GET ${path}: ${String(response.status)} ${await response.text()}`);
  }
  return parseJson(await response.text());
};

const apiPost = async (
  baseUrl: string,
  path: string,
  body: Record<string, unknown>,
): Promise<Record<string, unknown>> => {
  const response = await fetch(`${baseUrl}${path}`, {
    method: 'POST',
    headers: { accept: 'application/json', 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`POST ${path}: ${String(response.status)} ${await response.text()}`);
  }
  return record(parseJson(await response.text()), path);
};

/** The id of `term` in the installed vocabularies. */
const propertyId = async (baseUrl: string, term: string): Promise<number> => {
  const doc = await apiGet(baseUrl, '/api/properties?per_page=300');
  if (!isArray(doc)) {
    throw new Error('/api/properties: expected an array');
  }
  for (const entry of doc) {
    const row = record(entry, 'property');
    if (stringField(row, 'o:term', 'property') === term) {
      return numberField(row, 'o:id', 'property');
    }
  }
  throw new Error(`no property with term ${term}`);
};

const literal = (property: number, value: string): Record<string, unknown> => ({
  type: 'literal',
  property_id: property,
  is_public: true,
  '@value': value,
});

export interface Seeded {
  readonly itemSetId: number;
  readonly itemIds: readonly number[];
  readonly mediaId: number;
  readonly siteId: number;
}

/** The `asset` and `job` rows `seedSystem` writes before the port starts. */
export interface SeededSystem {
  readonly assetId: number;
  readonly jobId: number;
  /** The administrator `omeka install` created, the owner of both rows. */
  readonly adminId: number;
}

/** An upstream `DATETIME`: the port's `started` column reads with
 * `NaiveDateTime::parse`, which takes `YYYY-MM-DD HH:MM:SS`, not an ISO string.
 */
const datetime = (at: Date): string => at.toISOString().slice(0, 19).replace('T', ' ');

/** A numeric column value, or `null` when the row has none. */
const integerColumn = (row: Record<string, unknown>, key: string): number | null => {
  const value = row[key];
  return typeof value === 'number' ? value : null;
};

/**
 * Write two assets and two jobs straight into the database, with each asset's
 * bytes under `asset/<storage id>.<extension>`.
 *
 * No endpoint creates either row: `POST /api/assets` needs a multipart file and
 * `POST /api/jobs` needs a `class` the dispatcher sets. The spec that edits and
 * deletes them needs rows to start from, and the accessibility pass needs a row
 * to open after that spec has deleted one, so each table gets a spare. This
 * runs before the port starts, so the inserts do not race the server.
 *
 * The jobs get no status, which `job.status` allows, so the run covers a job row
 * whose `o:status` is null.
 */
export const seedSystem = (
  workDir: string,
  dbPath: string,
  ownerEmail: string,
): SeededSystem => {
  const store = storeRoot(workDir);
  mkdirSync(join(store, 'asset'), { recursive: true });
  for (const storageId of [ASSET_STORAGE_ID, ASSET_SPARE_STORAGE_ID]) {
    writeFileSync(
      join(store, 'asset', `${storageId}.${ASSET_EXTENSION}`),
      Buffer.from(PNG_BASE64, 'base64'),
    );
  }

  const database = new DatabaseSync(dbPath);
  try {
    database.exec('PRAGMA busy_timeout = 5000');
    const ownerRow = database.prepare('SELECT id FROM user WHERE email = ?').get(ownerEmail);
    const ownerId = ownerRow === undefined ? null : integerColumn(ownerRow, 'id');
    if (ownerId === null) {
      throw new Error(`no user row for ${ownerEmail}, so the seeded rows would have no owner`);
    }
    const insertAsset = database.prepare(
      'INSERT INTO asset (owner_id, name, media_type, storage_id, extension, alt_text) ' +
        'VALUES (?, ?, ?, ?, ?, ?)',
    );
    const insertJob = database.prepare(
      'INSERT INTO job (owner_id, pid, status, class, args, log, started, ended) ' +
        'VALUES (?, NULL, NULL, ?, ?, NULL, ?, NULL)',
    );
    const started = datetime(new Date());

    const asset = insertAsset.run(
      ownerId,
      ASSET_NAME,
      'image/png',
      ASSET_STORAGE_ID,
      ASSET_EXTENSION,
      null,
    );
    insertAsset.run(
      ownerId,
      ASSET_SPARE_NAME,
      'image/png',
      ASSET_SPARE_STORAGE_ID,
      ASSET_EXTENSION,
      null,
    );
    const job = insertJob.run(ownerId, JOB_CLASS, JSON.stringify(JOB_ARGS), started);
    insertJob.run(ownerId, JOB_SPARE_CLASS, JSON.stringify(JOB_ARGS), started);

    return {
      assetId: Number(asset.lastInsertRowid),
      jobId: Number(job.lastInsertRowid),
      adminId: ownerId,
    };
  } finally {
    database.close();
  }
};

/**
 * Create the item set, the items, one imported media file and one site, then
 * report the ids the run produced.
 */
export const seedData = async (
  baseUrl: string,
  cli: string,
  workDir: string,
  dbPath: string,
): Promise<Seeded> => {
  const title = await propertyId(baseUrl, TITLE_TERM);

  const itemSet = await apiPost(baseUrl, '/api/item_sets', {
    'o:is_public': true,
    [TITLE_TERM]: [literal(title, ITEM_SET_TITLE)],
  });
  const itemSetId = numberField(itemSet, 'o:id', 'item_set');

  const itemIds: number[] = [];
  for (const itemTitle of ITEM_TITLES) {
    const item = await apiPost(baseUrl, '/api/items', {
      'o:is_public': true,
      [TITLE_TERM]: [literal(title, itemTitle)],
      'o:item_set': [{ 'o:id': itemSetId }],
    });
    itemIds.push(numberField(item, 'o:id', 'item'));
  }

  const firstItem = itemIds[0];
  if (firstItem === undefined) {
    throw new Error('no item was seeded, so no media could be imported');
  }
  writeFileSync(join(mediaDir(workDir), MEDIA_FILE), Buffer.from(PNG_BASE64, 'base64'));
  runCli(
    cli,
    [
      'import',
      'media',
      '-d',
      dbPath,
      '--source',
      mediaDir(workDir),
      '--store',
      storeRoot(workDir),
      '--item',
      String(firstItem),
    ],
    workDir,
  );
  const mediaDoc = await apiGet(baseUrl, `/api/media?item_id=${String(firstItem)}`);
  if (!isArray(mediaDoc)) {
    throw new Error('/api/media: expected an array');
  }
  const firstMedia = mediaDoc[0];
  if (firstMedia === undefined) {
    throw new Error(`importing ${MEDIA_FILE} wrote no media row`);
  }

  const site = await apiPost(baseUrl, '/api/sites', {
    'o:title': SITE_TITLE,
    'o:theme': 'default',
    'o:slug': SITE_SLUG,
    'o:is_public': true,
  });
  return {
    itemSetId,
    itemIds,
    mediaId: numberField(record(firstMedia, 'media'), 'o:id', 'media'),
    siteId: numberField(site, 'o:id', 'site'),
  };
};

/** Create the media directory before `seedData` writes into it. */
export const prepareWorkDir = (workDir: string): void => {
  mkdirSync(mediaDir(workDir), { recursive: true });
};
