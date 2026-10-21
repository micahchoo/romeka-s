/**
 * The list picker over a folder shaped like BHC006 (`import-bhc006-lists.ts`):
 * four catalogues at the top and an old CSV and ODS copy of one in
 * `Artifacts/`. Simple shows every list beside the inspector of the selected
 * one; Advanced shows the same lists as rows. The person chooses which lists
 * to use, turns one down and still sees the rest, is asked once when a list
 * pairs the same items as one in use, and pairs names loosely with one
 * toggle. Each answer is read back from the plan the screen saves.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { AF, bhc006, CSV, D, IN_USE, LISTS, ODS, PN, RAO } from './import-bhc006-lists';
import type { ScenarioList } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
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

const go = (mode: string, screenName: string): void => {
  window.location.hash = `#/import/1/${mode}/${screenName}`;
  window.dispatchEvent(new HashChangeEvent('hashchange'));
};

const last = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};

const sheetsOf = (plan: Record<string, unknown>): readonly Record<string, unknown>[] =>
  (Array.isArray(plan['sheets']) ? plan['sheets'] : []).map((sheet) => asRecord(sheet, []));

const filesOf = (plan: Record<string, unknown>): readonly unknown[] => sheetsOf(plan).map((sheet) => sheet['file']);

const start = (mode: string, screenName: string, lists?: readonly ScenarioList[], plan: Record<string, unknown> = {}): ReturnType<typeof simpleServer> => {
  const server = simpleServer(bhc006(lists), plan);
  installFetch(server.handler);
  window.location.hash = `#/import/1/${mode}/${screenName}`;
  render(<Routed />);
  return server;
};

/** Opens "What we found", which adds the first list, and waits for it to be saved. */
const opened = async (): Promise<ReturnType<typeof simpleServer>> => {
  const server = start('simple', 'found');
  await waitFor(() => {
    expect(filesOf(last(server.saved))).toHaveLength(1);
  }, { timeout: 5000 });
  return server;
};

const option = async (file: string): Promise<HTMLElement> => screen.findByRole('option', { name: file }, { timeout: 5000 });

/** Selects the list on the left and returns its inspector. */
const inspect = async (file: string): Promise<HTMLElement> => {
  fireEvent.click(await option(file));
  return screen.findByRole('region', { name: file }, { timeout: 5000 });
};

const useBox = (inspector: HTMLElement): HTMLElement => within(inspector).getByRole('checkbox', { name: 'use this list' });

afterEach(() => {
  window.location.hash = '';
});

