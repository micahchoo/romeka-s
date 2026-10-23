/**
 * The list picker over a folder shaped like BHC006 (`import-bhc006-lists.ts`):
 * four catalogues at the top and an old CSV and ODS copy of one in
 * `Artifacts/`, on the folder's Lists section of Advanced's Structure. The
 * person chooses which lists to use, turns one down and still sees the
 * rest, is asked once when a list pairs the same items as one in use, and
 * pairs names loosely with the folds of the join. Each answer is read back
 * from the plan the screen saves. Simple's Structure tree takes the keys the
 * old "Your list" listbox took.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { AF, bhc006, CSV, D, IN_USE, LISTS, ODS, PN, RAO } from './import-bhc006-lists';
import type { ScenarioList } from './import-simple-scenarios';
import { PATH, simpleServer } from './import-simple-server';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

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

const row = async (file: string): Promise<HTMLElement> => screen.findByRole('listitem', { name: file }, { timeout: 5000 });

/** The list `file` as its folder reads it, under the picker: how it is read, joined and what a row is. */
const sheet = async (file: string): Promise<HTMLElement> => screen.findByRole('region', { name: file }, { timeout: 5000 });

/** Advanced's Lists section of the plan's one folder, with AF used from the picker, as the list that pairs the most. */
const withAf = async (): Promise<ReturnType<typeof simpleServer>> => {
  const server = start('advanced', 'lists');
  fireEvent.click(within(await row(AF)).getByRole('button', { name: 'Use this list' }));
  await waitFor(() => {
    expect(filesOf(last(server.saved))).toEqual([AF]);
  }, { timeout: 5000 });
  return server;
};

const normalizeOf = (plan: Record<string, unknown>, file: string): unknown =>
  asRecord(sheetsOf(plan).find((one) => one['file'] === file)?.['join'], [])['normalize'];

const FOLDS = ['Capital letters (A2 and a2)', 'Zeros at the start (007 and 7)', 'The characters _ - . and space (B 4 and B_4)'];

afterEach(() => {
  window.location.hash = '';
});

