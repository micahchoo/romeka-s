/**
 * Single answers on the Simple screens, read back from the plan they save:
 * What we found lists the groups beside the inspector of one and leaves a
 * group out after a confirmation, turning down the folder's list keeps it
 * out, an item type value maps to a new item type or to none, and Check lists
 * its questions and facts beside the inspector of one, suggests each kind's
 * default policy and saves an answer given there.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { bhc006, IN_USE } from './import-bhc006-lists';
import { scenario } from './import-simple-scenarios';
import { PATH, simpleServer } from './import-simple-server';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';

const subscribe = (listener: () => void): (() => void) => {
  window.addEventListener('hashchange', listener);
  return () => {
    window.removeEventListener('hashchange', listener);
  };
};

const Routed = (): ReactElement => {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  const rest = hash.replace(/^#\/import\/?/, '').split('/').filter((part) => part !== '');
  return <ImportPage param={rest[0] ?? null} rest={rest} />;
};

const go = (screenName: string): void => {
  window.location.hash = `#/import/1/simple/${screenName}`;
  window.dispatchEvent(new HashChangeEvent('hashchange'));
};

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};

/** The "use this list" box of the folder's one list, which "Your list" selects first. */
const useBox = async (): Promise<HTMLElement> =>
  within(await screen.findByRole('region', { name: 'register.csv' }, { timeout: 5000 })).getByRole('checkbox', { name: 'use this list' });

const sheetsOf = (plan: Record<string, unknown>): readonly unknown[] => (Array.isArray(plan['sheets']) ? plan['sheets'] : []);

/** Opens "What we found", which sets the pattern and reads the folder's list, and waits for both to be saved. */
const opened = async (): Promise<ReturnType<typeof simpleServer>> => {
  const server = simpleServer(scenario('meera', 9));
  installFetch(server.handler);
  window.location.hash = '#/import/1/simple/found';
  render(<Routed />);
  await waitFor(() => {
    expect(sheetsOf(last(server.saved))).toHaveLength(1);
  }, { timeout: 5000 });
  return server;
};

afterEach(() => {
  window.location.hash = '';
});

describe('What we found', () => {
  it('writes nothing on the first visit: the draft already groups by the name alone', async () => {
    const server = simpleServer(scenario('meera', 9));
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'found']} />);
    expect(await screen.findByText('Items: names equal but for the ending.', {}, { timeout: 5000 })).toBeInTheDocument();
    const patterns = server.saved.map((plan) => asRecord(plan['grouping'], [])['pattern']);
    expect(patterns.every((pattern) => pattern === asRecord(asRecord(asRecord(loadFixture('import/import-draft.json'), [])['plan'], [])['grouping'], [])['pattern'])).toBe(true);
  });

  it('says a pattern cleared in Advanced makes no items, and offers one item per name', async () => {
    const server = simpleServer(scenario('meera', 9));
    const draft = asRecord(loadFixture('import/import-draft.json'), []);
    const plan = asRecord(draft['plan'], []);
    const cleared = { ...draft, plan: { ...plan, grouping: { ...asRecord(plan['grouping'], []), pattern: null } } };
    installFetch((url, init) =>
      url.pathname === PATH && init?.method !== 'PUT' && server.saved.length === 0 ? jsonResponse(cleared) : server.handler(url, init),
    );
    render(<ImportPage param="1" rest={['1', 'simple', 'found']} />);
    expect(await screen.findByText('Files are not put into items. Advanced mode chose this.', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(server.saved).toHaveLength(0);
    fireEvent.click(screen.getByRole('button', { name: 'Make one item per name' }));
    await waitFor(() => {
      expect(String(asRecord(last(server.saved)['grouping'], [])['pattern'])).toContain('(?<identifier>');
    }, { timeout: 3000 });
  });
});

