/**
 * The whole Simple pass, from the start page to "Add", over the prototype's
 * two folders at 9 and at 40,000 items (`import-simple-scenarios.ts`). The
 * drive does what `proto4/drive4.mjs` does in its first pass: it marks one
 * example (two above 9 items), maps the two columns that matter, takes the
 * item type from the list or the top folder, accepts the suggested answers
 * on Check and starts the run.
 *
 * Simple's steps are Structure, Check and Add, and the drive does each of
 * those on the folder's node of Structure. The test server stands in for no
 * proposer (`import-plan-proposal.ts`), so the folder is written by hand
 * and the List question that would suggest its list is absent. The draft
 * therefore starts with the list read in as "Use this list" reads it
 * (`listEdit`): the list the proposer suggests, and the one "What we found"
 * added with no click before. Groups of items by their facts are on
 * Advanced's archive node (P5 design call 5), so the drive that leaves a
 * group out goes there and back by the mode switch. "They are right" went
 * with "What we found": nothing read it.
 *
 * It counts what `SCALE.md` counts. A decision is a question the screens put
 * to the person (each `data-decision`), counted once however often it
 * shows, plus "where are your files". A click is every click, search and
 * choice from the start page to the start. A Check question is a row of a
 * list, which names its decision whether or not it is selected. The clicks
 * must be within 3 of the prototype's, with the trip to Advanced's groups
 * added. The decisions may be at most 3 more than the prototype's: the
 * questions of the deleted steps (the example rule, each group, which list,
 * its pairing, what a row is, its header line, the set dial) are asked by
 * the proposer's questions on a suggested folder, which this server does not
 * stand in for, so the pass asks fewer. The ones it still asks are named.
 * No screen may draw more than about 1,000 elements.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, brandId } from '../src/api/parse';
import { importApi } from '../src/api/import-routes';
import { ImportPage } from '../src/pages/import/page';
import { listEdit } from '../src/pages/import/simple/list-read';
import { flatPlan } from './import-plan-members';
import { scenario } from './import-simple-scenarios';
import type { Scenario } from './import-simple-scenarios';
import { simpleServer } from './import-simple-server';
import { installFetch } from './stub';

const subscribe = (listener: () => void): (() => void) => {
  window.addEventListener('hashchange', listener);
  return () => {
    window.removeEventListener('hashchange', listener);
  };
};

/** The admin's router for `#/import…`, in small: the page follows the address. */
const Routed = (): ReactElement => {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  const rest = hash.replace(/^#\/import\/?/, '').split('/').filter((part) => part !== '');
  return <ImportPage param={rest[0] ?? null} rest={rest} />;
};

const ELEMENTS = 1200;

/**
 * The clicks a group left out costs beyond the prototype's: its groups are on
 * Advanced's archive node now (P5 design call 5), so the drive switches to
 * Advanced, picks the archive, switches back and picks the folder again.
 */
const GROUP_ROUND_TRIP = 4;

class Drive {
  clicks = 0;
  readonly seen = new Set<string>(['where']);

  constructor(readonly at: Scenario) {}

  /** One click, choice or search; a link to a screen moves the address as a browser would. */
  act(fire: () => void, target?: Element): void {
    this.clicks += 1;
    fire();
    const href = target?.getAttribute('href');
    if (href?.startsWith('#') === true) {
      window.location.hash = href;
      window.dispatchEvent(new HashChangeEvent('hashchange'));
    }
  }

  click(element: Element): void {
    this.act(() => {
      fireEvent.click(element);
    }, element);
  }

  async choose(select: HTMLElement, label: string): Promise<void> {
    const option = await within(select).findByRole('option', { name: label }, { timeout: 5000 });
    this.act(() => {
      fireEvent.change(select, { target: { value: option.getAttribute('value') } });
    });
  }

  /** Every question on screen now, and that the screen stays small. */
  look(): void {
    for (const element of document.querySelectorAll('[data-decision]')) {
      this.seen.add(element.getAttribute('data-decision') ?? '');
    }
    expect(document.querySelectorAll('body *').length).toBeLessThan(ELEMENTS);
  }
}

const next = async (drive: Drive): Promise<void> => {
  drive.click(await screen.findByRole('link', { name: 'Next' }));
};

/** The saved answers: the save is done. */
const saved = async (): Promise<void> => {
  await waitFor(() => {
    expect(screen.getAllByRole('status').some((status) => status.textContent === 'Saved')).toBe(true);
  }, { timeout: 5000 });
};

/** The list the proposer suggests for the scenario's folder, read in as "Use this list" reads it: the draft's members with it. */
const withSuggestedList = async (at: Scenario): Promise<Record<string, unknown>> => {
  installFetch(simpleServer(at).handler);
  const id = brandId<'ImportId'>(1);
  const { plan } = await importApi.get(id);
  const edit = await listEdit(id, plan, 'all', { source: 's1', file: at.sheet.file });
  const listed = flatPlan(asRecord(JSON.parse(JSON.stringify(edit(plan))), []));
  return { sheets: listed['sheets'], slots: listed['slots'] };
};

const markExample = async (drive: Drive, mark: Scenario['marks'][number]): Promise<void> => {
  drive.click(await screen.findByRole('button', { name: 'Mark an example' }, { timeout: 5000 }));
  const find = await screen.findByRole('searchbox', { name: 'Find a file name' });
  fireEvent.change(find, { target: { value: mark.find } });
  drive.click(screen.getByRole('button', { name: 'Find' }));
  drive.click(await screen.findByRole('radio', { name: mark.pick }));
  const parts = await screen.findByRole('group', { name: `Parts of the name ${mark.pick}` });
  for (const part of mark.parts) {
    drive.click(within(parts).getByRole('button', { name: part }));
  }
  for (const word of mark.ticks) {
    const box = await screen.findByRole('checkbox', { name: new RegExp(`^${word} `, 'u') });
    if (!(box instanceof HTMLInputElement) || !box.checked) {
      drive.click(box);
    }
  }
  const use = await screen.findByRole('button', { name: 'Use this' }, { timeout: 5000 });
  await waitFor(() => {
    expect(use).toBeEnabled();
  });
  expect(within(screen.getByRole('region', { name: 'By hand' })).getByText(/^Written this way: /)).toBeInTheDocument();
  drive.look();
  drive.click(use);
  await saved();
  await waitFor(() => {
    expect(screen.queryByRole('region', { name: 'By hand' })).toBeNull();
  });
  drive.look();
};

/** The tree of the mode's Structure, once that mode has drawn it: the other mode's stays a moment after the switch. */
const treeOf = (mode: 'simple' | 'advanced'): Promise<HTMLElement> =>
  waitFor(() => {
    const shown = document.querySelector(`.import-${mode} [role="listbox"]`);
    if (!(shown instanceof HTMLElement)) {
      throw new Error(`${mode} has not drawn its tree`);
    }
    return shown;
  }, { timeout: 5000 });

/** Leaves out the group the scenario's `leave` words describe, on Advanced's archive node, and comes back to Simple. */
const leaveGroup = async (drive: Drive, words: readonly string[]): Promise<void> => {
  drive.click(screen.getByRole('link', { name: 'Advanced' }));
  // The archive's root comes first; the folder written by hand over all of it has the same name.
  const tree = await treeOf('advanced');
  const [root] = within(tree).getAllByRole('option', { name: 'All of archive' });
  if (root === undefined) {
    throw new Error('no archive node');
  }
  drive.click(root);
  const found = await waitFor(() => {
    const group = [...document.querySelectorAll('article.import-group')].find((one) => words.every((word) => one.textContent.includes(word)));
    if (!(group instanceof HTMLElement)) {
      throw new Error(`no group with ${words.join(', ')}`);
    }
    return group;
  }, { timeout: 5000 });
  drive.look();
  drive.click(within(found).getByRole('button', { name: 'Leave them out' }));
  drive.click(await within(found).findByRole('button', { name: /^Change these/ }, { timeout: 5000 }));
  await saved();
  drive.click(screen.getByRole('link', { name: 'Simple' }));
  // The mode switch keeps the archive's node: the folder is the second "All of archive".
  const back = await treeOf('simple');
  const folder = within(back).getAllByRole('option', { name: 'All of archive' })[1];
  if (folder === undefined) {
    throw new Error('no folder node');
  }
  drive.click(folder);
};

const pass = async (at: Scenario): Promise<{ readonly drive: Drive; readonly server: ReturnType<typeof simpleServer> }> => {
  const server = simpleServer(at, await withSuggestedList(at));
  installFetch(server.handler);
  window.location.hash = '#/import';
  render(<Routed />);
  const drive = new Drive(at);

  drive.click(await screen.findByRole('button', { name: 'Use everything in archive' }));
  expect(await screen.findByRole('heading', { name: 'How your archive becomes items and sets' }, { timeout: 5000 })).toBeInTheDocument();
  await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
  drive.look();
  for (const mark of at.marks) {
    await markExample(drive, mark);
  }
  if (at.leave !== null) {
    await leaveGroup(drive, at.leave);
  }

  drive.click(await screen.findByRole('button', { name: 'Where each column goes' }, { timeout: 5000 }));
  expect(await screen.findByRole('region', { name: at.sheet.file }, { timeout: 5000 })).toBeInTheDocument();
  drive.look();
  for (const [column, label] of at.mapping) {
    await drive.choose(await screen.findByRole('combobox', { name: `Where the column ${column} goes` }, { timeout: 5000 }), label);
  }

  const from = await screen.findByRole('combobox', { name: 'The item type comes from' }, { timeout: 5000 });
  if (at.typeFrom === 'top') {
    await drive.choose(from, 'Top folder');
  }
  expect(await screen.findByRole('region', { name: 'Item type for each value' }, { timeout: 5000 })).toBeInTheDocument();
  drive.look();
  drive.click(within(screen.getByRole('list', { name: 'Steps' })).getByRole('link', { name: 'Check' }));

  const accept = await screen.findByRole('button', { name: /^Use the [\d,]+ suggested answers$/ }, { timeout: 5000 });
  drive.look();
  drive.click(accept);
  expect(await screen.findByText('Every question has an answer.')).toBeInTheDocument();
  await next(drive);

  // Add may save the suggested title first, which draws the button again: read it after.
  await screen.findByRole('button', { name: /^Add [\d,]+ items? to Omeka$/ }, { timeout: 5000 });
  await waitFor(() => {
    expect(screen.getByRole('button', { name: /^Add [\d,]+ items? to Omeka$/ })).toBeEnabled();
  }, { timeout: 3000 });
  const add = screen.getByRole('button', { name: /^Add [\d,]+ items? to Omeka$/ });
  drive.look();
  drive.click(add);
  expect(await screen.findByText('Writing to Omeka...', {}, { timeout: 5000 })).toBeInTheDocument();
  return { drive, server };
};

afterEach(() => {
  window.location.hash = '';
});

describe.each([
  ['meera', 9],
  ['bhc', 9],
  ['meera', 40_000],
  ['bhc', 40_000],
] as const)('the Simple pass, %s at %i items', (name, size) => {
  it('asks what the prototype asked, with as many clicks, and starts the run on the check it showed', async () => {
    const at = scenario(name, size);
    const { drive, server } = await pass(at);
    const decisions = `decisions ${String(drive.seen.size)}: ${[...drive.seen].join(' | ')}`;
    expect(drive.seen.size, decisions).toBeLessThanOrEqual(at.proto.decisions + 3);
    expect([...drive.seen], decisions).toEqual(expect.arrayContaining(['where', 'type-from', 'who', ...at.mapping.map(([column]) => `column:${column}`)]));
    const trip = at.leave === null ? 0 : GROUP_ROUND_TRIP;
    expect(Math.abs(drive.clicks - trip - at.proto.clicks), `clicks ${String(drive.clicks)}, ${String(trip)} of them the trip to Advanced's groups`).toBeLessThanOrEqual(3);

    expect(server.runs).toHaveLength(1);
    expect(String(server.runs[0]?.['digest'])).toMatch(/^digest-\d+$/);
    const plan = server.saved[server.saved.length - 1] ?? {};
    const grouping = asRecord(plan['grouping'], []);
    expect(String(grouping['pattern'])).toContain('(?<part>');
    const sheets = Array.isArray(plan['sheets']) ? plan['sheets'] : [];
    expect(sheets).toHaveLength(1);
    const sheet = asRecord(sheets[0], []);
    expect(asRecord(sheet['read'], [])['headerLine']).toBe(at.sheet.headerLine);
    expect(asRecord(asRecord(sheet['join'], [])['column'], [])['header']).toBe(at.sheet.columns[0]?.header);
    const item = asRecord(asRecord(plan['slots'], [])['item'], []);
    expect(Object.keys(item)).toEqual(expect.arrayContaining(['dcterms:title', 'template']));
    if (name === 'meera') {
      expect(JSON.stringify(item['dcterms:date'])).toContain('"header":"Year"');
      expect(JSON.stringify(item['template'])).toContain('"header":"Type"');
    } else {
      expect(asRecord(item['template'], [])['sources']).toEqual([{ from: 'folder', level: 1 }]);
      expect(JSON.stringify(plan['decisions'])).toContain('"kind":"many"');
    }
    const example = asRecord(plan['example'], []);
    expect(Array.isArray(example['marks']) ? example['marks'] : []).toHaveLength(at.marks.length);
    expect(example['path']).toEqual(expect.stringContaining(at.marks[at.marks.length - 1]?.pick ?? ''));
  }, 60_000);
});
