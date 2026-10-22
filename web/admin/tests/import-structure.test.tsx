/**
 * The Structure screen over BHC006's proposals as the engine counts them:
 * `fixtures/import/proposal-bhc006*.json` are the BHC006 replica's proposals
 * with no edit, after leaving out Extras, and after that and PN's rows
 * describing files, written and checked by omeka-api's
 * `bhc006_tests::the_admins_recorded_proposals_are_the_replicas`. The screen
 * runs through the real page, client and proposal parser. Every number
 * below is the engine's (design section 0 and the B6 grain counts); none is
 * the design's own sentence.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { asRecord, isRecord } from '../src/api/parse';
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
    expect(screen.getByRole('button', { name: 'Use this structure' })).toHaveProperty('disabled', false);
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
  });

  it('two clicks make BHC006’s pinned edits, and a click back removes one', async () => {
    const server = structureServer();
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });

    // Click 1: leave out Extras. The answer turns on at once; the proposal with the edit comes first, then one save.
    const leaveOut = within(select('Extras')).getByRole('button', { name: /^Leave out/ });
    fireEvent.click(leaveOut);
    expect(leaveOut.getAttribute('aria-pressed')).toBe('true');
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    });
    expect(server.asked[server.asked.length - 1]).toEqual([LEAVE_OUT_EXTRAS]);
    expect(server.saved[0]?.['edits']).toEqual([LEAVE_OUT_EXTRAS]);
    expect(canonical(server.saved[0]?.['structure'])).toBe(canonical(asRecord(loadFixture('import/proposal-bhc006-extras.json'), [])['structure']));
    expect(await screen.findByText('In use. Your changes: 1.')).toBeTruthy();
    expect(countLine()).toEqual(['Items to add 570', 'Sets 5', 'Not imported 2 folders', 'Your changes 1']);
    expect(strip()).toEqual([['Extras · Import this folder: Leave out', '733 → 570 items']]);
    expect(rowFacts('Extras')).toBe('293 files · you left it out · changed');

    // Click 2: PN's rows describe files. The person's answer is pressed, the suggestion keeps its ✓.
    fireEvent.click(within(select('Photo Narratives (PN)')).getByRole('button', { name: /^A file/ }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(2);
    });
    expect(server.saved[1]?.['edits']).toEqual([LEAVE_OUT_EXTRAS, PN_ROWS_ARE_FILES]);
    await waitFor(() => {
      expect(countLine()[0]).toBe('Items to add 544');
    });
    expect(answers(inspectorOf(), 'Each row describes')).toEqual([
      ['A file189 items', true],
      ['An item✓ our suggestion215 items', false],
    ]);
    expect(strip()).toEqual([
      ['Extras · Import this folder: Leave out', '733 → 570 items'],
      ['Photo Narratives (PN) · Each row describes: A file', '570 → 544 items'],
    ]);

    // A click back on the suggestion deletes that edit: the changes do not grow.
    fireEvent.click(within(inspectorOf()).getByRole('button', { name: /^An item/ }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(3);
    });
    expect(server.saved[2]?.['edits']).toEqual([LEAVE_OUT_EXTRAS]);
    await waitFor(() => {
      expect(countLine()).toEqual(['Items to add 570', 'Sets 5', 'Not imported 2 folders', 'Your changes 1']);
    });
    expect(strip()).toEqual([['Extras · Import this folder: Leave out', '733 → 570 items']]);
  });

  it('takes a click back when its counts do not come, and saves nothing', async () => {
    const server = structureServer({ failPreviewWithEdits: true });
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const extras = select('Extras');
    fireEvent.click(within(extras).getByRole('button', { name: /^Leave out/ }));
    expect(await within(extras).findByRole('alert')).toHaveProperty('textContent', 'We could not count. Try again.');
    expect(answers(extras, 'Import this folder').map(([, pressed]) => pressed)).toEqual([true, false]);
    expect(server.saved).toEqual([]);
  });
});

const LEAVE_OUT_EXTRAS = { edit: 'leaveOut', scope: { source: 's1', folder: 'Extras' } };
const PN_ROWS_ARE_FILES = { edit: 'grain', list: { source: 's1', file: 'BHC006_GMT_PN.xlsx' }, describes: 'media' };

/** JSON with sorted keys, so two plans compare by what they hold. */
const canonical = (value: unknown): string =>
  JSON.stringify(value, (_key, member: unknown) =>
    isRecord(member) ? Object.fromEntries(Object.entries(member).sort(([a], [b]) => a.localeCompare(b))) : member,
  );

/** The engine's recorded proposal for each list of edits the screen can send. */
const RECORDED: Readonly<Record<string, string>> = {
  [canonical([])]: 'import/proposal-bhc006.json',
  [canonical([LEAVE_OUT_EXTRAS])]: 'import/proposal-bhc006-extras.json',
  [canonical([LEAVE_OUT_EXTRAS, PN_ROWS_ARE_FILES])]: 'import/proposal-bhc006-two.json',
};

/**
 * The import routes over one stored plan. A preview answers the recorded
 * proposal of the edits of the plan it is sent, else of the stored plan's,
 * and says the proposal matches the plan when their structures are equal,
 * as the server does. A save stores the plan and answers the next revision.
 */
const structureServer = ({ failPreviewWithEdits = false } = {}): { readonly saved: Record<string, unknown>[]; readonly asked: unknown[] } => {
  const detail = asRecord(loadFixture('import/import-draft.json'), []);
  let stored = { ...detail };
  const saved: Record<string, unknown>[] = [];
  const asked: unknown[] = [];
  installFetch(
    route({
      [PATH]: (_url, init) => {
        if (init?.method === 'PUT') {
          const body = asRecord(JSON.parse(typeof init.body === 'string' ? init.body : '{}'), []);
          const plan = asRecord(body['plan'], ['plan']);
          saved.push(plan);
          stored = { ...stored, plan, revision: Number(stored['revision']) + 1 };
        }
        return jsonResponse(stored);
      },
      [`${PATH}/preview`]: (_url, init) => {
        const body = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), []);
        const plan = asRecord(body['plan'] ?? stored['plan'], ['plan']);
        const edits = plan['edits'] ?? [];
        if (body['plan'] !== undefined) {
          asked.push(edits);
          if (failPreviewWithEdits) {
            return jsonResponse({ errors: { error: 'the count failed' } }, { status: 500 });
          }
        }
        const name = RECORDED[canonical(edits)];
        if (name === undefined) {
          throw new Error(`no recorded proposal for ${canonical(edits)}`);
        }
        const proposal = asRecord(loadFixture(name), []);
        const matchesPlan = canonical(proposal['structure']) === canonical(plan['structure']);
        return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, proposal: { ...proposal, matchesPlan } });
      },
    }),
  );
  return { saved, asked };
};

const countLine = (): readonly (string | null)[] =>
  Array.from(document.querySelectorAll('.structure-status .guided-facts > span'), (span) => span.textContent);

/** Each line of the changes strip: the change, and its effect or why it does not apply. */
const strip = (): readonly (readonly (string | null)[])[] =>
  within(screen.getByRole('region', { name: 'Your changes' }))
    .getAllByRole('listitem')
    .map((item) => Array.from(item.querySelectorAll('span'), (span) => span.textContent));

const rowFacts = (name: string): string | null | undefined =>
  screen.getByRole('option', { name }).querySelector('.guided-row-facts')?.textContent;