/** "What we found" over BHC006's two workbooks in use, before any example: four groups of one file each. */
const openFound = (): ReturnType<typeof simpleServer> => {
  const server = simpleServer(bhc006(), IN_USE);
  installFetch(server.handler);
  render(<ImportPage param="1" rest={['1', 'simple', 'found']} />);
  return server;
};

describe('What we found, as a list of groups and an inspector', () => {
  it('counts the files, items and files left out in one line; the kinds of problem show once, in the status line', async () => {
    openFound();
    const files = await screen.findByText('Files', { selector: 'span' }, { timeout: 5000 });
    const line = files.parentElement ?? files;
    expect(line).toHaveTextContent('Files 16');
    expect(line).toHaveTextContent('Items 4');
    expect(line).toHaveTextContent('Files left out 1');
    expect(line).not.toHaveTextContent('Kinds of problem');
    expect(screen.getAllByText(/7 kinds of problem/)).toHaveLength(1);
  }, 20_000);

  it('names each group by chips of its facts on the left, and shows the selected group’s examples and actions on the right', async () => {
    openFound();
    const groups = await screen.findByRole('listbox', { name: 'Your items in groups' }, { timeout: 5000 });
    expect(within(groups).getAllByRole('option').map((option) => option.getAttribute('aria-selected'))).toEqual(['true', 'false', 'false', 'false']);
    expect(within(groups).getByRole('option', { name: '1 file, pictures, row, Photographs' })).toHaveTextContent('1 item');
    expect(within(groups).getByRole('option', { name: '1 file, pictures, no row, _scratch' })).toBeInTheDocument();

    fireEvent.click(within(groups).getByRole('option', { name: '1 file, pictures, no row, _scratch' }));
    const group = await screen.findByRole('region', { name: '1 item' });
    expect(group).toHaveTextContent('bhc_7');
    expect(group).toHaveTextContent('bhc_7_0.jpg');
    expect(within(group).getByRole('link', { name: 'Show all 1' }).getAttribute('href')).toMatch(/^#\/import\/1\/simple\/all-items\//);
    expect(within(group).getByRole('button', { name: 'Leave out' })).toBeEnabled();
    expect(within(group).queryByRole('button', { name: 'Each file its own item' })).toBeNull();
  }, 20_000);

  it('leaves a group out only after naming what changes, and saves one decision for its items', async () => {
    const server = openFound();
    const groups = await screen.findByRole('listbox', { name: 'Your items in groups' }, { timeout: 5000 });
    fireEvent.click(within(groups).getByRole('option', { name: '1 file, pictures, no row, _scratch' }));
    const group = await screen.findByRole('region', { name: '1 item' });
    fireEvent.click(within(group).getByRole('button', { name: 'Leave out' }));
    expect(await within(group).findByText('This changes 1: bhc_200001.', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(server.saved.some((plan) => Array.isArray(plan['decisions']) && plan['decisions'].length > 0)).toBe(false);
    fireEvent.click(within(group).getByRole('button', { name: 'Change these 1' }));
    await waitFor(() => {
      expect(last(server.saved)['decisions']).toEqual([
        { kind: 'many', entries: [{ file: { source: 's1', path: '_scratch/bhc_200001_0.jpg' } }], change: { kind: 'leave' } },
      ]);
    }, { timeout: 3000 });
  }, 20_000);
});

describe('Your list', () => {
  it('uses the folder’s list as the suggested answer, and turning “use this list” off takes it out and keeps it out', async () => {
    const server = await opened();
    expect(String(asRecord(last(server.saved)['grouping'], [])['pattern'])).toContain('(?<identifier>');
    go('list');
    fireEvent.click(await useBox());
    await waitFor(() => {
      expect(sheetsOf(last(server.saved))).toHaveLength(0);
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: 'register.csv' }]);
    expect(last(server.saved)['example']).toBeNull();
    expect(await screen.findByRole('option', { name: 'register.csv' })).toHaveTextContent('Turned down');
    expect(await useBox()).not.toBeChecked();
    expect(await useBox()).toBeEnabled();

    const saves = server.saved.length;
    go('found');
    expect(await screen.findByRole('listbox', { name: 'Your items in groups' }, { timeout: 5000 })).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(server.saved).toHaveLength(saves);
  }, 20_000);
});

describe('Where they go', () => {
  it('shows the list column that feeds the item sets as the pressed choice, and “None” takes it out', async () => {
    const column = { index: 1, header: 'Collection' };
    const slots = { item: { sets: { sources: [{ from: 'column', sheet: 'register', column }], combine: 'first' } }, media: {}, itemSet: {} };
    const server = simpleServer(scenario('meera', 9), { slots });
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    const dial = await screen.findByRole('group', { name: 'Item sets' }, { timeout: 5000 });
    expect(within(dial).getByRole('button', { name: 'Column “Collection”' })).toHaveAttribute('aria-pressed', 'true');
    expect(within(dial).getByRole('button', { name: 'None' })).toHaveAttribute('aria-pressed', 'false');
    fireEvent.click(within(dial).getByRole('button', { name: 'None' }));
    await waitFor(() => {
      expect(asRecord(asRecord(last(server.saved)['slots'], [])['item'], [])['sets']).toBeUndefined();
    }, { timeout: 3000 });
    expect(within(dial).queryByRole('button', { name: 'Column “Collection”' })).toBeNull();
    expect(within(dial).getByRole('button', { name: 'None' })).toHaveAttribute('aria-pressed', 'true');
  }, 20_000);

  it('maps a value to a new item type, and another to no item type', async () => {
    const server = await opened();
    go('list');
    const type = await screen.findByRole('combobox', { name: 'Where the column Type goes' }, { timeout: 5000 });
    fireEvent.change(type, { target: { value: 'template' } });
    go('where');
    const letter = await screen.findByRole('combobox', { name: 'Item type for letter' }, { timeout: 5000 });
    expect(letter).toHaveValue('');
    fireEvent.change(letter, { target: { value: within(letter).getByRole('option', { name: 'A new item type “letter”' }).getAttribute('value') } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Item type for magazine' }), { target: { value: 'none' } });
    await waitFor(() => {
      const item = asRecord(asRecord(last(server.saved)['slots'], [])['item'], []);
      expect(asRecord(item['template'], [])['map']).toEqual({ letter: 'letter', magazine: '' });
    }, { timeout: 3000 });
    expect(last(server.saved)['newTypes']).toEqual([{ label: 'letter', from: 'mapping', class: null }]);
  }, 20_000);
});

describe('Simple and Advanced read one plan', () => {
  const NOTICE = 'An example is kept, but not in a form this screen can show.';

  it('Simple’s answers do not make Advanced report a kept example, and they are still there on a reload', async () => {
    const server = await opened();
    go('list');
    fireEvent.click(await useBox());
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['notLists']).toHaveLength(1);
    }, { timeout: 3000 });
    go('check');
    fireEvent.click(await screen.findByRole('button', { name: /^Use the \d+ suggested answers?$/ }, { timeout: 5000 }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['answered']).not.toEqual([]);
    }, { timeout: 3000 });
    expect(last(server.saved)['example']).toBeNull();

    window.location.hash = '#/import/1/advanced/files';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    expect(await screen.findByText('None. Files are one item only when their names are equal except for the extension.', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.queryByText(NOTICE)).toBeNull();

    go('list');
    expect(await screen.findByRole('option', { name: 'register.csv' }, { timeout: 5000 })).toHaveTextContent('Turned down');
  }, 30_000);
});

/** Check over BHC006's two workbooks in use. */
const openCheck = (plan: Record<string, unknown> = IN_USE): ReturnType<typeof simpleServer> => {
  const server = simpleServer(bhc006(), plan);
  installFetch(server.handler);
  render(<ImportPage param="1" rest={['1', 'simple', 'check']} />);
  return server;
};

const question = async (name: string): Promise<HTMLElement> => {
  const questions = await screen.findByRole('listbox', { name: 'Questions' }, { timeout: 5000 });
  fireEvent.click(within(questions).getByRole('option', { name }));
  return within(questions).getByRole('option', { name });
};

const OPEN = ['cells-count', 'no-file', 'itemsWithoutRow', 'exists', 'date-form', 'untitled', 'unknown-type'];

describe('Check', () => {
  it('lists every question and every fact on the left, each with its count and state', async () => {
    openCheck();
    const questions = await screen.findByRole('listbox', { name: 'Questions' }, { timeout: 5000 });
    expect(within(questions).getAllByRole('option').map((option) => option.textContent)).toEqual([
      'A row with the wrong number of cells1open',
      'A row that names no file of this import1open',
      'Items with no row210open',
      'Already in Omeka1open',
      'A date not in a form Omeka can sort1open',
      'Items with no title2open',
      'An item type that Omeka does not have9open',
      'A file your list names that Omeka does not take82fact',
      'Not a picture, a sound, a film or a PDF1fact',
    ]);
    const counts = screen.getByText('Items to add', { selector: 'span' }).parentElement;
    expect(counts).toHaveTextContent('Items to add 4');
    expect(counts).toHaveTextContent('Questions open 7');
    expect(counts).toHaveTextContent('Answered 0');
  }, 20_000);

  it('shows a fact with no answer to give, and the way to the setting behind it', async () => {
    openCheck();
    await question('A file your list names that Omeka does not take');
    const fact = await screen.findByRole('region', { name: '82 files in your list are of a type Omeka refuses.' });
    expect(fact).toHaveTextContent('BHC006_GMT_AF_296a.NEF');
    expect(within(fact).getByRole('link', { name: /allowed file types/ })).toHaveAttribute('href', '#/settings');
    expect(within(fact).queryByRole('button')).toBeNull();
  }, 20_000);

  it('saves an answer chosen in the inspector, the same policy and the same “answered” as before', async () => {
    const server = openCheck();
    const option = await question('Already in Omeka');
    const known = await screen.findByRole('region', { name: '1 item is already in Omeka.' });
    const answers = within(known).getByRole('group', { name: 'Answer for all 1' });
    expect(within(answers).getByRole('button', { name: 'Skip them suggested' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(within(answers).getByRole('button', { name: 'Add them again' }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['flags'], [])['exists']).toBe('keep');
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['simple'], [])['answered']).toEqual(['exists']);
    await waitFor(() => {
      expect(option).toHaveTextContent('answered');
    }, { timeout: 3000 });
  }, 20_000);

  it('“Use the 7 suggested answers” answers every open question with its suggestion', async () => {
    const server = openCheck();
    fireEvent.click(await screen.findByRole('button', { name: 'Use the 7 suggested answers' }, { timeout: 5000 }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['answered']).toEqual(OPEN);
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['flags'], [])).toEqual({});
    expect(await screen.findByText('Every question has an answer.', {}, { timeout: 3000 })).toBeInTheDocument();
    expect(within(screen.getByRole('listbox', { name: 'Questions' })).getAllByText('suggested used')).toHaveLength(OPEN.length);
  }, 20_000);

  it('suggests leaving out files that are not media, the kind’s default, when Advanced chose to keep them', async () => {
    installFetch(simpleServer(scenario('bhc', 9), { flags: { 'not-media': 'keep' } }).handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'check']} />);
    await question('Not a picture, a sound, a film or a PDF');
    const card = await screen.findByRole('region', { name: '1 time: Not a picture, a sound, a film or a PDF.' }, { timeout: 5000 });
    const leave = within(card).getByRole('button', { name: /^Leave out/ });
    const keep = within(card).getByRole('button', { name: /^Keep as they are/ });
    expect(keep).toHaveAttribute('aria-pressed', 'true');
    expect(leave).toHaveAccessibleName('Leave out suggested');
    expect(keep).toHaveAccessibleName('Keep as they are');
  }, 20_000);
});
