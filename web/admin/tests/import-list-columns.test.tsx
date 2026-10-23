/**
 * Where each column of BHC006's Artifacts list goes, on the Structure node.
 * The rows describe files, so each column the plan maps goes on the item or
 * on each file. The proposer picks one (rule C, `proposal/grain.rs`) and
 * counts both: the segments show those counts, the pick carries the ✓, and
 * a click moves the column through the Structure clicks.
 *
 * The proposal is the engine's recording of the replica
 * (`fixtures/import/proposal-bhc006.json`), whose lists hold only the
 * `Identifier` cells. Three columns are added to it with the counts the R1
 * probe read from the real catalogue: Keywords differ in 42 of 272 shared
 * items and never disjointly, Digitized by differs in 1 (a case difference),
 * the year in none; 312 items and 679 files get a value from each. Their
 * column numbers are this test's. The plan maps all three on the item.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';
const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const SHEET = 'BHC006_GMT_AF.xlsx';

const KEYWORDS = { index: 5, header: 'Keywords' };
const YEAR = { index: 6, header: 'Year Of Digitization' };
const DIGITIZED = { index: 7, header: 'Digitized by' };

const rollUp = (column: { readonly index: number; readonly header: string }, groups: number, disjoint: number) => ({
  ...column,
  varies: { groups, disjoint, of: 272 },
  pick: { chosen: { value: 'item', counts: { items: 312, files: 0 } }, rival: { value: 'file', counts: { items: 0, files: 679 } }, others: [] },
});

const fromColumn = (column: { readonly index: number; readonly header: string }) => ({ from: 'column', sheet: SHEET, column });

const ON_THE_ITEM = {
  'dcterms:subject': { sources: [fromColumn(KEYWORDS)], combine: 'all' },
  'dcterms:dateSubmitted': { sources: [fromColumn(YEAR)], combine: 'first' },
  'dcterms:contributor': { sources: [fromColumn(DIGITIZED)], combine: 'all' },
};

const records = (value: unknown): Record<string, unknown>[] => (Array.isArray(value) ? value.filter(isRecord) : []);

/** The list choice with its chosen list counted at `columns` columns. */
const withColumns = (list: unknown, columns: number): Record<string, unknown> => {
  const choice = asRecord(list, []);
  const chosen = asRecord(choice['chosen'], []);
  return { ...choice, chosen: { ...chosen, counts: { ...asRecord(chosen['counts'], []), columns } } };
};

/** The replica's proposal with the three columns counted, and its structure mapping them on the item. */
const PROPOSAL = ((): Record<string, unknown> => {
  const recorded = importFixture('proposal-bhc006.json');
  const structure = asRecord(recorded['structure'], []);
  const levels = records(structure['levels']).map((level) =>
    level['id'] === 'Artifacts' ? { ...level, slots: { ...asRecord(level['slots'], []), item: ON_THE_ITEM } } : level,
  );
  const nodes = records(recorded['nodes']).map((node) =>
    node['level'] === 'Artifacts'
      ? { ...node, list: withColumns(node['list'], 8), columns: [...records(node['columns']), rollUp(KEYWORDS, 42, 0), rollUp(YEAR, 0, 0), rollUp(DIGITIZED, 1, 1)] }
      : node,
  );
  return { ...recorded, structure: { ...structure, levels }, nodes };
})();

/**
 * A draft whose plan holds that structure. A preview answers the proposal
 * with the structure of the plan it is sent, which is what the proposer does
 * with the slot rules of a suggested series (it carries them), and says
 * whether that matches the stored plan. A save stores the plan.
 */
