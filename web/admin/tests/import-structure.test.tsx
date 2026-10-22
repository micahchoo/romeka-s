/**
 * The Structure screen over BHC006's proposal as the engine counts it:
 * `fixtures/import/proposal-bhc006.json` is the BHC006 replica's proposal
 * with no edit, written and checked by omeka-api's
 * `bhc006_tests::the_admins_recorded_proposal_is_the_replicas`. The screen
 * runs through the real page, client and proposal parser. Every number
 * below is the engine's (design section 0 and the B6 grain counts); none is
 * the design's own sentence.
 */
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, route } from './stub';

const PATH = '/api/romeka/imports/1';
/** The replica's walk: 2,189 files, 6 of them lists. */
const SOURCES = [
  {
    source: 's1',
    scanned: '2026-10-21T09:00:00+00:00',
    files: 2189,
    sheetFiles: ['Artifacts/BHC006_GMT_AF.csv', 'Artifacts/BHC006_GMT_AF.ods', 'BHC006_CC_SECOND_AF.xlsx', 'BHC006_GMT_AF.xlsx', 'BHC006_GMT_D.xlsx', 'BHC006_GMT_PN.xlsx'],
    error: null,
  },
];

const open = (): void => {
  const proposal = loadFixture('import/proposal-bhc006.json');
  installFetch(
    route({
      [PATH]: () => jsonResponse(loadFixture('import/import-draft.json')),
      [`${PATH}/preview`]: (_url, init) => {
        const asked = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
        expect(asked['include']).toEqual(['proposal']);
        return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, proposal });
      },
    }),
  );
  render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
};

/** The answers of one question, by the question's name: each button's text, whether it is the pressed one. */
const answers = (inspector: HTMLElement, question: string): readonly (readonly [string, boolean])[] =>
  Array.from(within(inspector).getByRole('group', { name: question }).querySelectorAll('button'), (button) => [
    button.textContent,
    button.getAttribute('aria-pressed') === 'true',
  ] as const);

const inspectorOf = (): HTMLElement => {
  const inspector = screen.getByRole('listbox', { name: 'Your archive' }).parentElement?.querySelector('.guided-inspector');
  if (!(inspector instanceof HTMLElement)) {
    throw new Error('no inspector');
  }
  return inspector;
};

const select = (name: string): HTMLElement => {
  fireEvent.click(screen.getByRole('option', { name }));
  return inspectorOf();
};

afterEach(() => {
  cleanup();
  window.location.hash = '';
});

describe('the Structure screen', () => {
  it('shows BHC006’s draft: six series with their items and rivals', async () => {
    open();
    const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });

    expect(screen.getByText('Our suggestion. Nothing is used until you confirm.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Use this structure' })).toHaveProperty('disabled', true);
    const counts = (label: string): string | null => screen.getByText(label, { selector: '.guided-facts > span' }).textContent;
    expect(counts('Items to add')).toBe('Items to add 733');
    expect(counts('Sets')).toBe('Sets 6');
    expect(counts('Not imported')).toBe('Not imported 1 folder');
    expect(counts('Your changes')).toBe('Your changes 0');

    const lines = within(tree)
      .getAllByRole('option')
      .map((option) => [option.querySelector('.guided-row-name')?.textContent, option.querySelector('.guided-row-facts')?.textContent]);
    expect(lines).toEqual([
      ['All of archive', '2,189 files · 6 lists'],
      ['Every item', 'Offered: Citizen Curators, M. Curator'],
      ['Artifacts', '318 items · 696 pictures · list 680/690'],
      ['Audio Interviews (AI)', '3 items · 3 sounds · no list'],
      ['Documents (D)', '15 items · 15 pictures · list 0/15'],
      ['Extras', '163 items · 126 pictures, 37 films · no list'],
      ['Photo Narratives (PN)', '215 items · 189 pictures · list 189/215'],
      ['Video (V)', '19 items · 13 films, 6 sounds · no list'],
      ['tiny-iiif', '147 files · none we can show'],
      ['Artifacts/BHC006_GMT_AF.ods', 'does not read'],
      ['BHC006_CC_SECOND_AF.xlsx', '0 of 29 rows name a file'],
    ]);
    expect(within(tree).getByRole('group', { name: 'Not imported' })).toBeTruthy();
    expect(within(tree).getByRole('group', { name: 'Lists not used' })).toBeTruthy();

    // The first series opens: each answer with what it makes, the suggested one pressed and marked.
    const artifacts = inspectorOf();
    expect(within(artifacts).getByRole('heading', { name: 'Artifacts' })).toBeTruthy();
    expect(answers(artifacts, 'Items')).toEqual([
      ['Same name up to part 5✓ our suggestion318 items', true],
      ['Each name an item696 items', false],
    ]);
    expect(answers(artifacts, 'List')).toEqual([
      ['BHC006_GMT_AF.xlsx✓ our suggestion680 of 690 rows name a file', true],
      ['Artifacts/BHC006_GMT_AF.csv5 of 5 rows name a file', false],
      ['No list', false],
    ]);
    expect(answers(artifacts, 'Each row describes')).toEqual([
      ['A file✓ our suggestion318 items', true],
      ['An item328 items', false],
    ]);
    expect(answers(artifacts, 'Set')).toEqual([
      ['Artifacts✓ our suggestionfolder name · 318 items', true],
      ['Artifactcolumn Item Type · 690 of 690 rows · 313 items', false],
      ['No set', false],
    ]);
    expect(within(artifacts).getByText('10 rows name no file.')).toBeTruthy();

    const extras = select('Extras');
    expect(answers(extras, 'Items')).toEqual([
      ['Each name an item✓ our suggestion163 items', true],
      ['Same name up to part 515 items', false],
    ]);
    expect(answers(extras, 'Import this folder')).toEqual([
      ['Import✓ our suggestion293 files · 126 pictures, 37 films', true],
      ['Leave out163 items not added', false],
    ]);
    expect(within(extras).getByText('No list names these files.')).toBeTruthy();

    const narratives = select('Photo Narratives (PN)');
    expect(answers(narratives, 'Each row describes')).toEqual([
      ['An item✓ our suggestion215 items', true],
      ['A file189 items', false],
    ]);
    expect(within(narratives).getByText('26 rows name no file.')).toBeTruthy();
    expect(within(narratives).getByText('Case, zeros and separators ignored. Exactly as written: 86.')).toBeTruthy();

    const documents = select('Documents (D)');
    expect(answers(documents, 'Items')).toEqual([
      ['Each name an item✓ our suggestion15 items', true],
      ['Same name up to part 52 items', false],
    ]);
    expect(within(documents).queryByRole('group', { name: 'Each row describes' })).toBeNull();
    expect(within(documents).getByText(/^Pair by the last 2 parts of the name · 15 of 15 rows find a file$/)).toBeTruthy();
    expect(within(documents).getByText(/^One set per part 5 of the name: 20170616 \(11 items\) · 20171013 \(4 items\)$/)).toBeTruthy();

    // Nothing on the screen changes the plan yet.
    for (const node of ['Artifacts', 'Extras', 'tiny-iiif']) {
      const inspector = select(node);
      expect(within(inspector).getAllByRole('button').every((button) => button.hasAttribute('disabled'))).toBe(true);
    }
  });
});
