/**
 * Single answers, read back from the plan they save, where Simple's old steps
 * asked them: Structure opens without a write; Advanced's archive node lists
 * the groups of items by facts and leaves a group out after a confirmation;
 * the folder's Lists section uses its list, and turning it down keeps it out;
 * the archive's own sets show a list column's sets; the folder's node maps an
 * item type value to a new item type or to none. Check lists its questions
 * and facts beside the inspector of one, suggests each kind's default policy
 * and saves an answer given there.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { bhc006, IN_USE } from './import-bhc006-lists';
import { scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { loadFixture } from './loadFixture';
import { flatPlan } from './import-plan-members';
import { installFetch } from './stub';

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

const go = (place: string): void => {
  window.location.hash = `#/import/1/${place.includes('/') ? place : `simple/${place}`}`;
  window.dispatchEvent(new HashChangeEvent('hashchange'));
};

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};

const sheetsOf = (plan: Record<string, unknown>): readonly unknown[] => (Array.isArray(plan['sheets']) ? plan['sheets'] : []);

/** The folder's one list in its node's Lists section, Advanced's rule zoom. */
const listRow = async (): Promise<HTMLElement> => screen.findByRole('listitem', { name: 'register.csv' }, { timeout: 5000 });

/** Opens the folder's Lists section in Advanced and uses its one list, as "What we found" did on the first visit; waits for it to be saved. */
const opened = async (): Promise<ReturnType<typeof simpleServer>> => {
  const server = simpleServer(scenario('meera', 9));
  installFetch(server.handler);
  window.location.hash = '#/import/1/advanced/lists';
  render(<Routed />);
  fireEvent.click(within(await listRow()).getByRole('button', { name: 'Use this list' }));
  await waitFor(() => {
    expect(sheetsOf(last(server.saved))).toHaveLength(1);
  }, { timeout: 5000 });
  return server;
};

afterEach(() => {
  window.location.hash = '';
});

describe('Structure, where “What we found” was', () => {
  it('writes nothing on the first visit: the draft already groups by the name alone', async () => {
    const server = simpleServer(scenario('meera', 9));
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'found']} />);
    expect(await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 })).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 700));
    const patterns = server.saved.map((plan) => asRecord(plan['grouping'], [])['pattern']);
    const drafted = flatPlan(asRecord(asRecord(loadFixture('import/import-draft.json'), [])['plan'], []));
    expect(patterns.every((pattern) => pattern === asRecord(drafted['grouping'], [])['pattern'])).toBe(true);
  });
});

/** Advanced's archive node over BHC006's two workbooks in use, before any example: four groups of one file each. */
const openGroups = async (): Promise<{ readonly server: ReturnType<typeof simpleServer>; readonly groups: HTMLElement }> => {
  const server = simpleServer(bhc006(), IN_USE);
  installFetch(server.handler);
  render(<ImportPage param="1" rest={['1', 'advanced', 'structure']} />);
  const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
  // The archive's node is the first; the one folder over everything is named like it.
  fireEvent.click(within(tree).getAllByRole('option', { name: 'All of archive' })[0] ?? tree);
  return { server, groups: await screen.findByRole('region', { name: 'Your items in groups' }, { timeout: 5000 }) };
};

/** One group of the archive's groups, by the words that name its facts. */
const groupOf = (groups: HTMLElement, facts: string): HTMLElement => {
  const group = within(groups).getByText((_, element) => element?.tagName === 'P' && element.textContent.startsWith(facts)).closest('article');
  if (!(group instanceof HTMLElement)) {
    throw new Error(`no group ${facts}`);
  }
  return group;
};

