/**
 * Every import screen on a plan of several series: the BHC006 replica's
 * proposal (`fixtures/import/proposal-bhc006.json`, six series), which a
 * plan holds after the first click on the Structure screen. No screen fails
 * on it. Advanced's Structure shows every folder in its tree; what a
 * folder's sections show and edit is that series, the address keeps the
 * folder picked, and Advanced's old Files, Lists and Fields addresses land
 * on it.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { Component } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { ADVANCED_SCREENS, SIMPLE_SCREENS } from '../src/pages/import/screens';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';

const SECTION_FIXTURES: Readonly<Record<string, string>> = {
  anomalies: 'preview-anomalies.json',
  check: 'preview-check.json',
  columnMatches: 'preview-column-matches.json',
  entries: 'preview-entries.json',
  flags: 'preview-flags.json',
  histogram: 'preview-histogram.json',
  identical: 'preview-identical.json',
  listCandidates: 'preview-list-candidates.json',
  nameWords: 'preview-name-words.json',
  records: 'preview-records.json',
  shapes: 'preview-shapes.json',
  sheets: 'preview-sheets.json',
  unpaired: 'preview-unpaired.json',
  values: 'preview-values.json',
};

const importFixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);

const PROPOSAL = importFixture('proposal-bhc006.json');
const STRUCTURE = asRecord(PROPOSAL['structure'], []);
const LEVELS: readonly Record<string, unknown>[] = Array.isArray(STRUCTURE['levels']) ? STRUCTURE['levels'].filter(isRecord) : [];
const FOLDERS = LEVELS.map((level) => String(level['id']));

/** The draft whose plan holds the proposal's structure; every PUT body's plan, in order. */
const several = (): { readonly saved: Record<string, unknown>[] } => {
  const detail = importFixture('import-draft.json');
  let plan: Record<string, unknown> = { ...asRecord(detail['plan'], []), structure: STRUCTURE };
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
        const asked = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
        const include = Array.isArray(asked['include']) ? asked['include'].map(String) : [];
        const answer: Record<string, unknown> = { ...importFixture('preview-check.json') };
        delete answer['check'];
        for (const section of include) {
          const name = SECTION_FIXTURES[section];
          answer[section] = section === 'proposal' ? PROPOSAL : name === undefined ? undefined : importFixture(name)[section];
        }
        return jsonResponse(answer);
      },
      [`${PATH}/template.csv`]: () => new Response('Identifier\n', { headers: { 'Content-Type': 'text/csv' } }),
      '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
      '/api/item_sets': () => listResponse(loadFixture('item-sets-page.json'), 2),
      '/api/resource_templates': () => listResponse(loadFixture('resource-templates-page.json'), 2),
      '/api/sites': () => listResponse(loadFixture('sites-page.json'), 1),
      '/api/users': () => listResponse(loadFixture('users-page.json'), 1),
    }),
  );
  return { saved };
};

class Caught extends Component<{ readonly children: ReactNode }, { readonly failed: boolean }> {
  override state = { failed: false };

  static getDerivedStateFromError(): { readonly failed: boolean } {
    return { failed: true };
  }

  override render(): ReactNode {
    return this.state.failed ? <p>This page failed.</p> : this.props.children;
  }
}

const at = (...rest: readonly string[]): ReactElement => (
  <Caught>
    <ImportPage param="1" rest={['1', ...rest]} />
  </Caught>
);

/** Waits until the screen's chunk and every count it asked for have come in. */
const settled = async (): Promise<void> => {
  await waitFor(
    () => {
      expect(screen.queryByText('Loading the import...')).toBeNull();
      expect(document.querySelector('[aria-busy="true"], .import-slow')).toBeNull();
    },
    { timeout: 5000 },
  );
};

/** The folder the tree has selected. */
const selected = (): string | null | undefined =>
  within(screen.getByRole('listbox', { name: 'Your archive' }))
    .getAllByRole('option')
    .find((option) => option.getAttribute('aria-selected') === 'true')
    ?.querySelector('.guided-row-name')?.textContent;


afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('a plan of several series', () => {
  it.each([...SIMPLE_SCREENS.map((one) => ['simple', one] as const), ...ADVANCED_SCREENS.map((one) => ['advanced', one] as const)])(
    'opens %s %s without the notice that the page failed',
    async (mode, place) => {
      const failed = vi.spyOn(console, 'error').mockImplementation(() => undefined);
      several();
      render(at(mode, place));
      await screen.findByRole('heading', { level: 3 }, { timeout: 5000 });
      await settled();
      expect(screen.queryByText('This page failed.')).toBeNull();
      expect(failed.mock.calls.flat().filter((logged) => logged instanceof Error)).toEqual([]);
      failed.mockRestore();
    },
    15_000,
  );

  it('lands the marked example’s address on Structure, where each folder is marked', async () => {
    several();
    render(at('simple', 'example'));
    expect(await screen.findByRole('heading', { name: 'How your archive becomes items and sets' }, { timeout: 5000 })).toBeInTheDocument();
  });

  it.each([
    ['files', 'Items'],
    ['lists', 'Lists'],
    ['fields', 'Fields'],
  ])('lands Advanced’s old %s address on Structure, at the first folder, its section %s open', async (place, section) => {
    several();
    render(at('advanced', place));
    expect(await screen.findByRole('heading', { name: 'Structure: each folder with its rules' }, { timeout: 5000 })).toBeInTheDocument();
    const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const names = within(tree).getAllByRole('option').map((option) => option.querySelector('.guided-row-name')?.textContent);
    expect(names.filter((name) => FOLDERS.includes(name ?? ''))).toEqual(FOLDERS);
    expect(selected()).toBe(FOLDERS[0]);
    expect(screen.getByRole('button', { name: section })).toHaveAttribute('aria-expanded', 'true');
  });

  it('an edit on the second folder changes that series alone', async () => {
    const { saved } = several();
    render(at('advanced', 'fields'));
    fireEvent.click(await screen.findByRole('option', { name: FOLDERS[1] ?? '' }, { timeout: 5000 }));
    await waitFor(() => {
      expect(selected()).toBe(FOLDERS[1]);
    });
    fireEvent.change(await screen.findByRole('combobox', { name: 'Add a rule' }), { target: { value: 'visibility' } });
    expect(await screen.findByRole('article', { name: 'Visibility' })).toBeInTheDocument();
    await waitFor(() => {
      expect(saved).toHaveLength(1);
    }, { timeout: 3000 });
    const structure = asRecord(saved[0]?.['structure'], []);
    const levels = Array.isArray(structure['levels']) ? structure['levels'] : [];
    const second = asRecord(levels[1], []);
    expect(asRecord(asRecord(second['slots'], [])['item'], [])['visibility']).toEqual({ sources: [], combine: 'first' });
    expect({ ...second, slots: undefined }).toEqual({ ...LEVELS[1], slots: undefined });
    expect(levels.filter((_, index) => index !== 1)).toEqual(LEVELS.filter((_, index) => index !== 1));
    expect(structure['freeSets']).toEqual(STRUCTURE['freeSets']);
  });

  it('keeps the picked folder in the address, so a reload opens on it', async () => {
    several();
    render(at('advanced', 'structure'));
    fireEvent.click(await screen.findByRole('option', { name: FOLDERS[2] ?? '' }, { timeout: 5000 }));
    await waitFor(() => {
      expect(window.location.hash.startsWith('#/import/1/advanced/structure/')).toBe(true);
    });
    const kept = window.location.hash;
    cleanup();

    several();
    window.location.hash = kept;
    render(at(...kept.split('/').slice(3)));
    await waitFor(() => {
      expect(selected()).toBe(FOLDERS[2]);
    }, { timeout: 5000 });
    fireEvent.click(screen.getByRole('button', { name: 'Items' }));
    const pattern = asRecord(LEVELS[2]?.['grouping'], [])['pattern'];
    expect(await screen.findByLabelText('Regex')).toHaveValue(typeof pattern === 'string' ? pattern : '');
  });
});
