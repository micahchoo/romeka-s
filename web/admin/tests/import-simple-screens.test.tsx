/**
 * Single answers on the Simple screens, read back from the plan they save:
 * turning down the folder's list keeps it out, an item type value maps to a
 * new item type or to none, and Check suggests each kind's default policy.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
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
    expect(await screen.findByText(/^We looked at your/, {}, { timeout: 5000 })).toBeInTheDocument();
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
    expect(await screen.findByText(/^The files are not put into items yet/, {}, { timeout: 5000 })).toBeInTheDocument();
    expect(server.saved).toHaveLength(0);
    fireEvent.click(screen.getByRole('button', { name: 'Make one item per name' }));
    await waitFor(() => {
      expect(String(asRecord(last(server.saved)['grouping'], [])['pattern'])).toContain('(?<identifier>');
    }, { timeout: 3000 });
  });
});

describe('Your list', () => {
  it('uses the folder’s list as the suggested answer, and “No” takes it out and keeps it out', async () => {
    const server = await opened();
    expect(String(asRecord(last(server.saved)['grouping'], [])['pattern'])).toContain('(?<identifier>');
    go('list');
    fireEvent.click(await screen.findByRole('button', { name: 'No, it is not about these items' }, { timeout: 5000 }));
    await waitFor(() => {
      expect(sheetsOf(last(server.saved))).toHaveLength(0);
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: 'register.csv' }]);
    expect(last(server.saved)['example']).toBeNull();
    expect(await screen.findByRole('button', { name: 'No, it is not about these items', pressed: true })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Yes, use it', pressed: false })).toBeEnabled();

    const saves = server.saved.length;
    go('found');
    expect(await screen.findByRole('region', { name: 'Your items in groups' }, { timeout: 5000 })).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(server.saved).toHaveLength(saves);
  }, 20_000);
});

describe('Where they go', () => {
  it('shows the list column that feeds the item sets as the checked choice, and “No item set” takes it out', async () => {
    const column = { index: 1, header: 'Collection' };
    const slots = { item: { sets: { sources: [{ from: 'column', sheet: 'register', column }], combine: 'first' } }, media: {}, itemSet: {} };
    const server = simpleServer(scenario('meera', 9), { slots });
    installFetch(server.handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    const fromList = await screen.findByRole('radio', { name: 'From your list, the column “Collection”' }, { timeout: 5000 });
    expect(fromList).toBeChecked();
    expect(screen.getByRole('radio', { name: 'No item set' })).not.toBeChecked();
    fireEvent.click(screen.getByRole('radio', { name: 'No item set' }));
    await waitFor(() => {
      expect(asRecord(asRecord(last(server.saved)['slots'], [])['item'], [])['sets']).toBeUndefined();
    }, { timeout: 3000 });
    expect(screen.queryByRole('radio', { name: 'From your list, the column “Collection”' })).toBeNull();
    expect(screen.getByRole('radio', { name: 'No item set' })).toBeChecked();
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
    fireEvent.click(await screen.findByRole('button', { name: 'No, it is not about these items' }, { timeout: 5000 }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['notLists']).toHaveLength(1);
    }, { timeout: 3000 });
    go('check');
    fireEvent.click(await screen.findByRole('button', { name: /^Use the \d+ suggested answers|^Use the suggested answer/ }, { timeout: 5000 }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['answered']).not.toEqual([]);
    }, { timeout: 3000 });
    expect(last(server.saved)['example']).toBeNull();

    window.location.hash = '#/import/1/advanced/files';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    expect(await screen.findByText('None. Files are one item only when their names are equal except for the extension.', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(screen.queryByText(NOTICE)).toBeNull();

    go('list');
    expect(await screen.findByRole('button', { name: 'No, it is not about these items', pressed: true }, { timeout: 5000 })).toBeInTheDocument();
  }, 30_000);
});

describe('Check', () => {
  it('suggests leaving out files that are not media, the kind’s default, when Advanced chose to keep them', async () => {
    installFetch(simpleServer(scenario('bhc', 9), { flags: { 'not-media': 'keep' } }).handler);
    render(<ImportPage param="1" rest={['1', 'simple', 'check']} />);
    const card = await screen.findByRole('article', { name: '1 time: Not a picture, a sound, a film or a PDF.' }, { timeout: 5000 });
    const leave = within(card).getByRole('radio', { name: /^Leave them out/ });
    const keep = within(card).getByRole('radio', { name: /^Keep them as they are/ });
    expect(keep).toBeChecked();
    expect(leave.closest('label')).toHaveTextContent('suggested');
    expect(keep.closest('label')).not.toHaveTextContent('suggested');
  }, 20_000);
});
