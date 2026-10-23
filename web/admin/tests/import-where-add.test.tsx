/**
 * What "Where they go" chose, now on Structure, and "Add", read back from
 * the plan they save and the facts they show: the archive's own sets save
 * each set source as a free set of its own, a new set takes its title, the
 * item type source is one select on the folder's node, and Add shows the
 * check as label and value rows.
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

const SETS = { slot: 'sets', values: { total: 1, page: 1, perPage: 16, items: [{ text: 'Citizen Curators', count: 512, spellings: ['Citizen Curators'], to: null }] }, near: [] };

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

describe('the archive’s own sets, where “Where they go” chose the item sets', () => {
  const COLUMN = { from: 'column', sheet: 'register', column: { index: 1, header: 'Collection' } };
  const each = (...sources: unknown[]) => ({ each: { sources, combine: 'first', names: 'title' } });
  /** The plan's free sets: in v3 every item set the import adds is one, never a series' rule. */
  const freeSets = (plan: Record<string, unknown>): unknown => asRecord(plan['structure'], [])['freeSets'];
  const open = async (): Promise<{ readonly server: ReturnType<typeof simpleServer>; readonly sets: HTMLElement }> => {
    const slots = { item: { sets: { sources: [COLUMN], combine: 'first' } }, media: {}, itemSet: {} };
    const server = simpleServer(scenario('meera', 9), { slots });
    installFetch(withSetValues(server));
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    // The old step's address lands on Structure; the archive's node opens its own sets.
    const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    // The root first; the one folder over everything is named like it.
    fireEvent.click(within(tree).getAllByRole('option', { name: 'All of archive' })[0] ?? tree);
    fireEvent.click(await screen.findByRole('button', { name: 'Show and change them' }, { timeout: 5000 }));
    return { server, sets: await screen.findByRole('region', { name: 'Your own sets' }, { timeout: 5000 }) };
  };
  const add = async (server: ReturnType<typeof simpleServer>, sets: HTMLElement, kind: string, fill: (form: HTMLElement) => void, after: unknown): Promise<void> => {
    const form = within(sets).getByRole('form', { name: 'Add a set' });
    fireEvent.change(within(form).getByRole('combobox', { name: 'Add a set' }), { target: { value: kind } });
    fill(within(sets).getByRole('form', { name: 'Add a set' }));
    fireEvent.click(within(within(sets).getByRole('form', { name: 'Add a set' })).getByRole('button', { name: 'Use' }));
    await waitFor(() => {
      expect(freeSets(last(server.saved))).toEqual(after);
    }, { timeout: 3000 });
    expect(JSON.stringify(asRecord(last(server.saved)['structure'], [])['levels'])).not.toContain('"sets"');
  };
  const valuesFrom = (name: string) => (form: HTMLElement): void => {
    const from = within(form).getByRole('combobox', { name: 'Values from' });
    fireEvent.change(from, { target: { value: within(from).getByRole('option', { name }).getAttribute('value') } });
  };

  it('saves the sets each choice names beside the list column’s: per folder, per top folder, one set by its title, one in Omeka by its id', async () => {
    const { server, sets } = await open();
    expect(within(sets).getByText('One set per value of List register, column “Collection”')).toBeInTheDocument();
    const path = { from: 'folder', level: 'path' };
    const top = { from: 'folder', level: 1 };
    await add(server, sets, 'each', valuesFrom('Folder path'), [each(COLUMN), each(path)]);
    await add(server, sets, 'each', valuesFrom('Folder level 1'), [each(COLUMN), each(path), each(top)]);
    await add(server, sets, 'title', (form) => {
      fireEvent.change(within(form).getByRole('textbox', { name: 'Title' }), { target: { value: 'archive' } });
    }, [each(COLUMN), each(path), each(top), { title: 'archive', description: null }]);
    await add(server, sets, 'existing', (form) => {
      const which = within(form).getByRole('combobox', { name: 'Set in Omeka' });
      fireEvent.change(which, { target: { value: within(which).getAllByRole('option')[1]?.getAttribute('value') } });
    }, [each(COLUMN), each(path), each(top), { title: 'archive', description: null }, { id: 1 }]);
    // "None": each one removed.
    for (let left = 5; left > 0; left -= 1) {
      fireEvent.click(within(sets).getAllByRole('button', { name: 'Remove' })[0] ?? sets);
      await waitFor(() => {
        expect(freeSets(last(server.saved))).toHaveLength(left - 1);
      }, { timeout: 3000 });
    }
    expect(await within(sets).findByText('You have no set of your own.')).toBeInTheDocument();
  }, 20_000);

  it('names one new set by its title, and an empty title saves nothing', async () => {
    const { server, sets } = await open();
    const form = within(sets).getByRole('form', { name: 'Add a set' });
    fireEvent.change(within(form).getByRole('combobox', { name: 'Add a set' }), { target: { value: 'title' } });
    const title = within(form).getByRole('textbox', { name: 'Title' });
    fireEvent.change(title, { target: { value: '  ' } });
    expect(within(form).getByRole('button', { name: 'Use' })).toBeDisabled();
    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(server.saved).toHaveLength(0);
    fireEvent.change(title, { target: { value: 'Citizen Curators' } });
    fireEvent.click(within(form).getByRole('button', { name: 'Use' }));
    await waitFor(() => {
      expect(freeSets(last(server.saved))).toEqual([each(COLUMN), { title: 'Citizen Curators', description: null }]);
    }, { timeout: 3000 });
  }, 20_000);

  it('says what a set per value makes: each set with its items', async () => {
    const { sets } = await open();
    const row = within(sets).getByText('One set per value of List register, column “Collection”').closest('li');
    await waitFor(() => {
      expect(row).toHaveTextContent('Citizen Curators');
    }, { timeout: 3000 });
    expect(row).toHaveTextContent('512');
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
