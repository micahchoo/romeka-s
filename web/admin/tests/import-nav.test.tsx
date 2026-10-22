/**
 * The Import link sits in the Content group after Media for whoever may read
 * the import routes (`GET /api/romeka/archive` answers), with or without an
 * archive root, and is hidden from whoever the route refuses. Inside one
 * import, Simple's steps are Structure, Check and Add; the old steps' addresses
 * land on Structure's questions once the plan has several series.
 */
import { act, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { asRecord } from '../src/api/parse';
import { Shell } from '../src/app/shell';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, route, urlsFor } from './stub';

const archive = (answer: 'roots' | 'none' | 'refused') =>
  installFetch(
    route({
      '/api/romeka/archive': () =>
        answer === 'refused'
          ? jsonResponse({ errors: { error: 'Permission denied.' } }, { status: 403 })
          : jsonResponse({ roots: answer === 'roots' ? ['archive'] : [], root: answer === 'roots' ? 'archive' : null, path: '', entries: [] }),
      '/api/me': () => jsonResponse({ errors: { error: 'signed out' } }, { status: 401 }),
    }),
  );

const contentLinks = (): readonly string[] => {
  const nav = screen.getByRole('navigation', { name: 'Admin pages' });
  const group = within(nav).getByRole('heading', { name: 'Content' }).closest('section');
  return group === null ? [] : within(group).getAllByRole('link').map((link) => link.textContent);
};

afterEach(() => {
  window.location.hash = '';
});

describe('Import nav link', () => {
  it.each(['roots', 'none'] as const)('follows Media for an admin, archive roots: %s', async (answer) => {
    window.location.hash = '#/items';
    archive(answer);
    render(<Shell />);
    expect(await screen.findByRole('link', { name: 'Import' })).toHaveAttribute('href', '#/import');
    expect(contentLinks()).toEqual(['Admin dashboard', 'Items', 'Item sets', 'Media', 'Import', 'Sites']);
  });

  it('is hidden from whoever the import routes refuse', async () => {
    window.location.hash = '#/items';
    const fetchMock = archive('refused');
    render(<Shell />);
    await screen.findByRole('navigation', { name: 'Admin pages' });
    await waitFor(() => {
      expect(urlsFor(fetchMock, '/api/romeka/archive')).toHaveLength(1);
    });
    // Let the probe's answer settle before reading the nav.
    await act(async () => {});
    expect(contentLinks()).toEqual(['Admin dashboard', 'Items', 'Item sets', 'Media', 'Sites']);
  });
});

/** The recorded answer of each preview section: the server's wire fixtures, and the BHC006 replica's proposal. */
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

/**
 * A draft over the BHC006 replica's files whose plan has `structure`: the
 * archive as found (one series), or the six series of its proposal. Each
 * preview answers the sections it asks for from the recorded answers.
 */
const draftOf = (structure: 'asFound' | 'proposed'): void => {
  const detail = importFixture('import-draft.json');
  const plan = asRecord(detail['plan'], []);
  const proposal = importFixture('proposal-bhc006.json');
  const stored = structure === 'asFound' ? detail : { ...detail, plan: { ...plan, structure: proposal['structure'] } };
  installFetch(
    route({
      '/api/romeka/imports/1': () => jsonResponse(stored),
      '/api/romeka/imports/1/preview': (_url, init) => {
        const asked = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
        const include = Array.isArray(asked['include']) ? asked['include'].map(String) : [];
        const answer: Record<string, unknown> = { ...importFixture('preview-check.json') };
        delete answer['check'];
        for (const section of include) {
          const name = SECTION_FIXTURES[section];
          answer[section] = section === 'proposal' ? proposal : name === undefined ? undefined : importFixture(name)[section];
        }
        return jsonResponse(answer);
      },
    }),
  );
};

