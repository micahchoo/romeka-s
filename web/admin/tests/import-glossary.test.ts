/**
 * Plain words on the import screens: no English string of the `import`
 * namespace uses a contract word a person in Simple mode must never see.
 *
 * The lists are the approved prototype's (round 4, `GLOSSARY.md`, checked by
 * its `glossary-check.js`): the left column of its table is Simple's banned
 * list, and "Advanced may show" lists the precise words Advanced may use. A
 * key under `advanced.` is shown in Advanced only, so it is held to the banned
 * list less those words. The `import-simple` namespace shows in Simple only, so
 * all of it is held to the banned list. To ban a word, add it here.
 */
import { describe, expect, it } from 'vitest';
import { en } from '../src/i18n/namespaces/import';
import { en as simple } from '../src/i18n/namespaces/import-simple';

const BANNED = [
  'slot', 'source ref', 'source', 'staging', 'evidence', 'evidence index', 'rescan', 'walk', 'entry',
  'sheet', 'row key', 'group key', 'file group', 'unmatched', 'record', 'item record', 'media record',
  'rule', 'pattern', 'regex', 'derive', 'preview', 'decision', 'move decision', 'set decision',
  'orphan', 'flag', 'flag kind', 'policy', 'blocking', 'anomaly', 'digest', 'fingerprint', 'ledger',
  'ledger entry', 'draft', 'plan', 'ran_plan', 'frozen', 'action', 'create', 'append', 'revise',
  'update', 'replace', 'match key', 'match', 'scope', 'exists', 'join', 'normalise', 'normalize',
  'tolerance', 'ingester', 'mediaSource', 'template', 'Base Resource', 'newTypes', 'newTerms',
  'property', 'term', 'dcterms', 'vocabulary', 'visibility', 'owner', 'touched', 'before-image',
  'force', 'encoding', 'delimiter', 'header line', 'cells-count', 'EDTF', 'tus', 'run_failed',
  'undo_failed', 'combine', 'cascade', 'constant', 'install', 'hash', 'shape',
];

const ADVANCED_ALLOWED = new Set([
  'pattern', 'regex', 'source', 'join', 'match', 'match key', 'scope', 'unmatched', 'exists',
  'normalize', 'combine', 'constant', 'rule', 'encoding', 'delimiter', 'header line', 'visibility',
  'owner', 'action', 'create', 'append', 'revise', 'update', 'replace', 'flag', 'flag kind', 'policy',
  'anomaly', 'record', 'before-image', 'cells-count', 'EDTF', 'dcterms', 'hash',
]);

const escape = (word: string): string =>
  word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/[ _-]/g, '[ _-]');
/** The word alone or with an English ending, not inside a longer word: `glossary-check.js`'s rule. */
const wordRe = (word: string): RegExp =>
  new RegExp(`(^|[^\\p{L}\\p{N}_])${escape(word)}(s|es|ed|d|ing)?(?![\\p{L}\\p{N}_])`, 'iu');
const RULES = BANNED.map((word) => ({ word, re: wordRe(word) }));

/** The banned words in one string; its `{name}` placeholders are filled in by code, not read. */
const bannedIn = (key: string, text: string): readonly string[] => {
  const shown = text.replace(/\{[A-Za-z][A-Za-z0-9]*\}/g, ' ');
  const advanced = key.startsWith('advanced.');
  return RULES.filter(({ word, re }) => !(advanced && ADVANCED_ALLOWED.has(word)) && re.test(shown)).map(
    ({ word }) => `${key}: "${word}" in "${text}"`,
  );
};

describe('import strings use plain words', () => {
  it('finds a banned word in a string, with an ending, and not inside a longer word', () => {
    expect(bannedIn('home.x', 'Your draft is saved')).toHaveLength(1);
    expect(bannedIn('home.x', 'Two files matched')).toHaveLength(1);
    expect(bannedIn('home.x', 'The run failed')).toHaveLength(1);
    expect(bannedIn('home.x', 'The status of {source}')).toEqual([]);
    expect(bannedIn('advanced.x', 'The pattern and the action')).toEqual([]);
    expect(bannedIn('advanced.x', 'A slot rule')).toHaveLength(1);
  });

  it('every English string passes its mode’s word list', () => {
    const found = Object.entries(en).flatMap(([key, text]) => bannedIn(key, text));
    expect(found).toEqual([]);
    expect(Object.keys(en).length).toBeGreaterThan(0);
  });

  it('every Simple screen string passes Simple’s word list', () => {
    const found = Object.entries(simple).flatMap(([key, text]) => bannedIn(key, text));
    expect(found).toEqual([]);
    expect(Object.keys(simple).every((key) => !key.startsWith('advanced.'))).toBe(true);
  });
});