describe('a folder’s Lists section in Advanced: the lists and how each is read', () => {
  it('shows the six lists, the two in use marked, and how a list in use pairs its rows', async () => {
    start('advanced', 'lists', LISTS, IN_USE);
    const picker = await screen.findByRole('region', { name: 'The lists in your folder' }, { timeout: 5000 });
    const rows = await within(picker).findAllByRole('listitem');
    expect(rows.map((one) => one.getAttribute('aria-label'))).toEqual([AF, CSV, ODS, PN, RAO, D]);
    expect(rows.map((one) => one.textContent.includes('In use'))).toEqual([true, false, false, true, false, false]);
    expect(await row(PN)).toHaveTextContent('86 of 215 rows name a file, by “File”. +103 ignoring case, zeros and _ - .');

    const pn = await sheet(PN);
    expect(within(pn).getByRole('combobox', { name: 'Join column' })).toHaveValue('0');
    expect(within(pn).getByRole('combobox', { name: 'Each row describes' })).toHaveValue('item');
    for (const fold of FOLDS) {
      expect(within(pn).getByRole('checkbox', { name: fold })).not.toBeChecked();
    }
  }, 20_000);

  it('counts the rows that name no file and the items with no row, with the way to pair them by hand', async () => {
    start('advanced', 'lists', LISTS, IN_USE);
    expect(await screen.findByRole('region', { name: 'Lists' }, { timeout: 5000 })).toHaveTextContent(`${AF}, ${PN}`);
    const unpaired = await screen.findByText(/^92 rows have no file\./, {}, { timeout: 5000 });
    expect(unpaired).toHaveTextContent('92 rows have no file. 210 items have files but no row.');
    expect(within(unpaired).getByRole('link', { name: 'Pair them by hand' }).getAttribute('href')).toMatch(/^#\/import\/1\/advanced\/pair\//);
  }, 20_000);

  it('pairs names loosely: every fold on, then every fold off', async () => {
    const server = start('advanced', 'lists', LISTS, IN_USE);
    const pn = await sheet(PN);
    for (const fold of FOLDS) {
      fireEvent.click(within(pn).getByRole('checkbox', { name: fold }));
    }
    await waitFor(() => {
      expect(normalizeOf(last(server.saved), PN)).toEqual({ caseFold: true, leadingZeros: true, separators: true });
    }, { timeout: 3000 });
    expect(normalizeOf(last(server.saved), AF)).toEqual({ caseFold: false, leadingZeros: false, separators: false });
    for (const fold of FOLDS) {
      expect(within(pn).getByRole('checkbox', { name: fold })).toBeChecked();
    }
    for (const fold of FOLDS) {
      fireEvent.click(within(pn).getByRole('checkbox', { name: fold }));
    }
    await waitFor(() => {
      expect(normalizeOf(last(server.saved), PN)).toEqual({ caseFold: false, leadingZeros: false, separators: false });
    }, { timeout: 3000 });
  }, 20_000);

  it('Stop using it takes the list out, and keeps it out', async () => {
    const server = start('advanced', 'lists', LISTS, IN_USE);
    fireEvent.click(within(await row(PN)).getByRole('button', { name: 'Stop using it' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF]);
    }, { timeout: 3000 });
    expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: PN }]);
    await waitFor(async () => {
      expect(await row(PN)).toHaveTextContent('Turned down');
    }, { timeout: 3000 });
    expect(within(await row(PN)).getByRole('button', { name: 'Use this list' })).toBeEnabled();
  }, 20_000);

  it('warns that the old copy pairs the same items as the list in use', async () => {
    start('advanced', 'lists', LISTS, IN_USE);
    const csv = await row(CSV);
    expect(csv).toHaveTextContent(`same items as ${AF} · 292 items`);
    expect(csv).toHaveTextContent('Not used');
    expect(await row(PN)).not.toHaveTextContent('same items as');
  }, 20_000);

  it('turns down one list and keeps every other list offered', async () => {
    const server = await withAf();
    fireEvent.click(within(await row(D)).getByRole('button', { name: 'Not about these items' }));
    await waitFor(() => {
      expect(asRecord(last(server.saved)['simple'], [])['notLists']).toEqual([{ source: 's1', file: D }]);
    }, { timeout: 3000 });
    await waitFor(async () => {
      expect(await row(D)).toHaveTextContent('Turned down');
    }, { timeout: 3000 });
    for (const file of [PN, RAO, CSV]) {
      expect(within(await row(file)).getByRole('button', { name: 'Use this list' })).toBeEnabled();
    }
    expect(filesOf(last(server.saved))).toEqual([AF]);
  }, 20_000);

  it('adds a second list beside the first, and shows each list’s own counts', async () => {
    const server = await withAf();
    fireEvent.click(within(await row(PN)).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF, PN]);
    }, { timeout: 5000 });
    await waitFor(async () => {
      expect(await row(PN)).toHaveTextContent('In use');
    }, { timeout: 3000 });
    expect(await row(AF)).toHaveTextContent('680 of 690');
    expect(await row(PN)).toHaveTextContent('86 of 215');
  }, 20_000);

  it('asks before it adds a list that pairs the same items as one in use; in its place leaves one list', async () => {
    const server = await withAf();
    const csv = await row(CSV);
    fireEvent.click(within(csv).getByRole('button', { name: 'Use this list' }));
    expect(within(csv).getByText(`It pairs 292 of the same items as ${AF}.`)).toBeInTheDocument();
    const saves = server.saved.length;
    fireEvent.click(within(csv).getByRole('button', { name: `Use this one in place of ${AF}` }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([CSV]);
    }, { timeout: 5000 });
    expect(server.saved.length).toBeGreaterThan(saves);
    await waitFor(async () => {
      expect(await row(AF)).not.toHaveTextContent('In use');
    }, { timeout: 3000 });
  }, 20_000);

  it('keeps both lists when the person says both', async () => {
    const server = await withAf();
    const csv = await row(CSV);
    fireEvent.click(within(csv).getByRole('button', { name: 'Use this list' }));
    fireEvent.click(within(csv).getByRole('button', { name: 'Use both' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([AF, CSV]);
    }, { timeout: 5000 });
  }, 20_000);

  it('Use this list reads the list into the folder, from a plan with none', async () => {
    const server = start('advanced', 'lists');
    const pn = await row(PN);
    expect(pn).toHaveTextContent('86 of 215 rows name a file, by “File”. +103 ignoring case, zeros and _ - .');
    fireEvent.click(within(pn).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(filesOf(last(server.saved))).toEqual([PN]);
    }, { timeout: 5000 });
    expect(asRecord(sheetsOf(last(server.saved))[0]?.['join'], [])['column']).toEqual({ index: 0, header: 'File' });
  }, 20_000);

  it('shows the error of a list that did not read, with nothing to choose', async () => {
    start('advanced', 'lists', [...LISTS, { file: 'broken.xlsx', rows: 0, paired: 0, normalized: 0, error: 'The workbook has no sheet.' }]);
    const broken = await row('broken.xlsx');
    expect(broken).toHaveTextContent('Not read');
    expect(broken).toHaveTextContent('This list could not be read: The workbook has no sheet.');
    expect(within(broken).queryAllByRole('checkbox')).toHaveLength(0);
    expect(within(broken).queryAllByRole('button')).toHaveLength(0);
  }, 20_000);
});