describe('Your list in Simple: the lists and the inspector', () => {
  it('adds the list that pairs the most rows first, not the first by path', async () => {
    const server = await opened();
    expect(filesOf(last(server.saved))).toEqual([AF]);
  }, 20_000);

  it('shows the six lists, the two in use marked, and the inspector of the one selected', async () => {
    start('simple', 'list', LISTS, IN_USE);
    const listbox = await screen.findByRole('listbox', { name: 'The lists in your folder' }, { timeout: 5000 });
    const options = await within(listbox).findAllByRole('option');
    expect(options.map((one) => one.textContent.includes('In use'))).toEqual([true, false, false, true, false, false]);
    expect(await option(AF)).toHaveAttribute('aria-selected', 'true');
    expect(await option(PN)).toHaveTextContent('86 / 215');

    const pn = await inspect(PN);
    expect(await option(PN)).toHaveAttribute('aria-selected', 'true');
    expect(useBox(pn)).toBeChecked();
    expect(pn).toHaveTextContent('86 / 215');
    expect(within(pn).getByRole('button', { name: '+103 ignoring case, zeros and _ - .' })).toHaveAttribute('aria-pressed', 'false');
    expect(within(pn).getByRole('combobox', { name: 'Pair each row with an item by the column' })).toHaveValue('0');
    expect(within(pn).getByRole('button', { name: 'Item suggested' })).toHaveAttribute('aria-pressed', 'true');
  }, 20_000);

  it('counts what the lists in use pair, in one line', async () => {
    start('simple', 'list', LISTS, IN_USE);
    const facts = await screen.findByText(/^Lists in use/, { selector: 'span' }, { timeout: 5000 });
    const line = facts.parentElement ?? facts;
    expect(line).toHaveTextContent('Lists in use 2');
    expect(line).toHaveTextContent('Rows paired 731 / 905');
    expect(line).toHaveTextContent('Rows naming no file 92');
    expect(line).toHaveTextContent('Rows naming a refused file 82');
    expect(within(line).getByRole('link', { name: 'Check' })).toHaveAttribute('href', '#/import/1/simple/check');
    expect(line).toHaveTextContent('Items without a row 210');
    expect(within(line).getByRole('link', { name: 'Pair by hand' })).toHaveAttribute('href', '#/import/1/simple/pair');
  }, 20_000);

  it('pairs names loosely with one toggle: every fold on, then every fold off', async () => {
    const server = start('simple', 'list', LISTS, IN_USE);
    const pn = await inspect(PN);
    const loose = within(pn).getByRole('button', { name: '+103 ignoring case, zeros and _ - .' });
    fireEvent.click(loose);
    const normalizeOf = (plan: Record<string, unknown>, file: string): unknown =>
      asRecord(sheetsOf(plan).find((sheet) => sheet['file'] === file)?.['join'], [])['normalize'];
    await waitFor(() => {
      expect(normalizeOf(last(server.saved), PN)).toEqual({ caseFold: true, leadingZeros: true, separators: true });
    }, { timeout: 3000 });
    expect(normalizeOf(last(server.saved), AF)).toEqual({ caseFold: false, leadingZeros: false, separators: false });
    expect(loose).toHaveAttribute('aria-pressed', 'true');
    expect(pn).toHaveTextContent('189 / 215');
    fireEvent.click(loose);
    await waitFor(() => {
      expect(normalizeOf(last(server.saved), PN)).toEqual({ caseFold: false, leadingZeros: false, separators: false });
    }, { timeout: 3000 });
  }, 20_000);

  it('turning “use this list” off takes the list out, and keeps it out', async () => {
    const server = start('simple', 'list', LISTS, IN_USE);
    const pn = await inspect(PN);
    fireEvent.click(useBox(pn));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF]);
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: PN }]);
    expect(await option(PN)).toHaveTextContent('Turned down');
    expect(useBox(pn)).not.toBeChecked();
  }, 20_000);

  it('moves the selection with the arrow keys, Home and End, and Enter goes to the inspector', async () => {
    start('simple', 'list', LISTS, IN_USE);
    const af = await option(AF);
    af.focus();
    fireEvent.keyDown(af, { key: 'ArrowDown' });
    const csv = await option(CSV);
    expect(csv).toHaveAttribute('aria-selected', 'true');
    expect(csv).toHaveFocus();
    expect(af).toHaveAttribute('aria-selected', 'false');
    expect(await screen.findByRole('region', { name: CSV })).toBeInTheDocument();
    fireEvent.keyDown(csv, { key: 'ArrowDown' });
    expect(await option(ODS)).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(await option(ODS), { key: 'End' });
    expect(await option(D)).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(await option(D), { key: 'ArrowDown' });
    expect(await option(D)).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(await option(D), { key: 'ArrowUp' });
    expect(await option(RAO)).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(await option(RAO), { key: 'Home' });
    expect(af).toHaveAttribute('aria-selected', 'true');
    fireEvent.keyDown(af, { key: 'Enter' });
    expect(document.activeElement).toContainElement(screen.getByRole('region', { name: AF }));
  }, 20_000);

  it('warns that the old copy pairs the same items as the list in use', async () => {
    start('simple', 'list', LISTS, IN_USE);
    const csv = await inspect(CSV);
    expect(csv).toHaveTextContent(`same items as ${AF} · 292 items`);
    expect(useBox(csv)).not.toBeChecked();
    const pn = await inspect(PN);
    expect(pn).not.toHaveTextContent('same items as');
  }, 20_000);

  it('turns down one list and keeps every other list offered', async () => {
    const server = await opened();
    go('simple', 'list');
    const d = await inspect(D);
    fireEvent.click(within(d).getByRole('button', { name: 'Not about these items' }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: D }]);
    }, { timeout: 3000 });
    expect(await option(D)).toHaveTextContent('Turned down');
    for (const file of [PN, RAO, CSV]) {
      expect(useBox(await inspect(file))).toBeEnabled();
    }
    expect(filesOf(last(server.saved))).toEqual([AF]);
  }, 20_000);

  it('adds a second list beside the first, and shows each list’s own counts', async () => {
    const server = await opened();
    go('simple', 'list');
    fireEvent.click(useBox(await inspect(PN)));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF, PN]);
    }, { timeout: 5000 });
    expect(await option(PN)).toHaveTextContent('In use');
    expect(await inspect(AF)).toHaveTextContent('680 / 690');
    expect(await inspect(PN)).toHaveTextContent('86 / 215');
  }, 20_000);

  it('asks before it adds a list that pairs the same items as one in use; in its place leaves one list', async () => {
    const server = await opened();
    go('simple', 'list');
    const csv = await inspect(CSV);
    fireEvent.click(useBox(csv));
    expect(within(csv).getByText(`It pairs 292 of the same items as ${AF}.`)).toBeInTheDocument();
    const saves = server.saved.length;
    fireEvent.click(within(csv).getByRole('button', { name: `Use this one in place of ${AF}` }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([CSV]);
    }, { timeout: 5000 });
    expect(server.saved.length).toBeGreaterThan(saves);
    expect(await option(AF)).not.toHaveTextContent('In use');
  }, 20_000);

  it('keeps both lists when the person says both', async () => {
    const server = await opened();
    go('simple', 'list');
    const csv = await inspect(CSV);
    fireEvent.click(useBox(csv));
    fireEvent.click(within(csv).getByRole('button', { name: 'Use both' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF, CSV]);
    }, { timeout: 5000 });
  }, 20_000);

  it('shows the error of a list that did not read, with nothing to choose', async () => {
    start('simple', 'list', [...LISTS, { file: 'broken.xlsx', rows: 0, paired: 0, normalized: 0, error: 'The workbook has no sheet.' }]);
    const broken = await inspect('broken.xlsx');
    expect(await option('broken.xlsx')).toHaveTextContent('Not read');
    expect(broken).toHaveTextContent('This list could not be read: The workbook has no sheet.');
    expect(within(broken).queryAllByRole('checkbox')).toHaveLength(0);
    expect(within(broken).queryAllByRole('button')).toHaveLength(0);
  }, 20_000);
});

describe('the list picker in Advanced', () => {
  const row = async (file: string): Promise<HTMLElement> => screen.findByRole('listitem', { name: file }, { timeout: 5000 });

  it('shows the same lists, and adding one writes the plan Simple writes', async () => {
    const simple = start('simple', 'list');
    fireEvent.click(useBox(await inspect(PN)));
    await waitFor(() => {
      expect(filesOf(last(simple.saved))).toEqual([PN]);
    }, { timeout: 5000 });
    window.location.hash = '';
    cleanup();

    const advanced = start('advanced', 'lists');
    const pn = await row(PN);
    expect(pn).toHaveTextContent('86 of 215 rows name a file, by “File”. +103 ignoring case, zeros and _ - .');
    expect(await row(CSV)).toHaveTextContent(`same items as ${AF} · 292 items`);
    fireEvent.click(within(pn).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(filesOf(last(advanced.saved))).toEqual([PN]);
    }, { timeout: 5000 });
    expect(last(advanced.saved)).toEqual(last(simple.saved));
  }, 20_000);
});
