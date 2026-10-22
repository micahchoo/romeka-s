/**
 * Simple's words are dials, not prose: no English string of `import-simple`,
 * `import-structure` or `import-lists` is longer than 12 words. A word is a run of characters
 * between spaces, and a `{name}` placeholder counts as one.
 *
 * `LONG` names every string allowed to be longer, by key. It may only shrink.
 * It holds only the help a screen keeps inside its one "What do these mean?"
 * `<details>`.
 */
import { describe, expect, it } from 'vitest';
import { en as lists } from '../src/i18n/namespaces/import-lists';
import { en as simple } from '../src/i18n/namespaces/import-simple';
import { en as structure } from '../src/i18n/namespaces/import-structure';

const MOST_WORDS = 12;

const LONG: ReadonlySet<string> = new Set([
  // Your list: the help inside "What do these mean?".
  'guided.list.helpPaired',
  'guided.list.codeGloss',
  'guided.list.helpRowIs',
  'guided.list.columnsLede',
  'guided.list.levelLede',
  'guided.list.moreFields',
  // What we found and Check: the help inside "What do these mean?".
  'guided.found.gloss',
  'guided.found.now',
  'guided.found.helpGroups',
  'guided.check.once',
  // Where they go: the help inside "What do these mean?".
  'guided.where.typesGloss',
  'guided.where.valuesNote',
]);

const wordsIn = (text: string): number => text.split(/\s+/).filter((word) => word !== '').length;

const TABLES: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  'import-simple': simple,
  'import-structure': structure,
  'import-lists': lists,
};

describe('Simple’s strings are at most 12 words', () => {
  it('counts the words of a string, a placeholder as one', () => {
    expect(wordsIn('{count} rows name another file of an item')).toBe(8);
    expect(wordsIn('  Use  this list ')).toBe(3);
  });

  it.each(Object.keys(TABLES))('%s: every English string but the listed help is at most 12 words', (name) => {
    const table = TABLES[name] ?? {};
    const long = Object.entries(table)
      .filter(([key, text]) => !LONG.has(key) && wordsIn(text) > MOST_WORDS)
      .map(([key, text]) => `${key}: ${String(wordsIn(text))} words`);
    expect(long).toEqual([]);
  });

  it('lists only keys that exist and are still long, so the list only shrinks', () => {
    const all: Readonly<Record<string, string>> = { ...simple, ...structure, ...lists };
    const stale = [...LONG].filter((key) => all[key] === undefined || wordsIn(all[key]) <= MOST_WORDS);
    expect(stale).toEqual([]);
  });
});
