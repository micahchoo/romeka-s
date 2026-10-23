/**
 * The value map control (P6 design, section 2): one control for every place
 * a value becomes another. W3's case is BHC006 Artifacts' "Digitized by":
 * `Dilp Patil` on 2 items, `Dilip Patil` on 312, and `Supriya Nandgouli`
 * written two ways on 499. The server counts the values before the map,
 * says what the map makes each one (`to`) and offers the near spelling; the
 * counts are the V1 probe's on the real catalogue (`Dilip Patil` 307 there
 * read through the files; 312 items here, the design's count).
 *
 * The draft is the replica's proposal with the column mapped on the item, as
 * `import-list-columns.test.tsx` builds it. The test server answers `values`
 * from the plan it holds, as the real one does.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import type { ImportPlan } from '../src/api/import-plan';
import { parseImportPlan } from '../src/api/import-plan-tree-read';
import { ImportPage } from '../src/pages/import/page';
import { mapOf, withEntry, withMap, withMapped } from '../src/pages/import/value-map-edits';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';
const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const SHEET = 'BHC006_GMT_AF.xlsx';
const DIGITIZED = { index: 7, header: 'Digitized by' };
const CONTRIBUTOR = { sources: [{ from: 'column', sheet: SHEET, column: DIGITIZED }], combine: 'all' };

/** The values of "Digitized by" before the map, the most items first. */
const WRITTEN = [
  { text: 'Supriya Nandgouli', count: 499, spellings: ['Supriya Nandgouli', 'supriya Nandgouli'] },
  { text: 'Dilip Patil', count: 312, spellings: ['Dilip Patil'] },
  { text: 'Dilp Patil', count: 2, spellings: ['Dilp Patil'] },
];

const fold = (text: string): string => text.trim().normalize('NFC').toLowerCase();

const records = (value: unknown): Record<string, unknown>[] => (Array.isArray(value) ? value.filter(isRecord) : []);

const PROPOSAL = ((): Record<string, unknown> => {
  const recorded = importFixture('proposal-bhc006.json');
  const structure = asRecord(recorded['structure'], []);
  const levels = records(structure['levels']).map((level) =>
    level['id'] === 'Artifacts' ? { ...level, slots: { ...asRecord(level['slots'], []), item: { 'dcterms:contributor': CONTRIBUTOR } } } : level,
  );
  return { ...recorded, structure: { ...structure, levels } };
})();

const artifacts = (plan: Record<string, unknown>): Record<string, unknown> | undefined =>
  records(asRecord(plan['structure'], [])['levels']).find((level) => level['id'] === 'Artifacts');

const contributorOf = (plan: Record<string, unknown> | undefined): Record<string, unknown> =>
  asRecord(asRecord(asRecord(artifacts(plan ?? {})?.['slots'], [])['item'], [])['dcterms:contributor'], []);

/** What the plan's map makes of each written value, and the near spelling while neither side is mapped. */
const valuesOf = (plan: Record<string, unknown>): Record<string, unknown> => {
  const map = asRecord(contributorOf(plan)['map'] ?? {}, []);
  const to = (text: string): string | null => {
    const key = Object.keys(map).find((one) => fold(one) === fold(text));
    return key === undefined ? null : String(map[key]);
  };
  const items = WRITTEN.map((value) => ({ ...value, to: to(value.text) }));
  const open = to('Dilp Patil') === null && to('Dilip Patil') === null;
  return {
    slot: 'dcterms:contributor',
    values: { total: items.length, page: 1, perPage: 50, items },
    near: open ? [{ from: 'Dilp Patil', fromRecords: 2, to: 'Dilip Patil', toRecords: 312 }] : [],
  };
};

const server = (): { readonly saved: Record<string, unknown>[]; readonly asked: unknown[] } => {
  const detail = importFixture('import-draft.json');
  let plan: Record<string, unknown> = { ...asRecord(detail['plan'], []), structure: PROPOSAL['structure'] };
  let revision = 0;
  const saved: Record<string, unknown>[] = [];
  const asked: unknown[] = [];
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
          } else if (section === 'values') {
            asked.push(body['values']);
            answer[section] = [valuesOf(sent)];
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
      '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
    }),
  );
  return { saved, asked };
};

const inspector = (): HTMLElement => {
  const found = screen.getByRole('listbox', { name: 'Your archive' }).parentElement?.querySelector('.guided-inspector');
  if (!(found instanceof HTMLElement)) {
    throw new Error('no inspector');
  }
  return found;
};

