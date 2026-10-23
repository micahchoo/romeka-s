/**
 * Item sets in Advanced's Structure (P5 unit C3): the archive's node lists
 * every free set the plan holds, each with the sets it makes; a set lifted
 * from v2 offers both of v2's readings with their counts before the click;
 * a folder's set and a set level's are edited on their node. The proposals
 * are the engine's recordings (`fixtures/import/proposal-bhc006*.json`).
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';
const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const body = (init: RequestInit | undefined): Record<string, unknown> => asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
const array = (value: unknown): readonly unknown[] => (Array.isArray(value) ? value : []);

/** The free set the Simple and Advanced item sets rule wrote, one set per top folder. */
const BY_FOLDER = { each: { sources: [{ from: 'folder', level: 1 }], combine: 'first', names: 'title' } };
/** The second free set a list's `[sets]` column makes when the list comes back (`template.rs`, `add_list`). */
const FROM_LIST = { each: { sources: [{ from: 'column', sheet: 'BHC006_GMT_AF.xlsx', column: { index: 9, header: '[sets]' } }], combine: 'first', names: 'title' } };
/** The v2 sets rule as the lift writes it (`fixtures/import/import-saved.json`). */
const LIFTED = { each: { sources: [{ from: 'group', name: 'set' }], combine: 'first', map: { Scans: '' }, names: 'text' } };

const value = (text: string, count: number) => ({ text, count, spellings: [text], to: null });

interface Server {
  readonly saved: Record<string, unknown>[];
  /** The plan each preview was sent, with what it asked for. */
  readonly asked: { readonly include: readonly string[]; readonly plan: Record<string, unknown> }[];
}

/**
 * BHC006 whose stored plan holds `plan` over the draft's. A proposal is
 * `proposal`'s recording (or `proposal` itself), each edit sent marked applied; the `values` of a
 * plan with one free set are `values(that set)`; every other section is its
 * wire fixture. The install has the two sets of `item-sets-page.json`.
 */
