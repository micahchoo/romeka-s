/**
 * Check in Simple, rendered over the fake server: the title question offers
 * the item code for items of many files, a file the list names and the
 * install refuses is a fact with a link and no question, and the unmatched
 * files question speaks plainly when nobody marked an example.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import type { Scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { loadFixture } from './loadFixture';
import { installFetch } from './stub';

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};

const savedPlan = asRecord(asRecord(loadFixture('import/import-saved.json'), [])['plan'], []);
const savedSheets = savedPlan['sheets'];

/** A list is read, so the flags that need one are counted. */
const LISTED = {
  sheets: Array.isArray(savedSheets) ? savedSheets.slice(0, 1) : [],
};

const items = (files: '1' | '3'): Scenario => ({
  ...scenario('bhc', 9),
  groups: [
    {
      files,
      folder: 'Artifacts',
      media: 'image',
      row: true,
      count: 4,
      from: 0,
    },
  ],
  flags: [{ kind: 'empty-required', slot: 'dcterms:title', count: 4, detail: null }],
});

const openCheck = (server: ReturnType<typeof simpleServer>): void => {
  installFetch(server.handler);
  render(<ImportPage param="1" rest={['1', 'simple', 'check']} />);
};

const titleSources = (plan: Record<string, unknown>): unknown => {
  const item = asRecord(asRecord(plan['slots'], [])['item'], []);
  return asRecord(item['dcterms:title'] ?? { sources: [] }, [])['sources'];
};

afterEach(() => {
  window.location.hash = '';
});

describe('Check: the title', () => {
  it('suggests the item code for items of three files, and choosing it saves a title from the identifier group', async () => {
    const server = simpleServer(items('3'), LISTED);
    openCheck(server);
    const card = await screen.findByRole('article', { name: /^4 items have no title/ }, { timeout: 5000 });
    const code = within(card).getByRole('radio', {
      name: /^Use the item code as the title/,
    });
    expect(code.closest('label')).toHaveTextContent('suggested');
    expect(code).toBeChecked();
    fireEvent.click(within(card).getByRole('button', { name: 'Use the suggested answer' }));
    await waitFor(
      () => {
        expect(titleSources(last(server.saved))).toEqual([{ from: 'group', name: 'identifier' }]);
      },
      { timeout: 3000 },
    );
  }, 20_000);

  it('keeps the file name on offer, and the untitled answer truthful, for items of one file', async () => {
    openCheck(simpleServer(items('1'), LISTED));
    const card = await screen.findByRole('article', { name: /^4 items have no title/ }, { timeout: 5000 });
    expect(
      within(card)
        .getByRole('radio', { name: /^Add them without a title/ })
        .closest('label'),
    ).toHaveTextContent('suggested');
    expect(
      within(card).getByRole('radio', {
        name: 'Use the file name as the title',
      }),
    ).toBeInTheDocument();
    expect(card).not.toHaveTextContent('Omeka shows the identifier');
  }, 20_000);
});

describe('Check: a file the list names and the install refuses', () => {
  it('shows its count, an example and the link to the allowed file types, and asks nothing', async () => {
    const base = scenario('bhc', 9);
    openCheck(
      simpleServer(
        {
          ...base,
          flags: [
            {
              kind: 'listed-not-media',
              slot: null,
              count: 3,
              detail: 'BHC006_GMT_AF_296#1 .nef',
            },
          ],
        },
        LISTED,
      ),
    );
    const fact = await screen.findByRole('region', { name: /^3 files in your list/ }, { timeout: 5000 });
    expect(fact).toHaveTextContent('BHC006_GMT_AF_296#1 .nef');
    expect(within(fact).getByRole('link', { name: /allowed file types/ })).toHaveAttribute('href', '#/settings');
    expect(within(fact).queryByRole('radio')).toBeNull();
    expect(within(fact).queryByRole('button')).toBeNull();
    expect(screen.queryByRole('article', { name: /files in your list/ })).toBeNull();
  }, 20_000);
});

describe('Check: files that do not fit', () => {
  const stray: Scenario = {
    ...scenario('bhc', 9),
    unmatchedFiles: 2,
    flags: [],
  };

  it('speaks of how the other files are grouped when nobody marked an example', async () => {
    openCheck(simpleServer(stray, LISTED));
    expect(await screen.findByText('2 files have a name that does not fit how the other files are grouped into items.', {}, { timeout: 5000 })).toBeInTheDocument();
  }, 20_000);

  it('speaks of the marked example when one is marked', async () => {
    const example = {
      path: 'a_front.jpg',
      marks: [{ kind: 'words', words: ['front'], sep: '_' }],
    };
    openCheck(simpleServer(stray, { ...LISTED, example }));
    expect(await screen.findByText('2 files have a name that does not fit the example you marked.', {}, { timeout: 5000 })).toBeInTheDocument();
  }, 20_000);
});

describe('Add: a title question nobody answered', () => {
  const openAdd = (server: ReturnType<typeof simpleServer>): void => {
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'add']} />);
  };

  it('saves the suggested item code as the title once, and says so with a way back to Check', async () => {
    const server = simpleServer(items('3'), LISTED);
    openAdd(server);
    const line = await screen.findByText(/get their item code as the title/, {}, { timeout: 5000 });
    expect(within(line.closest('li') ?? line).getByRole('link', { name: 'Change it' })).toHaveAttribute('href', '#/import/1/simple/check');
    await waitFor(
      () => {
        expect(titleSources(last(server.saved))).toEqual([{ from: 'group', name: 'identifier' }]);
      },
      { timeout: 3000 },
    );
    const saves = server.saved.length;
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(server.saved).toHaveLength(saves);
  }, 20_000);

  it('says the title comes from the item code on a later visit too, and saves nothing', async () => {
    const slots = { item: { 'dcterms:title': { sources: [{ from: 'group', name: 'identifier' }], combine: 'first' } }, media: {}, itemSet: {} };
    const server = simpleServer(items('3'), { ...LISTED, slots, simple: { answered: ['empty-required'] } });
    openAdd(server);
    await screen.findByText(/get their item code as the title/, {}, { timeout: 5000 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    expect(server.saved.map(titleSources).filter((sources) => Array.isArray(sources) && sources.length > 0)).toEqual([]);
  }, 20_000);

  it('writes no title rule for a person who answered "without a title"', async () => {
    const answered = { simple: { answered: ['empty-required'] } };
    const server = simpleServer(items('3'), { ...LISTED, ...answered });
    openAdd(server);
    await screen.findByText(/^Ready to add/, {}, { timeout: 5000 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    expect(screen.queryByText(/get their item code as the title/)).toBeNull();
    expect(server.saved.map(titleSources).filter((sources) => Array.isArray(sources) && sources.length > 0)).toEqual([]);
  }, 20_000);
});
