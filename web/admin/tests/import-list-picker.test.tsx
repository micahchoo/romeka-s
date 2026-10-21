/**
 * The list picker both modes show, over a folder shaped like BHC006: four
 * catalogues at the top and a stale copy of one in `Artifacts/`. The person
 * chooses which lists to use, turns one down and still sees the rest, and is
 * asked once when a list pairs the same items as one in use. Each answer is
 * read back from the plan the screen saves.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { scenario } from './import-simple-scenarios';
import type { Scenario, ScenarioList } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const AF = 'BHC006_GMT_AF.xlsx';
const CSV = 'Artifacts/BHC006_GMT_AF.csv';
const PN = 'BHC006_GMT_PN.xlsx';
const D = 'BHC006_GMT_D.xlsx';
const RAO = 'BHC006_CC_SECOND_AF.xlsx';

const LISTS: readonly ScenarioList[] = [
  { file: CSV, rows: 607, paired: 602, normalized: 602, overlaps: [{ file: AF, sharedTargets: 600 }] },
  { file: AF, rows: 690, paired: 680, normalized: 680, overlaps: [{ file: CSV, sharedTargets: 600 }] },
  { file: PN, rows: 215, paired: 86, normalized: 184 },
  { file: D, rows: 15, paired: 0, normalized: 0 },
  { file: RAO, rows: 29, paired: 0, normalized: 0 },
];

const bhc006 = (lists: readonly ScenarioList[] = LISTS): Scenario => ({ ...scenario('bhc', 9), lists });

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

const filesOf = (plan: Record<string, unknown>): readonly unknown[] =>
  (Array.isArray(plan['sheets']) ? plan['sheets'] : []).map((sheet) => asRecord(sheet, [])['file']);

const row = async (file: string): Promise<HTMLElement> => screen.findByRole('listitem', { name: file }, { timeout: 5000 });

const start = (mode: string, screenName: string, lists?: readonly ScenarioList[]): ReturnType<typeof simpleServer> => {
  const server = simpleServer(bhc006(lists));
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

afterEach(() => {
  window.location.hash = '';
});

describe('the list picker in Simple', () => {
  it('adds the list that pairs the most rows first, not the first by path', async () => {
    const server = await opened();
    expect(filesOf(last(server.saved))).toEqual([AF]);
  }, 20_000);

  it('shows each list with its counts, and the overlap of the old copy with the list in use', async () => {
    await opened();
    go('simple', 'list');
    const af = await row(AF);
    expect(af).toHaveTextContent('690 rows');
    expect(af).toHaveTextContent('680 of 690 rows name one of your files.');
    expect(af).toHaveTextContent('In use');
    const pn = await row(PN);
    expect(pn).toHaveTextContent('86 of 215 rows name one of your files.');
    expect(pn).toHaveTextContent('98 more when capital letters, zeros at the start and _ - . are ignored.');
    expect(pn).toHaveTextContent('Not used');
    const csv = await row(CSV);
    expect(csv).toHaveTextContent('BHC006_GMT_AF.csv');
    expect(csv).toHaveTextContent('in the folder Artifacts');
    expect(csv).toHaveTextContent(`Pairs the same items as ${AF}: 600`);
    expect(within(pn).queryByText(/^Pairs the same items/)).toBeNull();
  }, 20_000);

  it('turns down one list and keeps every other list offered', async () => {
    const server = await opened();
    go('simple', 'list');
    fireEvent.click(within(await row(D)).getByRole('button', { name: 'Not about these items' }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: D }]);
    }, { timeout: 3000 });
    expect(await row(D)).toHaveTextContent('Turned down');
    for (const file of [PN, RAO, CSV]) {
      expect(within(await row(file)).getByRole('button', { name: 'Use this list' })).toBeEnabled();
    }
    expect(filesOf(last(server.saved))).toEqual([AF]);
  }, 20_000);

  it('adds a second list beside the first, and shows each list’s own counts', async () => {
    const server = await opened();
    go('simple', 'list');
    fireEvent.click(within(await row(PN)).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF, PN]);
    }, { timeout: 5000 });
    const afBlock = await screen.findByRole('region', { name: AF }, { timeout: 5000 });
    const pnBlock = await screen.findByRole('region', { name: PN }, { timeout: 5000 });
    expect(afBlock).toHaveTextContent('680 of 690 cells are the name of one of your files.');
    expect(pnBlock).toHaveTextContent('86 of 215 cells are the name of one of your files.');
  }, 20_000);

  it('asks before it adds a list that pairs the same items as one in use; in its place leaves one list', async () => {
    const server = await opened();
    go('simple', 'list');
    const csv = await row(CSV);
    fireEvent.click(within(csv).getByRole('button', { name: 'Use this list' }));
    expect(within(csv).getByText(`This list pairs 600 of the same items as ${AF}. Use both, or use this one in place of ${AF}?`)).toBeInTheDocument();
    const saves = server.saved.length;
    fireEvent.click(within(csv).getByRole('button', { name: `Use this one in place of ${AF}` }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([CSV]);
    }, { timeout: 5000 });
    expect(server.saved.length).toBeGreaterThan(saves);
    expect(await row(AF)).toHaveTextContent('Not used');
  }, 20_000);

  it('keeps both lists when the person says both', async () => {
    const server = await opened();
    go('simple', 'list');
    const csv = await row(CSV);
    fireEvent.click(within(csv).getByRole('button', { name: 'Use this list' }));
    fireEvent.click(within(csv).getByRole('button', { name: 'Use both' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF, CSV]);
    }, { timeout: 5000 });
  }, 20_000);

  it('shows the error of a list that did not read, with nothing to choose', async () => {
    start('simple', 'list', [...LISTS, { file: 'broken.xlsx', rows: 0, paired: 0, normalized: 0, error: 'The workbook has no sheet.' }]);
    const broken = await row('broken.xlsx');
    expect(broken).toHaveTextContent('This list could not be read: The workbook has no sheet.');
    expect(within(broken).queryAllByRole('button')).toHaveLength(0);
  }, 20_000);
});

describe('the list picker in Advanced', () => {
  it('shows the same lists, and adding one writes the plan Simple writes', async () => {
    const simple = start('simple', 'list');
    fireEvent.click(within(await row(PN)).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(filesOf(last(simple.saved))).toEqual([PN]);
    }, { timeout: 5000 });
    window.location.hash = '';
    cleanup();

    const advanced = start('advanced', 'lists');
    const pn = await row(PN);
    expect(pn).toHaveTextContent('86 of 215 rows name one of your files.');
    expect(await row(CSV)).toHaveTextContent(`Pairs the same items as ${AF}: 600`);
    fireEvent.click(within(pn).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(filesOf(last(advanced.saved))).toEqual([PN]);
    }, { timeout: 5000 });
    expect(last(advanced.saved)).toEqual(last(simple.saved));
  }, 20_000);
});
