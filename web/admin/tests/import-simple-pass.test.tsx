/**
 * The whole Simple pass, from the start page to "Add", over the prototype's
 * two folders at 9 and at 40,000 items (`import-simple-scenarios.ts`). The
 * drive does what `proto4/drive4.mjs` does in its first pass: it marks one
 * example (two above 9 items), confirms the groups, maps the two columns that
 * matter, takes the item type from the list or the top folder, accepts the
 * suggested answers on Check and starts the run.
 *
 * It counts what `SCALE.md` counts. A decision is a question the screens put
 * to the person (each `data-decision`, each group of items), counted once
 * however often it shows, plus "where are your files". A click is every
 * click, search and choice from the start page to the start. Both must be
 * within 3 of the prototype's, and no screen may draw more than about 1,000
 * elements.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
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
    for (const group of document.querySelectorAll('.import-group > p:first-child')) {
      const count = group.querySelector('b')?.textContent ?? '';
      this.seen.add(`group:${group.textContent.replace(count, '').trim()}`);
    }
    expect(document.querySelectorAll('body *').length).toBeLessThan(ELEMENTS);
  }
}

const next = async (drive: Drive): Promise<void> => {
  drive.click(await screen.findByRole('link', { name: 'Next' }));
};

/** The groups as the saved answers make them: the save is done and the groups read again. */
const groupsShown = async (): Promise<void> => {
  await waitFor(() => {
    expect(screen.getAllByRole('status').some((status) => status.textContent === 'Saved')).toBe(true);
    expect(screen.getByRole('region', { name: 'Your items in groups' })).toHaveAttribute('aria-busy', 'false');
  }, { timeout: 5000 });
};

const markExample = async (drive: Drive, mark: Scenario['marks'][number], first: boolean): Promise<void> => {
  drive.click(screen.getByRole('link', { name: first ? 'Show us one example' : 'Mark another part' }));
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
  expect(screen.getByText(/^With this, your files make [\d,]+ items?:/)).toBeInTheDocument();
  drive.look();
  drive.click(use);
  expect(await screen.findByText(/^Files are one item when their names differ only in the part you marked/, {}, { timeout: 5000 })).toBeInTheDocument();
  await groupsShown();
  drive.look();
};

const pass = async (at: Scenario): Promise<{ readonly drive: Drive; readonly server: ReturnType<typeof simpleServer> }> => {
  const server = simpleServer(at);
  installFetch(server.handler);
  window.location.hash = '#/import';
  render(<Routed />);
  const drive = new Drive(at);

  drive.click(await screen.findByRole('button', { name: 'Use everything in archive' }));
  expect(await screen.findByText(/^Right now, files are one item only when their names are the same except for the ending/, {}, { timeout: 5000 })).toBeInTheDocument();
  await groupsShown();
  drive.look();
  for (const [index, mark] of at.marks.entries()) {
    await markExample(drive, mark, index === 0);
  }
  if (at.leave !== null) {
    const words = at.leave;
    const group = [...document.querySelectorAll<HTMLElement>('.import-group')].find((one) => words.every((word) => one.textContent.includes(word)));
    expect(group).toBeDefined();
    if (group !== undefined) {
      drive.click(within(group).getByRole('button', { name: 'Leave them out' }));
      drive.click(await within(group).findByRole('button', { name: /^Change these/ }, { timeout: 5000 }));
    }
  }
  drive.click(screen.getByRole('button', { name: 'Yes, use the suggested answers' }));
  expect(await screen.findByText('Every group is confirmed.')).toBeInTheDocument();
  await next(drive);

  expect(await screen.findByRole('listitem', { name: at.sheet.file }, { timeout: 5000 })).toHaveTextContent('In use');
  expect(await screen.findByText('Where each column goes', {}, { timeout: 5000 })).toBeInTheDocument();
  drive.look();
  for (const [column, label] of at.mapping) {
    await drive.choose(screen.getByRole('combobox', { name: `Where the column ${column} goes` }), label);
  }
  await next(drive);

  const from = await screen.findByRole('combobox', { name: 'The item type comes from' }, { timeout: 5000 });
  if (at.typeFrom === 'top') {
    await drive.choose(from, 'The top folder of each file');
  }
  expect(await screen.findByRole('region', { name: 'Item type for each value' }, { timeout: 5000 })).toBeInTheDocument();
  drive.look();
  await next(drive);

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
    expect(Math.abs(drive.seen.size - at.proto.decisions), `decisions ${String(drive.seen.size)}: ${[...drive.seen].join(' | ')}`).toBeLessThanOrEqual(3);
    expect(Math.abs(drive.clicks - at.proto.clicks), `clicks ${String(drive.clicks)}`).toBeLessThanOrEqual(3);

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