const bhc006 = ({ proposal = 'proposal-bhc006.json', plan: over = {}, values = () => [] }: { readonly proposal?: string | Record<string, unknown>; readonly plan?: Record<string, unknown>; readonly values?: (set: unknown) => readonly unknown[] }): Server => {
  const detail = importFixture('import-draft.json');
  const recorded = typeof proposal === 'string' ? importFixture(proposal) : proposal;
  const base = asRecord(detail['plan'], []);
  let plan: Record<string, unknown> = { ...base, structure: recorded['structure'], ...over };
  if (isRecord(over['structure'])) {
    plan = { ...plan, structure: { ...asRecord(recorded['structure'], []), ...over['structure'] } };
  }
  let revision = 0;
  const saved: Record<string, unknown>[] = [];
  const asked: Server['asked'] = [];
  const templates = loadFixture('resource-templates-page.json');
  installFetch(
    route({
      [PATH]: (_url, init) => {
        if (init?.method === 'PUT') {
          plan = asRecord(body(init)['plan'], []);
          saved.push(plan);
          revision += 1;
        }
        return jsonResponse({ ...detail, plan, revision });
      },
      [`${PATH}/preview`]: (_url, init) => {
        const sent = body(init);
        const include = array(sent['include']).map(String);
        const tried = isRecord(sent['plan']) ? sent['plan'] : plan;
        asked.push({ include, plan: tried });
        const edits = array(tried['edits']);
        const answer: Record<string, unknown> = { sources: importFixture('preview-check.json')['sources'], pattern: { groups: ['identifier'] } };
        for (const section of include) {
          if (section === 'proposal') {
            answer[section] = { ...recorded, edits: edits.map((edit) => ({ edit, applied: true, reason: null })) };
          } else if (section === 'values') {
            const free = array(asRecord(tried['structure'], [])['freeSets']);
            const items = free.length === 1 ? values(free[0]) : [];
            answer[section] = ['sets', 'template', 'class'].map((slot) => ({
              slot,
              values: { items: slot === 'sets' ? items : [], total: slot === 'sets' ? items.length : 0, page: 1, perPage: 500 },
              near: [],
            }));
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
      '/api/resource_templates': () => listResponse(templates, array(templates).length),
    }),
  );
  return { saved, asked };
};

const open = async (): Promise<HTMLElement> => {
  render(<ImportPage param="1" rest={['1', 'advanced', 'structure']} />);
  return screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
};

const inspector = (): HTMLElement => {
  const found = screen.getByRole('listbox', { name: 'Your archive' }).parentElement?.querySelector('.guided-inspector');
  if (!(found instanceof HTMLElement)) {
    throw new Error('no inspector');
  }
  return found;
};

const pick = async (name: string): Promise<void> => {
  const tree = await open();
  const option = within(tree).getAllByRole('option').find((one) => one.querySelector('.guided-row-name')?.textContent === name);
  fireEvent.click(option ?? tree);
};

const freeSetsOf = (plan: Record<string, unknown> | undefined): readonly unknown[] => array(asRecord(plan?.['structure'], [])['freeSets']);

/** The series `level` of a plan's tree, at any depth. */
const seriesIn = (plan: Record<string, unknown>, level: string): Record<string, unknown> | undefined => {
  const find = (levels: readonly unknown[]): Record<string, unknown> | undefined => {
    for (const one of levels) {
      const found = isRecord(one) ? (one['kind'] === 'series' && one['id'] === level ? one : find(array(one['levels']))) : undefined;
      if (found !== undefined) {
        return found;
      }
    }
    return undefined;
  };
  return find(array(asRecord(plan['structure'], [])['levels']));
};

const setLevelIn = (plan: Record<string, unknown>, id: string): Record<string, unknown> | undefined =>
  array(asRecord(plan['structure'], [])['levels']).find((one): one is Record<string, unknown> => isRecord(one) && one['kind'] === 'set' && one['id'] === id);

afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('item sets in Advanced', () => {
  it('every free set shows, the list upload’s second one too', async () => {
    const server = bhc006({
      plan: { structure: { freeSets: [BY_FOLDER, FROM_LIST] } },
      values: (set) => (JSON.stringify(set) === JSON.stringify(BY_FOLDER) ? [value('Artifacts', 318), value('Photo Narratives (PN)', 215), value('Extras', 163), value('Documents (D)', 15)] : [value('Exhibition 2018', 12)]),
    });
    await pick('All of archive');
    const own = await within(inspector()).findByRole('region', { name: 'Your own sets' }, { timeout: 5000 });
    await waitFor(() => {
      expect(within(own).getAllByRole('listitem').map((row) => row.querySelectorAll('p')[1]?.textContent)).toEqual([
        'Artifacts (318 items) · Photo Narratives (PN) (215 items) · Extras (163 items) · and 1 more',
        'Exhibition 2018 (12 items)',
      ]);
    });
    const [first, second] = within(own).getAllByRole('listitem');
    expect(first?.querySelector('p')?.textContent).toBe('One set per value of Folder level 1');
    expect(second?.querySelector('p')?.textContent).toMatch(/^One set per value of .*\[sets\]/u);
    // Each free set's sets are counted on a plan where it is the only set.
    const counted = server.asked.filter(({ include }) => include.includes('values'));
    expect(counted.map(({ plan }) => freeSetsOf(plan))).toEqual(expect.arrayContaining([[BY_FOLDER], [FROM_LIST]]));
    expect(counted.every(({ plan }) => array(asRecord(plan['structure'], [])['levels']).every((level) => isRecord(level) && level['set'] === null))).toBe(true);

    fireEvent.click(within(second ?? own).getByRole('button', { name: 'Remove' }));
    await waitFor(() => {
      expect(freeSetsOf(server.saved[server.saved.length - 1])).toEqual([BY_FOLDER]);
    });
  });

  it('a v2 set rule read as titles writes names title', async () => {
    const server = bhc006({
      plan: { structure: { freeSets: [LIFTED] } },
      values: () => [value('Featured photographs', 40), value('1', 12), value('2', 9), value('Letters', 7)],
    });
    await pick('All of archive');
    const own = await within(inspector()).findByRole('region', { name: 'Your own sets' }, { timeout: 5000 });
    expect(within(own).getByText('These values are read as v2 read them: an id when a set has it, else a title.')).toBeInTheDocument();
    // The install has "Featured photographs" (#1) and "Working drafts" (#2).
    expect(await within(own).findByText('makes 3 new sets, finds 1')).toBeInTheDocument();
    expect(within(own).getByText('2 values name a set in Omeka; 2 name none')).toBeInTheDocument();
    expect(server.saved).toEqual([]);

    fireEvent.click(within(own).getByRole('button', { name: 'Read them as titles' }));
    await waitFor(() => {
      expect(freeSetsOf(server.saved[server.saved.length - 1])).toEqual([{ each: { ...LIFTED.each, names: 'title' } }]);
    });
  });

  it('adds a set of its own, by title, with a description', async () => {
    const server = bhc006({});
    await pick('All of archive');
    const own = await within(inspector()).findByRole('region', { name: 'Your own sets' }, { timeout: 5000 });
    expect(within(own).getByText('You have no set of your own.')).toBeInTheDocument();
    const add = within(own).getByRole('form', { name: 'Add a set' });
    fireEvent.change(within(add).getByLabelText('Title'), { target: { value: 'Exhibition 2018' } });
    fireEvent.change(within(add).getByLabelText('Description'), { target: { value: 'Shown at the town hall' } });
    fireEvent.click(within(add).getByRole('button', { name: 'Use' }));
    await waitFor(() => {
      expect(freeSetsOf(server.saved[server.saved.length - 1])).toEqual([{ title: 'Exhibition 2018', description: 'Shown at the town hall' }]);
    });
  });

  it('names a suggested folder’s set with a description, and keeps the click that made it', async () => {
    const server = bhc006({});
    await pick('Artifacts');
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Set' }));
    const name = await within(inspector()).findByRole('form', { name: 'Name this set' }, { timeout: 5000 });
    fireEvent.change(within(name).getByLabelText('Title'), { target: { value: 'Objects' } });
    fireEvent.change(within(name).getByLabelText('Description'), { target: { value: 'Things the families kept' } });
    fireEvent.click(within(name).getByRole('button', { name: 'Use' }));
    const artifacts = { from: 'folder', scope: { source: 's1', folder: 'Artifacts' } };
    await waitFor(() => {
      expect(server.asked.some(({ include, plan }) => include.includes('proposal') && JSON.stringify(plan['edits']) === JSON.stringify([{ edit: 'title', from: artifacts, title: 'Objects', description: 'Things the families kept' }]))).toBe(true);
    });

    // A set in Omeka answers the same question, and shows by the set's own title, read from the install.
    const again = await within(inspector()).findByRole('form', { name: 'Name this set' });
    fireEvent.change(within(again).getByRole('combobox', { name: 'Name this set' }), { target: { value: 'existing' } });
    fireEvent.change(await within(again).findByRole('combobox', { name: 'Set in Omeka' }), { target: { value: '1' } });
    fireEvent.click(within(again).getByRole('button', { name: 'Use' }));
    expect(await within(inspector()).findByText('In Omeka: Featured photographs')).toBeInTheDocument();
    expect(server.asked.filter(({ include }) => include.includes('proposal')).at(-1)?.plan['edits']).toEqual([{ edit: 'existing', from: artifacts, id: 1 }]);
  });

  it('a folder written by hand writes its set, a set already in Omeka, into the plan', async () => {
    const server = bhc006({ proposal: 'proposal-bhc006-own.json', plan: { edits: [{ edit: 'own', scope: { source: 's1', folder: 'Artifacts' } }] } });
    await pick('Artifacts');
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Set' }));
    const set = await within(inspector()).findByRole('form', { name: 'The items of this folder join' }, { timeout: 5000 });
    fireEvent.change(within(set).getByRole('combobox', { name: 'The items of this folder join' }), { target: { value: 'existing' } });
    fireEvent.change(await within(set).findByRole('combobox', { name: 'Set in Omeka' }), { target: { value: '2' } });
    fireEvent.click(within(set).getByRole('button', { name: 'Use' }));
    await waitFor(() => {
      const tried = server.asked.filter(({ include }) => include.includes('proposal')).map(({ plan }) => plan);
      expect(tried.map((plan) => seriesIn(plan, 'Artifacts')?.['set'])).toContainEqual({ id: 2 });
    });
    const last = server.asked.filter(({ include }) => include.includes('proposal')).at(-1)?.plan ?? {};
    expect(last['edits']).toEqual([{ edit: 'own', scope: { source: 's1', folder: 'Artifacts' } }]);
  });

  it('a set level kept around a folder written by hand shows its sets and takes a title', async () => {
    const kept = importFixture('proposal-bhc006-own-set.json');
    const documents = { source: 's1', folder: 'Documents (D)' };
    const server = bhc006({
      proposal: 'proposal-bhc006-own-set.json',
      plan: { structure: kept['structure'], edits: [{ edit: 'addSet', from: { from: 'namePlace', scope: documents, place: 4 } }, { edit: 'own', scope: documents }] },
    });
    await pick('One set per value');
    expect(within(inspector()).getByText('20170616 (11 items) · 20171013 (4 items)')).toBeInTheDocument();
    const level = within(inspector()).getByRole('form', { name: 'This level makes' });
    fireEvent.change(within(level).getByRole('combobox', { name: 'This level makes' }), { target: { value: 'title' } });
    fireEvent.change(within(level).getByLabelText('Title'), { target: { value: 'Dated' } });
    fireEvent.click(within(level).getByRole('button', { name: 'Use' }));
    await waitFor(() => {
      const tried = server.asked.filter(({ include }) => include.includes('proposal')).map(({ plan }) => setLevelIn(plan, 'Documents (D)4')?.['set']);
      expect(tried).toContainEqual({ title: 'Dated', description: null });
    });
  });

  it('a split set level around one folder maps what each value’s set becomes', { timeout: 20_000 }, async () => {
    // BHC006 with Documents' name place 4 taken: the split level the proposer counts around the suggested folder.
    const kept = importFixture('proposal-bhc006-own-set.json');
    const suggested = array(importFixture('proposal-bhc006.json')['nodes']).find((node) => isRecord(node) && node['level'] === 'Documents (D)');
    const documents = { source: 's1', folder: 'Documents (D)' };
    const place4 = { from: 'namePlace', scope: documents, place: 4 };
    const counts = { sets: [['20170616', 11], ['20171013', 4]], rows: null };
    const nodes = array(kept['nodes']).map((node) =>
      !isRecord(node) ? node : node['kind'] === 'kept' ? { kind: 'set', level: 'Documents (D)4', from: place4, counts } : node['kind'] === 'own' ? suggested : node,
    );
    const server = bhc006({
      proposal: { ...kept, nodes },
      plan: { structure: kept['structure'], edits: [{ edit: 'addSet', from: place4 }] },
      values: () => [value('20170616', 11), value('20171013', 4)],
    });
    await pick('2 sets');
    const map = (await within(inspector()).findByText('Value map of 2 sets', {}, { timeout: 5000 })).closest('details');
    if (!(map instanceof HTMLDetailsElement)) {
      throw new Error('no value map');
    }
    map.open = true;
    fireEvent(map, new Event('toggle'));

    // The values are counted on the level's own each, alone, over the folder's items.
    const june = await within(map).findByRole('combobox', { name: 'What 20170616 becomes' }, { timeout: 5000 });
    const level = setLevelIn({ structure: kept['structure'] }, 'Documents (D)4');
    const counted = server.asked.filter(({ include }) => include.includes('values')).at(-1)?.plan;
    expect(freeSetsOf(counted)).toEqual([level?.['set']]);
    expect(array(asRecord(counted?.['structure'], [])['levels']).some((one) => isRecord(one) && one['kind'] === 'set')).toBe(false);

    fireEvent.change(june, { target: { value: 'to:Featured photographs' } });
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    });
    const each = asRecord(asRecord(setLevelIn(server.saved[0] ?? {}, 'Documents (D)4')?.['set'], [])['each'], []);
    expect(each['map']).toEqual({ '20170616': 'Featured photographs' });
  });
});
