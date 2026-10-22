/**
 * "Where they go" and "Add" as dials, read back from the plan they save and
 * the facts they show: each item set choice is one segment and saves the
 * item set rule it always did, a new set's name is edited in place, the item
 * type source is one select, and Add shows the check as label and value rows.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { fact } from './fact-rows';
import { scenario } from './import-simple-scenarios';
import { PATH, simpleServer } from './import-simple-server';
import { installFetch } from './stub';
import type { Handler } from './stub';

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};

const itemRule = (plan: Record<string, unknown>, slot: string): unknown => asRecord(asRecord(plan['slots'], [])['item'], [])[slot];

const sourcesOf = (plan: Record<string, unknown>, slot: string): unknown => {
  const rule = itemRule(plan, slot);
  return rule === undefined ? undefined : asRecord(rule, [])['sources'];
};

const SETS = { slot: 'sets', values: { total: 1, page: 1, perPage: 16, items: [{ text: 'Citizen Curators', count: 512, spellings: ['Citizen Curators'] }] } };

/** The fake server, with the item set values the preview counts for every plan; the body is rewritten as it is read. */
const withSetValues =
  (server: ReturnType<typeof simpleServer>): Handler =>
  (url, init) => {
    const answer = server.handler(url, init);
    if (url.pathname !== `${PATH}/preview`) {
      return answer;
    }
    const body = new ReadableStream<Uint8Array>({
      async start(controller) {
        const read = asRecord(await answer.json(), []);
        if (Array.isArray(read['values'])) {
          read['values'] = read['values'].map((one: unknown) => (asRecord(one, [])['slot'] === 'sets' ? SETS : one));
        }
        controller.enqueue(new TextEncoder().encode(JSON.stringify(read)));
        controller.close();
      },
    });
    return new Response(body, { status: answer.status, headers: answer.headers });
  };

afterEach(() => {
  window.location.hash = '';
});

describe('Where they go: item sets as one segmented dial', () => {
  const COLUMN = { from: 'column', sheet: 'register', column: { index: 1, header: 'Collection' } };
  const open = (): ReturnType<typeof simpleServer> => {
    const slots = { item: { sets: { sources: [COLUMN], combine: 'first' } }, media: {}, itemSet: {} };
    const server = simpleServer(scenario('meera', 9), { slots });
    installFetch(withSetValues(server));
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    return server;
  };
  /** The plan's free sets: in v3 every item set the import adds is one, never a series' rule. */
  const freeSets = (plan: Record<string, unknown>): unknown => asRecord(plan['structure'], [])['freeSets'];
  const each = (...sources: unknown[]) => ({ each: { sources, combine: 'first', names: 'title' } });
  const choose = async (server: ReturnType<typeof simpleServer>, name: string, sets: unknown): Promise<void> => {
    const dial = await screen.findByRole('group', { name: 'Item sets' }, { timeout: 5000 });
    fireEvent.click(within(dial).getByRole('button', { name }));
    await waitFor(() => {
      expect(freeSets(last(server.saved))).toEqual(sets);
    }, { timeout: 3000 });
    expect(JSON.stringify(asRecord(last(server.saved)['structure'], [])['levels'])).not.toContain('"sets"');
    expect(within(dial).getByRole('button', { name })).toHaveAttribute('aria-pressed', 'true');
  };

  it('saves the sets each choice names, a list column first: one set by its title, one in Omeka by its id', async () => {
    const server = open();
    const dial = await screen.findByRole('group', { name: 'Item sets' }, { timeout: 5000 });
    expect(within(dial).getByRole('button', { name: 'Column “Collection”' })).toHaveAttribute('aria-pressed', 'true');
    await choose(server, 'Per folder', [each(COLUMN, { from: 'folder', level: 'path' })]);
    await choose(server, 'Per top folder', [each(COLUMN, { from: 'folder', level: 1 })]);
    await choose(server, 'One set', [each(COLUMN), { title: 'archive', description: null }]);
    await choose(server, 'One in Omeka', [each(COLUMN), { id: 1 }]);
    await choose(server, 'Column “Collection”', [each(COLUMN)]);
    fireEvent.click(within(dial).getByRole('button', { name: 'None' }));
    await waitFor(() => {
      expect(freeSets(last(server.saved))).toEqual([]);
    }, { timeout: 3000 });
  }, 20_000);

  it('names the one new set in place, and an empty name saves nothing', async () => {
    const server = open();
    await choose(server, 'One set', [each(COLUMN), { title: 'archive', description: null }]);
    const name = screen.getByRole('textbox', { name: 'Name of the new item set' });
    expect(name).toHaveValue('archive');
    fireEvent.change(name, { target: { value: 'Citizen Curators' } });
    await waitFor(() => {
      expect(freeSets(last(server.saved))).toEqual([each(COLUMN), { title: 'Citizen Curators', description: null }]);
    }, { timeout: 3000 });
    const saves = server.saved.length;
    fireEvent.change(name, { target: { value: '  ' } });
    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(server.saved).toHaveLength(saves);
    expect(name).toHaveValue('  ');
  }, 20_000);

  it('says what the choice makes in one row: the count, then each set with its items', async () => {
    open();
    await screen.findByRole('group', { name: 'Item sets' }, { timeout: 5000 });
    await waitFor(() => {
      expect(fact('Makes')).toHaveTextContent('1 set: Citizen Curators · 512');
    }, { timeout: 3000 });
  }, 20_000);
});

describe('Where they go: item types from one select', () => {
  it('takes the item type from the top folder, then from nowhere', async () => {
    const server = simpleServer(scenario('meera', 9));
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    const from = await screen.findByRole('combobox', { name: 'The item type comes from' }, { timeout: 5000 });
    fireEvent.change(from, { target: { value: within(from).getByRole('option', { name: 'Top folder' }).getAttribute('value') } });
    await waitFor(() => {
      expect(sourcesOf(last(server.saved), 'template')).toEqual([{ from: 'folder', level: 1 }]);
    }, { timeout: 3000 });
    expect(await screen.findByRole('region', { name: 'Item type for each value' }, { timeout: 5000 })).toBeInTheDocument();
    fireEvent.change(from, { target: { value: within(from).getByRole('option', { name: 'None' }).getAttribute('value') } });
    await waitFor(() => {
      expect(itemRule(last(server.saved), 'template')).toBeUndefined();
    }, { timeout: 3000 });
  }, 20_000);
});

describe('Add: the check as label and value rows', () => {
  it('shows items, files, what is left out and the action as rows, with who can see them', async () => {
    const server = simpleServer(scenario('meera', 9));
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'add']} />);
    expect(await screen.findByRole('button', { name: /^Add \d+ items? to Omeka$/ }, { timeout: 5000 })).toBeInTheDocument();
    expect(fact('What it does')).toHaveTextContent(/^Add new items/);
    expect(fact('Items').textContent).toMatch(/^\d+$/);
    expect(fact('Files').textContent).toMatch(/^\d+$/);
    expect(within(fact('Who can see them')).getByRole('combobox', { name: 'Who can see them' })).toHaveValue('private');
  }, 20_000);
});