describe('the list picker of one folder, on a plan of several folders', () => {
  const PROPOSAL = asRecord(loadFixture('import/proposal-bhc006.json'), []);
  const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
  const levelsOf = (plan: Record<string, unknown>): readonly Record<string, unknown>[] => {
    const levels: unknown = asRecord(plan['structure'], [])['levels'];
    return (Array.isArray(levels) ? levels : []).filter(isRecord);
  };
  const listsOf = (level: Record<string, unknown> | undefined): readonly Record<string, unknown>[] =>
    (Array.isArray(level?.['lists']) ? level['lists'] : []).filter(isRecord);
  const AF_FILE = 'BHC006_GMT_AF.xlsx';
  const D_FILE = 'BHC006_GMT_D.xlsx';

  /** BHC006 after the first Structure click: Artifacts reads AF, Documents (D) reads D. The scan's candidates name the folder that reads each. */
  const several = (): { readonly saved: Record<string, unknown>[] } => {
    const detail = importFixture('import-draft.json');
    let plan: Record<string, unknown> = { ...asRecord(detail['plan'], []), structure: PROPOSAL['structure'] };
    let revision = 0;
    const saved: Record<string, unknown>[] = [];
    const first = (name: string, section: string): Record<string, unknown> => {
      const all = importFixture(name)[section];
      return asRecord(Array.isArray(all) ? all[0] : null, []);
    };
    const template = first('preview-sheets.json', 'sheets');
    const candidate = first('preview-list-candidates.json', 'listCandidates');
    installFetch(
      route({
        [PATH]: (_url, init) => {
          if (init?.method === 'PUT') {
            plan = asRecord(asRecord(JSON.parse(typeof init.body === 'string' ? init.body : '{}'), [])['plan'], []);
            saved.push(plan);
            revision += 1;
          }
          return jsonResponse({ ...detail, plan, revision });
        },
        [`${PATH}/preview`]: (_url, init) => {
          const body = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
          const include = Array.isArray(body['include']) ? body['include'].map(String) : [];
          const sent = isRecord(body['plan']) ? body['plan'] : plan;
          const held = levelsOf(sent).flatMap((level) => listsOf(level).map((list) => ({ list, series: level['id'] })));
          const answer: Record<string, unknown> = { sources: importFixture('preview-check.json')['sources'], pattern: { groups: ['identifier'] } };
          for (const section of include) {
            if (section === 'sheets') {
              answer[section] = held.map(({ list }) => ({ ...template, id: list['id'], source: list['source'], file: list['file'] }));
            } else if (section === 'columnMatches') {
              answer[section] = [];
            } else if (section === 'listCandidates') {
              answer[section] = [AF_FILE, D_FILE].map((file) => {
                const holder = held.find(({ list }) => list['file'] === file);
                return { ...candidate, file, inPlan: holder?.list['id'] ?? null, ...(holder === undefined ? {} : { series: holder.series }) };
              });
            } else {
              answer[section] = section === 'proposal' ? PROPOSAL : importFixture(`preview-${section}.json`)[section];
            }
          }
          return jsonResponse(answer);
        },
        '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
        '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
      }),
    );
    return { saved };
  };

  it('the Structure tree moves the selection with the arrow keys, Home and End, and Enter goes to the inspector', async () => {
    several();
    window.location.hash = '#/import/1/simple/structure';
    render(<Routed />);
    const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const options = (): HTMLElement[] => within(tree).getAllByRole('option');
    const selected = (): string | null => options().find((one) => one.getAttribute('aria-selected') === 'true')?.querySelector('.guided-row-name')?.textContent ?? null;
    const names = options().map((one) => one.querySelector('.guided-row-name')?.textContent ?? null);
    const artifacts = options()[names.indexOf('Artifacts')];
    if (artifacts === undefined) {
      throw new Error('no Artifacts node');
    }
    fireEvent.click(artifacts);
    artifacts.focus();
    fireEvent.keyDown(artifacts, { key: 'ArrowDown' });
    const next = names[names.indexOf('Artifacts') + 1] ?? null;
    expect(selected()).toBe(next);
    expect(options().find((one) => one.getAttribute('aria-selected') === 'true')).toHaveFocus();
    fireEvent.keyDown(document.activeElement ?? tree, { key: 'End' });
    expect(selected()).toBe(names.at(-1));
    fireEvent.keyDown(document.activeElement ?? tree, { key: 'ArrowDown' });
    expect(selected()).toBe(names.at(-1));
    fireEvent.keyDown(document.activeElement ?? tree, { key: 'ArrowUp' });
    expect(selected()).toBe(names.at(-2));
    fireEvent.keyDown(document.activeElement ?? tree, { key: 'Home' });
    expect(selected()).toBe(names[0]);
    fireEvent.keyDown(document.activeElement ?? tree, { key: 'Enter' });
    const inspector = tree.parentElement?.querySelector('.guided-inspector');
    expect(inspector).toContainElement(document.activeElement instanceof HTMLElement ? document.activeElement : null);
  }, 20_000);

  it('a list another folder uses shows as used there, with Move here and no Stop using', async () => {
    const { saved } = several();
    window.location.hash = '#/import/1/advanced/lists/{"series":"Artifacts"}';
    render(<Routed />);
    const d = await screen.findByRole('listitem', { name: D_FILE }, { timeout: 5000 });
    expect(d).toHaveTextContent('Used by Documents (D)');
    expect(within(d).queryByRole('button', { name: 'Stop using it' })).toBeNull();
    const af = screen.getByRole('listitem', { name: AF_FILE });
    expect(af).toHaveTextContent('In use');
    expect(within(af).getByRole('button', { name: 'Stop using it' })).toBeInTheDocument();

    fireEvent.click(within(d).getByRole('button', { name: 'Move here' }));
    await waitFor(() => {
      expect(saved).toHaveLength(1);
    }, { timeout: 5000 });
    const levels = levelsOf(saved[0] ?? {});
    expect(listsOf(levels.find((level) => level['id'] === 'Artifacts')).map((list) => list['file'])).toEqual([AF_FILE, D_FILE]);
    expect(listsOf(levels.find((level) => level['id'] === 'Documents (D)'))).toEqual([]);
    expect(saved[0]?.['edits']).toEqual([
      { edit: 'own', scope: { source: 's1', folder: 'Documents (D)' } },
      { edit: 'own', scope: { source: 's1', folder: 'Artifacts' } },
    ]);
  }, 20_000);
});
