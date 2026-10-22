/**
 * Every import screen on a plan of several series: the BHC006 replica's
 * proposal (`fixtures/import/proposal-bhc006.json`, six series), which a
 * plan holds after the first click on the Structure screen. No screen fails
 * on it. Advanced's screens of one series (Files, Lists, Fields) offer a
 * picker of the plan's folders; what they show and edit is the picked
 * series, and the address keeps the pick.
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

const picker = (): HTMLElement => screen.getByRole('combobox', { name: 'Folder' });

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

  it.each(['files', 'lists', 'fields'])('offers every folder on Advanced %s, the first one picked', async (place) => {
    several();
    render(at('advanced', place));
    const folder = await screen.findByRole('combobox', { name: 'Folder' }, { timeout: 5000 });
    expect(within(folder).getAllByRole('option').map((option) => option.textContent)).toEqual(FOLDERS);
    expect(folder).toHaveValue(FOLDERS[0]);
  });

  it('an edit with the second folder picked changes that series alone', async () => {
    const { saved } = several();
    render(at('advanced', 'fields'));
    fireEvent.change(await screen.findByRole('combobox', { name: 'Folder' }, { timeout: 5000 }), { target: { value: FOLDERS[1] } });
    await waitFor(() => {
      expect(picker()).toHaveValue(FOLDERS[1]);
    });
    fireEvent.change(screen.getByRole('combobox', { name: 'Add a rule' }), { target: { value: 'visibility' } });
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
    render(at('advanced', 'files'));
    fireEvent.change(await screen.findByRole('combobox', { name: 'Folder' }, { timeout: 5000 }), { target: { value: FOLDERS[2] } });
    await waitFor(() => {
      expect(window.location.hash.startsWith('#/import/1/advanced/files/')).toBe(true);
    });
    const kept = window.location.hash;
    cleanup();

    several();
    window.location.hash = kept;
    render(at(...kept.split('/').slice(3)));
    await waitFor(() => {
      expect(picker()).toHaveValue(FOLDERS[2]);
    }, { timeout: 5000 });
    const pattern = asRecord(LEVELS[2]?.['grouping'], [])['pattern'];
    expect(screen.getByLabelText('Regex')).toHaveValue(typeof pattern === 'string' ? pattern : '');
  });
});
