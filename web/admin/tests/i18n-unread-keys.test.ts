/**
 * Every catalogue key is read by some code: `scripts/unread-keys.mjs` lists
 * the keys of each namespace that no code outside the catalogues names, and
 * there are none. A unit that deletes a screen deletes its words too, with
 * `node scripts/unread-keys.mjs --remove`.
 */
import { execFileSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const SCRIPT = resolve(dirname(fileURLToPath(import.meta.url)), '../scripts/unread-keys.mjs');

describe('the catalogues', () => {
  it('hold no key that no code reads', () => {
    const report = execFileSync(process.execPath, [SCRIPT], { encoding: 'utf8' });
    expect(report.trim().split('\n')).toEqual(['0 unread keys (src/i18n/namespaces)']);
  });
});