const stepLinks = (): readonly (readonly [string | null, string | null])[] =>
  within(screen.getByRole('list', { name: 'Steps' }))
    .getAllByRole('link')
    .map((link) => [link.textContent, link.getAttribute('aria-current')] as const);

describe('Simple’s steps', () => {
  it('Simple has three steps: Structure, Check, Add', async () => {
    draftOf('asFound');
    const { unmount } = render(<ImportPage param="1" rest={['1']} />);
    // A draft with files opens on Structure.
    expect(await screen.findByRole('heading', { name: 'How your archive becomes items and sets' })).toBeInTheDocument();
    expect(stepLinks()).toEqual([
      ['Structure', 'step'],
      ['Check', null],
      ['Add', null],
    ]);
    expect(screen.getByRole('link', { name: 'Next' })).toHaveAttribute('href', '#/import/1/simple/check');
    expect(screen.queryByRole('link', { name: 'Back' })).not.toBeInTheDocument();
    unmount();

    // An old step still opens for a plan of one series, which it edits, under Structure.
    render(<ImportPage param="1" rest={['1', 'simple', 'where']} />);
    expect(await screen.findByRole('heading', { name: 'Where should they go in Omeka?' })).toBeInTheDocument();
    expect(stepLinks()[0]).toEqual(['Structure', 'step']);
    expect(screen.getByRole('link', { name: 'Back' })).toHaveAttribute('href', '#/import/1/simple/structure');
  });

  it.each([
    ['found', 'Items', 'Same name up to part 5'],
    ['list', 'List', 'BHC006_GMT_AF.xlsx'],
    ['where', 'Set', 'Artifacts'],
  ])('lands the old address %s of a plan of several series on Structure, at the question %s', async (old, question, answer) => {
    draftOf('proposed');
    render(<ImportPage param="1" rest={['1', 'simple', old]} />);
    expect(await screen.findByRole('heading', { name: 'How your archive becomes items and sets' })).toBeInTheDocument();
    const group = await screen.findByRole('group', { name: question }, { timeout: 5000 });
    expect(stepLinks()[0]).toEqual(['Structure', 'step']);
    expect(document.activeElement?.closest('[role="group"]')).toBe(group);
    expect(document.activeElement?.textContent).toMatch(new RegExp(`^${answer}`));
  });

  it.each([
    ['structure', 'How your archive becomes items and sets', 'listbox', 'Your archive'],
    ['check', 'Check', 'listbox', 'Questions'],
    ['add', 'Add them', 'button', /^Add \d+ items to Omeka$/],
  ])('opens %s on a plan of several series, without the notice that the page failed', async (place, title, role, name) => {
    const failed = vi.spyOn(console, 'error');
    draftOf('proposed');
    render(<ImportPage param="1" rest={['1', 'simple', place]} />);
    expect(await screen.findByRole('heading', { name: title })).toBeInTheDocument();
    expect(await screen.findByRole(role, { name }, { timeout: 5000 })).toBeInTheDocument();
    expect(failed.mock.calls.flat().filter((logged) => logged instanceof Error)).toEqual([]);
    failed.mockRestore();
  });
});

const indexLinks = (): readonly (readonly [string | null, string | null])[] =>
  within(screen.getByRole('navigation', { name: 'Settings' }))
    .getAllByRole('link')
    .map((link) => [link.textContent, link.getAttribute('aria-current')] as const);

describe('Advanced’s steps', () => {
  it('Advanced has six steps: Structure, Problems, Existing items, Changes, Items, Run', async () => {
    draftOf('proposed');
    render(<ImportPage param="1" rest={['1', 'advanced', 'structure']} />);
    expect(await screen.findByRole('heading', { name: 'Structure: each folder with its rules' })).toBeInTheDocument();
    expect(indexLinks()).toEqual([
      ['Structure', 'page'],
      ['Problems', null],
      ['Existing items', null],
      ['Changes', null],
      ['Items', null],
      ['Run', null],
    ]);
    expect(await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 })).toBeInTheDocument();
  });
});