describe('the groups of items by facts, on Advanced’s archive node', () => {
  it('counts the files and the items, and the open questions show once, in Simple’s status line', async () => {
    const { groups } = await openGroups();
    expect(groups).toHaveTextContent('Your 4 items make 4 groups with the same facts.');
    expect(screen.getAllByText('16 files · 0 lists')).not.toHaveLength(0);
    cleanup();
    installFetch(simpleServer(bhc006(), IN_USE).handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    // The 7 that Check counts open over the same draft.
    expect(await screen.findAllByText(/7 questions open/, {}, { timeout: 5000 })).toHaveLength(1);
  }, 20_000);

  it('names each group by its facts, and shows its example and actions', async () => {
    const { groups } = await openGroups();
    expect(within(groups).getAllByRole('article')).toHaveLength(4);
    expect(groupOf(groups, '1 item has 1 file, all pictures, a row in your list. They are in the folder “Photographs”.')).toHaveTextContent('bhc_4');
    const scratch = groupOf(groups, '1 item has 1 file, all pictures, no row in your list. They are in the folder “_scratch”.');
    expect(scratch).toHaveTextContent('bhc_7');
    expect(scratch).toHaveTextContent('bhc_7_0.jpg');
    expect(within(scratch).getByRole('link', { name: 'Show all 1' }).getAttribute('href')).toMatch(/^#\/import\/1\/advanced\/items\//);
    expect(within(scratch).getByRole('button', { name: 'Leave them out' })).toBeEnabled();
    expect(within(scratch).queryByRole('button', { name: 'Make each file an item of its own' })).toBeNull();
  }, 20_000);

  it('leaves a group out only after naming what changes, and saves one decision for its items', async () => {
    const { server, groups } = await openGroups();
    const scratch = groupOf(groups, '1 item has 1 file, all pictures, no row in your list. They are in the folder “_scratch”.');
    fireEvent.click(within(scratch).getByRole('button', { name: 'Leave them out' }));
    expect(await within(scratch).findByText('This changes 1: bhc_200001.', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(server.saved.some((plan) => Array.isArray(plan['decisions']) && plan['decisions'].length > 0)).toBe(false);
    fireEvent.click(within(scratch).getByRole('button', { name: 'Change these 1' }));
    await waitFor(() => {
      expect(last(server.saved)['decisions']).toEqual([
        { kind: 'many', entries: [{ file: { source: 's1', path: '_scratch/bhc_200001_0.jpg' } }], change: { kind: 'leave' } },
      ]);
    }, { timeout: 3000 });
  }, 20_000);
});

describe('the folder’s list, in its Lists section', () => {
  it('uses the folder’s list, and turning it down takes it out and keeps it out', async () => {
    const server = await opened();
    expect(String(asRecord(last(server.saved)['grouping'], [])['pattern'])).toContain('(?<identifier>');
    fireEvent.click(within(await listRow()).getByRole('button', { name: 'Stop using it' }));
    await waitFor(() => {
      expect(sheetsOf(last(server.saved))).toHaveLength(0);
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: 'register.csv' }]);
    expect(last(server.saved)['example']).toBeNull();
    await waitFor(async () => {
      expect(await listRow()).toHaveTextContent('Turned down');
    });
    expect(within(await listRow()).getByRole('button', { name: 'Use this list' })).toBeEnabled();

    const saves = server.saved.length;
    go('structure');
    // Advanced's tree has the same name, so the step bar says Simple's is drawn.
    await screen.findByRole('list', { name: 'Steps' }, { timeout: 5000 });
    expect(await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 })).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(server.saved).toHaveLength(saves);
  }, 20_000);
});

describe('what “Where they go” chose, on Structure', () => {
  it('shows the list column that feeds the item sets among the archive’s own sets, and Remove takes it out', async () => {
    const column = { index: 1, header: 'Collection' };
    const slots = { item: { sets: { sources: [{ from: 'column', sheet: 'register', column }], combine: 'first' } }, media: {}, itemSet: {} };
    const server = simpleServer(scenario('meera', 9), { slots });
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    fireEvent.click(within(tree).getAllByRole('option', { name: 'All of archive' })[0] ?? tree);
    fireEvent.click(await screen.findByRole('button', { name: 'Show and change them' }, { timeout: 5000 }));
    const sets = await screen.findByRole('region', { name: 'Your own sets' }, { timeout: 5000 });
    const row = within(sets).getByText('One set per value of List register, column “Collection”').closest('li');
    fireEvent.click(within(row instanceof HTMLElement ? row : sets).getByRole('button', { name: 'Remove' }));
    await waitFor(() => {
      expect(asRecord(asRecord(last(server.saved)['slots'], [])['item'], [])['sets']).toBeUndefined();
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['structure'], [])['freeSets']).toEqual([]);
    expect(await within(sets).findByText('You have no set of your own.')).toBeInTheDocument();
  }, 20_000);

  it('maps a value to a new item type, and another to no item type, on the folder’s node', async () => {
    const server = await opened();
    go('structure');
    const from = await screen.findByRole('combobox', { name: 'The item type comes from' }, { timeout: 5000 });
    const type = await within(from).findByRole('option', { name: 'Column “Type”' }, { timeout: 5000 });
    fireEvent.change(from, { target: { value: type.getAttribute('value') } });
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
    fireEvent.click(within(await listRow()).getByRole('button', { name: 'Stop using it' }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['notLists']).toHaveLength(1);
    }, { timeout: 3000 });
    go('check');
    fireEvent.click(await screen.findByRole('button', { name: /^Use the \d+ suggested answers?$/ }, { timeout: 5000 }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['answered']).not.toEqual([]);
    }, { timeout: 3000 });
    expect(last(server.saved)['example']).toBeNull();

    go('advanced/files');
    expect(await screen.findByText('None. Files are one item only when their names are equal except for the extension.', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.queryByText(NOTICE)).toBeNull();

    // A reload, on the folder's Lists section.
    cleanup();
    window.location.hash = '#/import/1/advanced/lists';
    render(<Routed />);
    expect(await listRow()).toHaveTextContent('Turned down');
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

  it('the status line counts the questions Check counts as open, before and after an answer', async () => {
    openCheck();
    const counts = (await screen.findByText('Items to add', { selector: 'span' }, { timeout: 5000 })).parentElement;
    expect(counts).toHaveTextContent('Questions open 7');
    expect(await screen.findByRole('link', { name: '7 questions open' })).toHaveAttribute('href', '#/import/1/simple/check');
    await question('Already in Omeka');
    const known = await screen.findByRole('region', { name: '1 item is already in Omeka.' });
    fireEvent.click(within(within(known).getByRole('group', { name: 'Answer for all 1' })).getByRole('button', { name: 'Add them again' }));
    await waitFor(() => {
      expect(counts).toHaveTextContent('Questions open 6');
    }, { timeout: 3000 });
    expect(await screen.findByRole('link', { name: '6 questions open' }, { timeout: 3000 })).toBeInTheDocument();
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
