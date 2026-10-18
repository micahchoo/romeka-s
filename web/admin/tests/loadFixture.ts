/**
 * Reads a fixture from `tests/fixtures` as untouched JSON. The parsers are the
 * only thing allowed to give it a shape, so the return type is `unknown`.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const FIXTURES_DIR = resolve(dirname(fileURLToPath(import.meta.url)), 'fixtures');

export const loadFixture = (name: string): unknown => {
  const text = readFileSync(resolve(FIXTURES_DIR, name), 'utf8');
  const value: unknown = JSON.parse(text);
  return value;
};
