/**
 * One field, one name: every admin screen outside Standards names a field with
 * `plainName`, and ESLint refuses a page that reads the vocabulary label itself.
 */
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { plainName } from '../src/app/plain-terms';
import type { TermLabels } from '../src/i18n';

const english: TermLabels = { locale: 'en', label: (_term, label) => label };

describe('a field name', () => {
  it('is the item type’s own name, then the plain name, then the label, then the term', () => {
    const spatial = { term: 'dcterms:spatial', label: 'Spatial Coverage' };
    expect(plainName(spatial, english, 'Village')).toBe('Village');
    expect(plainName(spatial, english, '  ')).toBe('Place');
    expect(plainName(spatial, english)).toBe('Place');
    expect(plainName({ term: 'dcterms:dateAccepted', label: 'Date Accepted' }, english)).toBe('Date Accepted');
    expect(plainName({ term: 'local:curator', label: null }, english)).toBe('local:curator');
  });

  it('is the curated plain name in Spanish too, and English where a locale has none curated', () => {
    const spatial = { term: 'dcterms:spatial', label: 'Spatial Coverage' };
    expect(plainName(spatial, { ...english, locale: 'es' })).toBe('Lugar');
    expect(plainName(spatial, { ...english, locale: 'hi' })).toBe('Place');
  });

  it('is refused when a page reads the vocabulary label of a field itself', () => {
    const eslint = resolve(process.cwd(), 'node_modules', 'eslint', 'bin', 'eslint.js');
    const result = spawnSync(
      process.execPath,
      [eslint, '--no-ignore', '--no-warn-ignored', 'tests/lint/field-name-raw-label.ts'],
      { cwd: process.cwd(), encoding: 'utf8' },
    );
    expect(result.status).toBe(1);
    expect(result.stdout.match(/Name a field with plainName/g)).toHaveLength(2);
  }, 60_000);
});
