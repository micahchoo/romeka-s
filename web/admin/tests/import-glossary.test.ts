/**
 * Plain words on the import screens: no English string of the `import`,
 * `import-lists`, `import-run`, `import-simple`, `import-structure` and
 * `import-advanced` namespaces uses a contract word its mode must not show.
 *
 * The lists are the approved prototype's (round 4, `GLOSSARY.md`, checked by
 * its `glossary-check.js`): the left column of its table is Simple's banned
 * list, and "Advanced may show" lists the precise words Advanced may use. A
 * key under `advanced.`, and every key of `import-advanced`, is shown in
 * Advanced only, so it is held to the banned list less those words. The
 * `import-simple` and `import-structure` namespaces show in Simple only, so
 * all of them is held to the banned list. To ban a word, add it here.
 */
import { describe, expect, it } from 'vitest';
import { en as importEn } from '../src/i18n/namespaces/import';
import { en as advancedEn } from '../src/i18n/namespaces/import-advanced';
import { en as listsEn } from '../src/i18n/namespaces/import-lists';
import { en as runEn } from '../src/i18n/namespaces/import-run';
import { en as simple } from '../src/i18n/namespaces/import-simple';
import { en as structure } from '../src/i18n/namespaces/import-structure';
import { ml as mlImport } from '../src/i18n/namespaces/import/locales/ml';
import { ml as mlAdvanced } from '../src/i18n/namespaces/import-advanced/locales/ml';
import { ml as mlLists } from '../src/i18n/namespaces/import-lists/locales/ml';
import { ml as mlRun } from '../src/i18n/namespaces/import-run/locales/ml';
import { ml as mlSimple } from '../src/i18n/namespaces/import-simple/locales/ml';
import { ml as mlStructure } from '../src/i18n/namespaces/import-structure/locales/ml';

const en: Readonly<Record<string, string>> = { ...importEn, ...listsEn };

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
const bannedIn = (key: string, text: string, advanced = key.startsWith('advanced.')): readonly string[] => {
  const shown = text.replace(/\{[A-Za-z][A-Za-z0-9]*\}/g, ' ');
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

  it.each([
    ['import', en],
    ['import-run', runEn],
  ])('every English string of %s passes its mode’s word list', (_name, table: Readonly<Record<string, string>>) => {
    const found = Object.entries(table).flatMap(([key, text]) => bannedIn(key, text));
    expect(found).toEqual([]);
    expect(Object.keys(table).length).toBeGreaterThan(0);
  });

  it.each([
    ['import-simple', simple],
    ['import-structure', structure],
  ])('every %s string passes Simple’s word list', (_name, table: Readonly<Record<string, string>>) => {
    const found = Object.entries(table).flatMap(([key, text]) => bannedIn(key, text));
    expect(found).toEqual([]);
    expect(Object.keys(table).every((key) => !key.startsWith('advanced.'))).toBe(true);
  });

  it('every English string of the Advanced screens passes the Advanced word list', () => {
    expect(bannedIn('adv.x', 'A slot rule', true)).toHaveLength(1);
    expect(bannedIn('adv.x', 'The flag kind and its policy', true)).toEqual([]);
    const found = Object.entries(advancedEn).flatMap(([key, text]) => bannedIn(key, text, true));
    expect(found).toEqual([]);
    expect(Object.keys(advancedEn).length).toBeGreaterThan(0);
  });
});

/**
 * One spelling of a term in each locale. A machine-translated table can spell
 * a word two ways from one run to the next; the hand correction in the table
 * survives a rerun, and this keeps a new key from bringing the other back.
 */
const SPELLINGS: readonly { readonly locale: string; readonly table: Readonly<Record<string, string>>; readonly term: string; readonly not: RegExp }[] = [
  ...[mlImport, mlAdvanced, mlLists, mlRun, mlSimple, mlStructure].map((table) => ({ locale: 'ml', table, term: 'പാറ്റേൺ', not: /പാറ്തേ/u })),
];

describe('import strings spell a term one way in each locale', () => {
  it('finds no other spelling of the term', () => {
    const found = SPELLINGS.flatMap(({ locale, table, term, not }) =>
      Object.entries(table)
        .filter(([, text]) => not.test(text))
        .map(([key, text]) => `${locale} ${key}: not "${term}" in "${text}"`),
    );
    expect(found).toEqual([]);
  });
});
