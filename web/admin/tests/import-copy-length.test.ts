/**
 * Simple's words are dials, not prose: no English string of `import-simple`
 * or `import-lists` is longer than 12 words. A word is a run of characters
 * between spaces, and a `{name}` placeholder counts as one.
 *
 * `LONG` names every string allowed to be longer, by key. It may only shrink.
 * It holds the help a screen keeps inside its one "What do these mean?"
 * `<details>`, and the strings of screens not yet redesigned, which the next
 * units shrink.
 */
import { describe, expect, it } from 'vitest';
import { en as lists } from '../src/i18n/namespaces/import-lists';
import { en as simple } from '../src/i18n/namespaces/import-simple';

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
  // Where they go and Add: the next units shrink them.
  'guided.where.typesGloss',
  'guided.where.valuesNote',
  'guided.where.spellings',
  'guided.add.inPlace',
  // The review lists (changes, files, groups, items, pairs): the next units shrink them.
  'lists.changes.many',
  'lists.files.lede',
  'lists.groups.lede',
  'lists.groups.odd',
  'lists.items.lede',
  'lists.items.none',
  'lists.pair.lede',
  'lists.pairs.lede',
  'lists.pairs.exact',
  'lists.pairs.why.noItem',
  // "Fill in a list yourself", collapsed under Your list: the next units shrink them.
  'lists.trip.lede',
  'lists.trip.keep',
  'lists.trip.kinds',
  'lists.trip.fills',
  'lists.trip.fillsNone',
  'lists.trip.unused',
  'lists.trip.straysWhy',
]);

const wordsIn = (text: string): number => text.split(/\s+/).filter((word) => word !== '').length;

const TABLES: Readonly<Record<string, Readonly<Record<string, string>>>> = { 'import-simple': simple, 'import-lists': lists };

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
    const all: Readonly<Record<string, string>> = { ...simple, ...lists };
    const stale = [...LONG].filter((key) => all[key] === undefined || wordsIn(all[key]) <= MOST_WORDS);
    expect(stale).toEqual([]);
  });
});