/** The selected option's text of a select. */
const shown = (select: HTMLElement): string | undefined => (select instanceof HTMLSelectElement ? select.selectedOptions[0]?.textContent ?? undefined : undefined);

afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('the value map', () => {
  it("a join moves Dilp Patil's 2 rows into Dilip Patil, and a click back removes the entry", { timeout: 20_000 }, async () => {
    const { saved, asked } = server();
    render(<ImportPage param="1" rest={['1', 'advanced', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Fields' }));
    const card = await within(inspector()).findByRole('article', { name: /dcterms:contributor/ }, { timeout: 5000 });
    const details = card.querySelector('details');
    if (!(details instanceof HTMLDetailsElement)) {
      throw new Error('no value map');
    }
    details.open = true;
    fireEvent(details, new Event('toggle'));

    const join = await within(card).findByRole('button', { name: 'Make it Dilip Patil' }, { timeout: 5000 });
    expect(asked.at(-1)).toEqual({ slot: 'dcterms:contributor', kind: 'item', series: 'Artifacts', page: 1, perPage: 50 });
    expect(join.closest('li')?.textContent).toBe('Dilp Patil, 2 items, looks like Dilip Patil, 312 items. Make it Dilip Patil');
    expect(shown(within(card).getByRole('combobox', { name: 'What Dilp Patil becomes' }))).toBe('As written');
    expect(within(card).getByText('2 spellings, one value')).toBeTruthy();

    fireEvent.click(join);
    await waitFor(() => {
      expect(saved).toHaveLength(1);
    });
    expect(contributorOf(saved[0])).toEqual({ ...CONTRIBUTOR, map: { 'Dilp Patil': 'Dilip Patil' } });
    await waitFor(() => {
      expect(within(card).queryByRole('button', { name: 'Make it Dilip Patil' })).toBeNull();
    });
    const dilp = within(card).getByRole('combobox', { name: 'What Dilp Patil becomes' });
    expect(shown(dilp)).toBe('Same as Dilip Patil');

    fireEvent.change(dilp, { target: { value: '' } });
    await waitFor(() => {
      expect(saved).toHaveLength(2);
    });
    expect(contributorOf(saved[1])).toEqual(CONTRIBUTOR);
    expect(await within(card).findByRole('button', { name: 'Make it Dilip Patil' }, { timeout: 5000 })).toBeTruthy();
  });
});

describe('the value map edits', () => {
  const plan = (): ImportPlan => {
    const detail = importFixture('import-draft.json');
    return parseImportPlan({ ...asRecord(detail['plan'], []), structure: PROPOSAL['structure'] });
  };
  const home = { level: 'Artifacts', kind: 'item', slot: 'dcterms:contributor' } as const;

  it('one entry per value, compared as a title; an empty map is absent', () => {
    expect(withMapped({ 'supriya nandgouli': 'S. Nandgouli' }, 'Supriya Nandgouli', 'Supriya Nandgouli')).toEqual({ 'Supriya Nandgouli': 'Supriya Nandgouli' });
    const joined = withEntry(plan(), home, 'Dilp Patil', 'Dilip Patil');
    expect(mapOf(joined, home)).toEqual({ 'Dilp Patil': 'Dilip Patil' });
    const back = withEntry(joined, home, 'dilp patil', undefined);
    expect(contributorOf(asRecord(JSON.parse(JSON.stringify(back)), []))).toEqual(CONTRIBUTOR);
  });

  it('a free set keeps its map on its own each, and a rule the plan lacks stays absent', () => {
    const each = { each: { sources: [{ from: 'column', sheet: SHEET, column: DIGITIZED }], combine: 'first', names: 'title' } } as const;
    const withSet: ImportPlan = { ...plan(), structure: { ...plan().structure, freeSets: [{ title: 'All', description: null }, each] } };
    const mapped = withEntry(withSet, { freeSet: 1 }, 'Dilp Patil', '');
    expect(mapped.structure.freeSets[1]).toEqual({ each: { ...each.each, map: { 'Dilp Patil': '' } } });
    expect(withMap(mapped, { freeSet: 1 }, {}).structure.freeSets[1]).toEqual(each);
    expect(withEntry(withSet, { freeSet: 0 }, 'x', 'y')).toBe(withSet);
    expect(withEntry(withSet, { ...home, slot: 'dcterms:subject' }, 'x', 'y')).toBe(withSet);
  });
});
