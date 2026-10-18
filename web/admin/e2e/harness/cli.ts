/**
 * The `omeka` binary the harness drives.
 *
 * `OMEKA_BIN` is used as given. Otherwise the workspace is asked to build
 * `omeka-cli` on every run, because cargo is incremental, so a warm target pays a
 * fraction of a second while a stale `debug/omeka` can never serve the specs.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

import { REPO_ROOT } from './state';

export const cliPath = (): string => {
  const override = process.env.OMEKA_BIN;
  if (override !== undefined && override !== '') {
    return override;
  }
  const targetDir = process.env.CARGO_TARGET_DIR ?? join(REPO_ROOT, 'target');
  return join(targetDir, 'debug', 'omeka');
};

/** Build `omeka-cli` and return the binary. */
export const ensureCli = (): string => {
  const override = process.env.OMEKA_BIN;
  if (override !== undefined && override !== '') {
    return override;
  }
  execFileSync('cargo', ['build', '-p', 'omeka-cli'], { cwd: REPO_ROOT, stdio: 'inherit' });
  const binary = cliPath();
  if (!existsSync(binary)) {
    throw new Error(`omeka-cli built no binary at ${binary}`);
  }
  return binary;
};

/** Run one `omeka` command to completion and return its stdout. */
export const runCli = (binary: string, args: readonly string[], cwd: string): string =>
  execFileSync(binary, [...args], { cwd, encoding: 'utf8' });

/** The `--key_identity=...` / `--key_credential=...` pair `omeka key add` prints once. */
export interface MintedKey {
  readonly identity: string;
  readonly credential: string;
}

const lineValue = (output: string, name: string): string => {
  const line = output
    .split('\n')
    .map((entry) => entry.trim())
    .find((entry) => entry.startsWith(`${name}=`));
  if (line === undefined) {
    throw new Error(`omeka key add printed no ${name}: ${output}`);
  }
  return line.slice(name.length + 1);
};

/** Mint an API key for `email` and read the pair back off the command's output. */
export const mintKey = (binary: string, database: string, email: string, cwd: string): MintedKey => {
  const output = runCli(
    binary,
    ['key', 'add', '-d', database, '--owner', email, '--label', 'e2e'],
    cwd,
  );
  return {
    identity: lineValue(output, 'key_identity'),
    credential: lineValue(output, 'key_credential'),
  };
};
