/**
 * Simple's words are dials, not prose: no English string of `import-simple`,
 * `import-structure`, `import-fixes` or `import-lists` is longer than 12 words, and neither
 * is any string Simple's Structure shows from another namespace: the
 * archive's "Your own sets" is Advanced's control (`advanced/free-sets.tsx`),
 * so the test opens it in Simple and counts every text it draws. A word is a
 * run of characters between spaces, and a `{name}` placeholder counts as one.
 *
 * `LONG` names every string allowed to be longer, by key. It may only shrink.
 * It holds only the help a screen keeps inside its one "What do these mean?"
 * `<details>`.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { en as fixes } from '../src/i18n/namespaces/import-fixes';
import { en as lists } from '../src/i18n/namespaces/import-lists';
import { en as simple } from '../src/i18n/namespaces/import-simple';
import { en as structure } from '../src/i18n/namespaces/import-structure';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const MOST_WORDS = 12;

const LONG: ReadonlySet<string> = new Set([
  // Check: the help inside "What do these mean?".
  'guided.check.once',
]);

const wordsIn = (text: string): number => text.split(/\s+/).filter((word) => word !== '').length;

const TABLES: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  'import-simple': simple,
  'import-structure': structure,
  'import-lists': lists,
  'import-fixes': fixes,
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
    const all: Readonly<Record<string, string>> = { ...simple, ...structure, ...lists, ...fixes };
    const stale = [...LONG].filter((key) => all[key] === undefined || wordsIn(all[key]) <= MOST_WORDS);
    expect(stale).toEqual([]);
  });
});

/** BHC006's proposal over a draft whose own sets are one by title and one lifted from v2, which shows its two readings. */
const ownSets = (): void => {
  const detail = asRecord(loadFixture('import/import-draft.json'), []);
  const plan = asRecord(detail['plan'], []);
  const proposal = asRecord(loadFixture('import/proposal-bhc006.json'), []);
  const freeSets = [
    { title: 'Exhibition 2018', description: null },
    { each: { sources: [{ from: 'folder', level: 1 }], combine: 'first', names: 'text' } },
  ];
  const stored = { ...detail, plan: { ...plan, structure: { ...asRecord(plan['structure'], []), freeSets } } };
  const fixture = (name: string, section: string): unknown => asRecord(loadFixture(`import/${name}`), [])[section];
  installFetch(
    route({
      '/api/romeka/imports/1': () => jsonResponse(stored),
      '/api/romeka/imports/1/preview': (_url, init) => {
        const asked = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
        const include = Array.isArray(asked['include']) ? asked['include'].map(String) : [];
        const answer: Record<string, unknown> = { sources: fixture('preview-check.json', 'sources'), pattern: { groups: ['identifier'] } };
        for (const section of include) {
          answer[section] = section === 'proposal' ? proposal : fixture(`preview-${section.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}.json`, section);
        }
        return jsonResponse(answer);
      },
      '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
      '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
      '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
    }),
  );
};

/** Every text the element draws, one string per text node, and each option of its selects. */
const textsIn = (element: HTMLElement): readonly string[] => {
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
  const texts: string[] = [];
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    const text = node.textContent?.trim() ?? '';
    if (text !== '') {
      texts.push(text);
    }
  }
  return texts;
};

describe('the strings Simple’s Structure shows from Advanced', () => {
  afterEach(() => {
    cleanup();
  });

  it('“Your own sets” on the archive’s node draws no text longer than 12 words, in every kind of set it adds', async () => {
    ownSets();
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    fireEvent.click(await screen.findByRole('option', { name: 'All of archive' }, { timeout: 5000 }));
    fireEvent.click(await screen.findByRole('button', { name: 'Show and change them' }, { timeout: 5000 }));
    const sets = await screen.findByRole('region', { name: 'Your own sets' }, { timeout: 5000 });
    // The lifted set's two readings show once its values are counted.
    await within(sets).findByRole('button', { name: 'Read them as titles' }, { timeout: 5000 });
    const add = within(sets).getByRole('form', { name: 'Add a set' });
    const kinds = within(add).getByRole('combobox', { name: 'Add a set' });
    const shown = new Set(textsIn(sets));
    for (const option of within(kinds).getAllByRole('option')) {
      fireEvent.change(kinds, { target: { value: option.getAttribute('value') } });
      await waitFor(() => {
        expect(kinds).toHaveProperty('value', option.getAttribute('value'));
      });
      for (const text of textsIn(sets)) {
        shown.add(text);
      }
    }
    expect(shown).toContain('Folder path');
    const long = [...shown].filter((text) => wordsIn(text) > MOST_WORDS).map((text) => `${text}: ${String(wordsIn(text))} words`);
    expect(long).toEqual([]);
  }, 20_000);
});
