/**
 * Advanced's Structure over BHC006 after the first Structure click: the
 * tree Simple shows, each node with its rules. A folder opens to five
 * sections, each one fact line until opened. A choice the proposal offers
 * is Simple's click, so it survives the next one; the mode switch keeps the
 * node. The proposal is the engine's recording
 * (`fixtures/import/proposal-bhc006.json`); its numbers are the engine's.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';
const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const PROPOSAL = importFixture('proposal-bhc006.json');

const AF = { source: 's1', file: 'BHC006_GMT_AF.xlsx' };
const CSV = { source: 's1', file: 'Artifacts/BHC006_GMT_AF.csv' };
const ARTIFACTS = { source: 's1', folder: 'Artifacts' };

/**
 * BHC006 whose plan holds the proposal's structure. A preview answers the
 * recorded proposal with each edit of the plan it is sent marked applied;
 * the scan's list candidates are the two Artifacts lists; every other
 * section is its wire fixture. A save stores the plan.
 */
const bhc006 = (): { readonly saved: Record<string, unknown>[] } => {
  const detail = importFixture('import-draft.json');
  let plan: Record<string, unknown> = { ...asRecord(detail['plan'], []), structure: PROPOSAL['structure'] };
  let revision = 0;
  const saved: Record<string, unknown>[] = [];
  const all = importFixture('preview-list-candidates.json')['listCandidates'];
  const candidate = asRecord(Array.isArray(all) ? all[0] : null, []);
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
        const edits: readonly unknown[] = Array.isArray(sent['edits']) ? sent['edits'] : [];
        const answer: Record<string, unknown> = { sources: importFixture('preview-check.json')['sources'], pattern: { groups: ['identifier'] } };
        for (const section of include) {
          if (section === 'proposal') {
            answer[section] = { ...PROPOSAL, edits: edits.map((edit) => ({ edit, applied: true, reason: null })) };
          } else if (section === 'listCandidates') {
            answer[section] = [AF, CSV].map(({ file }) => ({ ...candidate, file, inPlan: file === AF.file ? 'BHC006_GMT_AF.xlsx' : null }));
          } else if (section === 'sheets' || section === 'columnMatches') {
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

/** Each section of the selected node: its name, its fact line, and whether it is open. */
const sections = (): readonly (readonly [string | null, string | null | undefined, string | null])[] =>
  Array.from(inspector().querySelectorAll('section.rule-section'), (section) => {
    const button = section.querySelector('h5 button');
    return [button?.textContent ?? null, section.querySelector('h5 .adv-plain')?.textContent, button?.getAttribute('aria-expanded') ?? null] as const;
  });

const answers = (question: string): readonly string[] =>
  Array.from(within(inspector()).getByRole('group', { name: question }).querySelectorAll('button'), (button) => button.textContent);

afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('Advanced’s Structure', () => {
  it('Advanced opens on BHC006’s tree, and Artifacts opens to its five sections', async () => {
    bhc006();
    const tree = await open();
    expect(screen.getByRole('heading', { name: 'Structure: each folder with its rules' })).toBeInTheDocument();
    const names = within(tree)
      .getAllByRole('option')
      .map((option) => option.querySelector('.guided-row-name')?.textContent);
    expect(names).toEqual([
      'All of archive',
      'Every item',
      'Artifacts',
      'Audio Interviews (AI)',
      'Documents (D)',
      'Extras',
      'Photo Narratives (PN)',
      'Video (V)',
      'tiny-iiif',
      'Artifacts/BHC006_GMT_AF.ods',
      'BHC006_CC_SECOND_AF.xlsx',
    ]);
    expect(within(inspector()).getByRole('heading', { name: 'Artifacts' })).toBeInTheDocument();
    expect(sections()).toEqual([
      ['Which files', '1,309 files · 696 pictures', 'false'],
      ['Items', 'Same name up to part 5 · 318 items', 'false'],
      ['Lists', 'BHC006_GMT_AF.xlsx', 'false'],
      ['Fields', '0 field rules', 'false'],
      ['Set', 'Artifacts', 'false'],
    ]);

    // Opened, Items asks Simple's question with every cut the proposer counted, and holds the pattern.
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Items' }));
    await waitFor(() => {
      expect(answers('Items')).toEqual(['Same name up to part 5✓ our suggestion318 items', 'Each name an item696 items', expect.stringMatching(/^Same name up to part 6/)]);
    });
    expect(await within(inspector()).findByLabelText('Regex')).toBeInTheDocument();
    expect(sections().map(([name, , opened]) => [name, opened])).toEqual([
      ['Which files', 'false'],
      ['Items', 'true'],
      ['Lists', 'false'],
      ['Fields', 'false'],
      ['Set', 'false'],
    ]);
  });

  it('the archive’s node holds the sources and the defaults', async () => {
    bhc006();
    const tree = await open();
    fireEvent.click(within(tree).getByRole('option', { name: 'All of archive' }));
    expect(await within(inspector()).findByRole('button', { name: 'Choose files' }, { timeout: 5000 })).toBeInTheDocument();
    expect(await within(inspector()).findByRole('combobox', { name: 'Visibility' })).toBeInTheDocument();
  });

  it('a list the proposal offers, picked in Advanced, is Simple’s list edit: the folder stays our suggestion', async () => {
    const { saved } = bhc006();
    await open();
    fireEvent.click(within(inspector()).getByRole('button', { name: 'Lists' }));
    const csv = await within(inspector()).findByRole('listitem', { name: CSV.file }, { timeout: 5000 });
    fireEvent.click(within(csv).getByRole('button', { name: 'Use this list' }));
    await waitFor(() => {
      expect(saved).toHaveLength(1);
    }, { timeout: 5000 });
    expect(saved[0]?.['edits']).toEqual([{ edit: 'list', scope: ARTIFACTS, list: CSV }]);
    expect(saved[0]?.['structure']).toEqual(PROPOSAL['structure']);
  });

  it('the mode switch keeps the node picked', async () => {
    bhc006();
    const tree = await open();
    fireEvent.click(within(tree).getByRole('option', { name: 'Documents (D)' }));
    const simple = within(screen.getByRole('group', { name: 'Mode' })).getByRole('link', { name: 'Simple' });
    await waitFor(() => {
      expect(simple).toHaveAttribute('href', `#/import/1/simple/structure/${encodeURIComponent(JSON.stringify({ node: 'series:Documents (D)' }))}`);
    });
  });
});
