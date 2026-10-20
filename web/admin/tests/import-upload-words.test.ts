/**
 * The upload strings (`upload.*` in the Import namespace, English and Spanish)
 * hold none of the contract words a person must not see.
 *
 * BANNED is the left column of the Simple table in the import-v2 glossary
 * (`.orchestrate/import-v2/proto4/GLOSSARY.md`, beside `glossary-check.js`),
 * copied here because that folder is outside this repository. The match is
 * the checker's: whole words, case folded, `-`, `_` and space alike, with the
 * endings s, es, ed, d and ing.
 */
import { describe, expect, it } from 'vitest';
import { en } from '../src/i18n/namespaces/import';
import { es } from '../src/i18n/namespaces/import/locales/es';

const BANNED = [
  'slot', 'source ref', 'source', 'staging', 'evidence', 'evidence index', 'rescan', 'walk', 'entry', 'sheet',
  'row key', 'group key', 'file group', 'unmatched', 'record', 'item record', 'media record', 'rule', 'pattern',
  'regex', 'derive', 'preview', 'decision', 'move decision', 'set decision', 'orphan', 'flag', 'flag kind',
  'policy', 'blocking', 'anomaly', 'digest', 'fingerprint', 'ledger', 'ledger entry', 'draft', 'plan', 'ran_plan',
  'frozen', 'action', 'create', 'append', 'revise', 'update', 'replace', 'match key', 'match', 'scope', 'exists',
  'join', 'normalise', 'normalize', 'tolerance', 'ingester', 'mediaSource', 'template', 'Base Resource',
  'newTypes', 'newTerms', 'property', 'term', 'dcterms', 'vocabulary', 'visibility', 'owner', 'touched',
  'before-image', 'force', 'encoding', 'delimiter', 'header line', 'cells-count', 'EDTF', 'tus', 'run_failed',
  'undo_failed', 'combine', 'cascade', 'constant', 'install', 'hash', 'shape',
] as const;

const escape = (word: string): string => word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/[ _-]/g, '[ _-]');
const PATTERNS = BANNED.map((word) => ({ word, re: new RegExp(`(^|[^\\p{L}\\p{N}_])${escape(word)}(s|es|ed|d|ing)?(?![\\p{L}\\p{N}_])`, 'iu') }));

const bannedIn = (text: string): readonly string[] => PATTERNS.filter(({ re }) => re.test(text)).map(({ word }) => word);

const uploadStrings = (table: Readonly<Record<string, string>>): [string, string][] =>
  Object.entries(table).filter(([key]) => key.startsWith('upload.'));

describe('the upload strings', () => {
  it('finds a banned word the way the glossary checker does', () => {
    expect(bannedIn('Files in the staging area')).toEqual(['staging']);
    expect(bannedIn('It matched two rows')).toEqual(['match']);
    expect(bannedIn('Sent over TUS')).toEqual(['tus']);
    expect(bannedIn('Status of the upload')).toEqual([]);
  });

  it('holds no banned word in English or Spanish', () => {
    const english = uploadStrings(en);
    const spanish = uploadStrings(es);
    expect(english.length).toBeGreaterThan(40);
    expect(spanish.map(([key]) => key)).toEqual(english.map(([key]) => key));
    const found = [...english, ...spanish].flatMap(([key, text]) => bannedIn(text).map((word) => `${key}: ${word}`));
    expect(found).toEqual([]);
  });
});
