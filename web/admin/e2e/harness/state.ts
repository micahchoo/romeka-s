/**
 * The handoff between the Playwright main process and the test workers.
 *
 * `global-setup.ts` starts the port and the harness origin, seeds data and writes
 * one JSON record here. The `baseURL` fixture reads it back, so a worker learns
 * the ephemeral port the run bound. Every path below is derived from this file's
 * own location, so the suite runs from any working directory.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { parseJson, numberField, record, stringField } from './json';

/** `web/admin`, the package the suite belongs to. */
export const APP_ROOT: string = resolve(import.meta.dirname, '..', '..');
/** The repository root, three levels above `web/admin`. */
export const REPO_ROOT: string = resolve(APP_ROOT, '..', '..');
/** The built SPA that the harness origin serves. */
export const DIST_ROOT: string = join(APP_ROOT, 'dist');

/** One harness run. `HARNESS` is the value `globalSetup` writes and the specs read. */
export interface Harness {
  /** The origin the browser loads: the built SPA, the file store and the API. */
  readonly baseUrl: string;
  /** The origin `omeka serve` bound. */
  readonly serverUrl: string;
  /** The SQLite database `omeka install` wrote. */
  readonly dbPath: string;
  /** The file store root, served under `/files`. */
  readonly storeRoot: string;
  /** The temporary directory holding the database, the store and the media file. */
  readonly workDir: string;
  /** The minted key's identity: 32 alphanumeric characters. */
  readonly keyIdentity: string;
  /** The minted key's credential: 32 alphanumeric characters. */
  readonly keyCredential: string;
  /** The seeded asset the write spec edits and deletes. */
  readonly assetId: number;
  /** The seeded job the write spec updates and deletes. */
  readonly jobId: number;
  /** The administrator `omeka install` created, for the password sign-in specs. */
  readonly adminName: string;
  readonly adminEmail: string;
  readonly adminPassword: string;
}

const stateDir = (): string => {
  const override = process.env.OMEKA_E2E_STATE_DIR;
  if (override !== undefined && override !== '') {
    return override;
  }
  const key = createHash('sha256').update(REPO_ROOT).digest('hex').slice(0, 12);
  return join(tmpdir(), `omeka-e2e-${key}`);
};

export const stateFile = (): string => join(stateDir(), 'harness.json');

export const writeHarness = (harness: Harness): void => {
  mkdirSync(stateDir(), { recursive: true });
  writeFileSync(stateFile(), JSON.stringify(harness, null, 2));
};

export const readHarness = (): Harness => {
  const doc = record(parseJson(readFileSync(stateFile(), 'utf8')), 'harness');
  return {
    baseUrl: stringField(doc, 'baseUrl', 'harness'),
    serverUrl: stringField(doc, 'serverUrl', 'harness'),
    dbPath: stringField(doc, 'dbPath', 'harness'),
    storeRoot: stringField(doc, 'storeRoot', 'harness'),
    workDir: stringField(doc, 'workDir', 'harness'),
    keyIdentity: stringField(doc, 'keyIdentity', 'harness'),
    keyCredential: stringField(doc, 'keyCredential', 'harness'),
    assetId: numberField(doc, 'assetId', 'harness'),
    jobId: numberField(doc, 'jobId', 'harness'),
    adminName: stringField(doc, 'adminName', 'harness'),
    adminEmail: stringField(doc, 'adminEmail', 'harness'),
    adminPassword: stringField(doc, 'adminPassword', 'harness'),
  };
};