const server = (): { readonly saved: Record<string, unknown>[] } => {
  const detail = importFixture('import-draft.json');
  let plan: Record<string, unknown> = { ...asRecord(detail['plan'], []), structure: PROPOSAL['structure'] };
  let revision = 0;
  const saved: Record<string, unknown>[] = [];
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
        const answer: Record<string, unknown> = { sources: importFixture('preview-check.json')['sources'], pattern: { groups: ['identifier'] } };
        for (const section of include) {
          if (section === 'proposal') {
            answer[section] = { ...PROPOSAL, structure: sent['structure'], matchesPlan: true };
          } else if (section === 'sheets' || section === 'columnMatches' || section === 'listCandidates') {
            answer[section] = [];
          } else {
            answer[section] = importFixture(`preview-${section}.json`)[section];
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

const inspector = (): HTMLElement => {
  const found = screen.getByRole('listbox', { name: 'Your archive' }).parentElement?.querySelector('.guided-inspector');
  if (!(found instanceof HTMLElement)) {
    throw new Error('no inspector');
  }
  return found;
};

/** Each segment of a question: its text, and whether it is pressed. */
const segments = (within_: HTMLElement, question: string): readonly (readonly [string | null, boolean])[] =>
  Array.from(within(within_).getByRole('group', { name: question }).querySelectorAll('button'), (button) => [
    button.textContent,
    button.getAttribute('aria-pressed') === 'true',
  ] as const);

const slotsOf = (plan: Record<string, unknown> | undefined, kind: 'item' | 'media'): Record<string, unknown> => {
  const artifacts = records(asRecord(plan?.['structure'], [])['levels']).find((level) => level['id'] === 'Artifacts');
  return asRecord(asRecord(artifacts?.['slots'], [])[kind], []);
};

const strip = (): readonly string[] =>
  within(screen.getByRole('region', { name: 'Your changes' }))
    .getAllByRole('listitem')
    .map((item) => item.querySelector('span')?.textContent ?? '');

afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('where each column goes, on the Structure node', () => {
  it('Keywords goes on the item though it differs in 42 items', async () => {
    server();
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const artifacts = inspector();

    expect(within(artifacts).getByText('8 columns · 3 differ inside items')).toBeTruthy();
    expect(segments(artifacts, 'Where Keywords goes')).toEqual([
      ['On the item✓ our suggestion312 items', true],
      ['On each file679 files', false],
    ]);
    expect(within(artifacts).getByText('Differs in 42 of 272 items, always shares values.')).toBeTruthy();
    expect(segments(artifacts, 'Where Digitized by goes')).toEqual([
      ['On the item✓ our suggestion312 items', true],
      ['On each file679 files', false],
    ]);
    expect(within(artifacts).getByText('Differs in 1 of 272 items, with no shared value in 1.')).toBeTruthy();
    expect(within(artifacts).getByText('Same in every item.')).toBeTruthy();
    // A column no field reads places nothing, so it is not asked.
    expect(within(artifacts).queryByRole('group', { name: 'Where Identifier goes' })).toBeNull();
    expect(screen.queryByRole('region', { name: 'Your changes' })).toBeNull();
  });

  it('a click puts Digitized by on each file as one change, and Take back returns it', { timeout: 20_000 }, async () => {
    const { saved } = server();
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });

    fireEvent.click(within(within(inspector()).getByRole('group', { name: 'Where Digitized by goes' })).getByRole('button', { name: /^On each file/ }));
    await waitFor(() => {
      expect(saved).toHaveLength(1);
    });
    expect(slotsOf(saved[0], 'media')).toEqual({ 'dcterms:contributor': ON_THE_ITEM['dcterms:contributor'] });
    expect(Object.keys(slotsOf(saved[0], 'item'))).toEqual(['dcterms:subject', 'dcterms:dateSubmitted']);
    expect(saved[0]?.['edits']).toBeUndefined();

    await waitFor(() => {
      expect(strip()).toEqual(['Artifacts · Digitized by: On each file']);
    });
    expect(segments(inspector(), 'Where Digitized by goes')).toEqual([
      ['On the item✓ our suggestion312 items', false],
      ['On each file679 files', true],
    ]);
    expect(screen.getByRole('option', { name: 'Artifacts' }).querySelector('.guided-row-facts')?.textContent).toMatch(/ · changed$/);
    expect(screen.getByText('Your changes', { selector: '.guided-facts > span' }).textContent).toBe('Your changes 1');

    fireEvent.click(within(screen.getByRole('region', { name: 'Your changes' })).getByRole('button', { name: 'Take back' }));
    await waitFor(() => {
      expect(saved).toHaveLength(2);
    });
    expect(slotsOf(saved[1], 'media')).toEqual({});
    expect(slotsOf(saved[1], 'item')['dcterms:contributor']).toEqual(ON_THE_ITEM['dcterms:contributor']);
    await waitFor(() => {
      expect(screen.queryByRole('region', { name: 'Your changes' })).toBeNull();
    });
  });

  it('Advanced shows the same pick beside the column in the Fields section', async () => {
    server();
    render(<ImportPage param="1" rest={['1', 'advanced', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Fields' }));
    const card = await within(inspector()).findByRole('article', { name: /dcterms:subject/ }, { timeout: 5000 });
    expect(segments(card, 'Where Keywords goes')).toEqual([
      ['On the item✓ our suggestion312 items', true],
      ['On each file679 files', false],
    ]);
    expect(within(card).getByText('Differs in 42 of 272 items, always shares values.')).toBeTruthy();
  });
});
