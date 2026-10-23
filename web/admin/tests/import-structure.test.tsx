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
import { installFetch, jsonResponse, listResponse, route } from './stub';

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

  it('a marked example writes the series and one own edit', async () => {
    const server = structureServer();
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });

    // From the Items question: mark the glued letters of one name as any letters. The count before "Use this" is the engine's.
    fireEvent.click(within(select('Artifacts')).getByRole('button', { name: 'Mark an example' }));
    fireEvent.click(await screen.findByRole('radio', { name: 'BHC006_GMT_AF_010a.jpg' }));
    fireEvent.click(within(screen.getByRole('group', { name: /^Parts of/ })).getByRole('button', { name: 'a' }));
    expect(await screen.findByText('Written this way: 328 items.')).toBeTruthy();
    expect(server.asked[server.asked.length - 1]).toEqual([OWN_ARTIFACTS]);
    expect(server.saved).toEqual([]);

    fireEvent.click(screen.getByRole('button', { name: 'Use this' }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    });
    const saved = server.saved[0] ?? {};
    expect(saved['edits']).toEqual([OWN_ARTIFACTS]);
    expect(canonical(saved['structure'])).toBe(canonical(asRecord(loadFixture('import/proposal-bhc006-own.json'), [])['structure']));
    const written = seriesIn(saved, 'Artifacts');
    expect(written['grouping']).toEqual({ pattern: MARKED_PATTERN, caseFold: false });
    expect(written['example']).toEqual({ path: 'Artifacts/BHC006_GMT_AF_010a.jpg', marks: [{ kind: 'letters', text: 'a', sep: '' }] });

    // The folder now says it is written by hand, with the items the engine counts for it, and offers the way back.
    expect(await screen.findByText('In use. Your changes: 1.')).toBeTruthy();
    expect(countLine()).toEqual(['Items to add 743', 'Sets 6', 'Not imported 1 folder', 'Your changes 1']);
    expect(rowFacts('Artifacts')).toBe('328 items · 696 pictures · written by you');
    const artifacts = inspectorOf();
    expect(within(artifacts).getByText('Written by you. We suggest nothing more here.')).toBeTruthy();
    expect(within(artifacts).getByText('Files per item: 55 with 1 file, 227 with 2 files, 19 with 3 files, 25 with 4 to 6 files, 2 with 7 or more files.')).toBeTruthy();
    expect(strip()).toEqual([['Artifacts · By hand: Written by you', '733 → 743 items']]);

    fireEvent.click(within(artifacts).getByRole('button', { name: 'Back to our suggestion' }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(2);
    });
    expect(server.saved[1]?.['edits']).toBeUndefined();
    expect(canonical(server.saved[1]?.['structure'])).toBe(canonical(asRecord(loadFixture('import/proposal-bhc006.json'), [])['structure']));
    await waitFor(() => {
      expect(countLine()[0]).toBe('Items to add 733');
    });
    expect(rowFacts('Artifacts')).toBe('318 items · 696 pictures · list 680/690');
  });

  it('a click on a folder written by hand waits, with the way back beside it', async () => {
    const waiting = asRecord(loadFixture('import/proposal-bhc006-own-waiting.json'), []);
    const server = structureServer({ stored: { edits: [ITEMS_WHOLE, OWN_ARTIFACTS], structure: waiting['structure'] } });
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });

    // The strip keeps each click's effect for the tab, so the hand edit's line may carry one from an earlier test.
    const [click, written] = strip();
    expect(click).toEqual(['Artifacts · Items: Each name an item', 'Waits while you write this folder by hand.']);
    expect(written?.[0]).toBe('Artifacts · By hand: Written by you');
    expect(screen.queryByText('We do not offer this now.')).toBeNull();
    const [waits] = within(screen.getByRole('region', { name: 'Your changes' })).getAllByRole('listitem');
    fireEvent.click(within(waits ?? document.body).getByRole('button', { name: 'Back to our suggestion' }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    });
    expect(server.saved[0]?.['edits']).toEqual([ITEMS_WHOLE]);
    await waitFor(() => {
      expect(strip()).toEqual([['Artifacts · Items: Each name an item']]);
    });
  });

  it('a set level kept around a folder written by hand shows in the tree, and its click is kept', async () => {
    const kept = asRecord(loadFixture('import/proposal-bhc006-own-set.json'), []);
    structureServer({ stored: { edits: [DOCUMENTS_PLACE_4, OWN_DOCUMENTS], structure: kept['structure'] } });
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    const tree = await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const lines = within(tree)
      .getAllByRole('option')
      .map((option) => [option.querySelector('.guided-row-name')?.textContent, option.querySelector('.guided-row-facts')?.textContent]);
    const at = lines.findIndex(([name]) => name === 'Documents (D)');
    expect(lines[at - 1]).toEqual(['One set per value', 'written by you · changed']);
    expect(lines[at]?.[1]).toMatch(/written by you$/);
    expect(within(select('One set per value')).getByText('Kept as you made it, around the folder you wrote.')).toBeTruthy();

    const [split] = strip();
    expect(split).toEqual(['Documents (D) · Set: part 5 of the name', 'Kept as you made it.']);
    expect(screen.queryByText('Waits while you write this folder by hand.')).toBeNull();
  });

  it('a list the person uploaded shows on its folder as kept', async () => {
    const proposal = asRecord(loadFixture('import/proposal-bhc006.json'), []);
    const nodes: readonly unknown[] = Array.isArray(proposal['nodes']) ? proposal['nodes'] : [];
    const kept = [
      { list: 'list-1', file: { source: 'staging', file: 'lists/1.csv' }, rows: { n: 12, of: 15 } },
      { list: 'list-2', file: { source: 'staging', file: 'lists/2.csv' }, rows: null },
    ];
    structureServer({ answer: { ...proposal, nodes: nodes.map((node) => (isRecord(node) && node['level'] === 'Documents (D)' ? { ...node, kept } : node)) } });
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const documents = select('Documents (D)');
    expect(within(documents).getByText('lists/1.csv: your uploaded list, kept. 12 of 15 rows pair.')).toBeTruthy();
    expect(within(documents).getByText('lists/2.csv: your uploaded list, kept. We could not read it.')).toBeTruthy();
  });

  it('Artifacts opens where each column goes, and a choice there survives a click', { timeout: 20_000 }, async () => {
    const server = structureServer({ carry: true });
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });

    // The archive as found: the plan holds no Artifacts yet, so the table reads the list of the structure on screen.
    fireEvent.click(await within(select('Artifacts')).findByRole('button', { name: 'Where each column goes' }));
    const table = await within(inspectorOf()).findByRole('table', {}, { timeout: 5000 });
    expect(within(table).getAllByRole('rowheader').map((cell) => cell.querySelector('.guided-col-name')?.textContent)).toEqual(['Sl No', 'Collection', 'Curator', 'Item Type']);
    const curator = within(table).getByRole('combobox', { name: 'Where the column Curator goes' });
    await within(curator).findByRole('option', { name: 'Description' });
    // Sets are the folder's Set and the archive's own sets; a column is never sent to one.
    expect(within(curator).queryByRole('option', { name: 'The item set' })).toBeNull();
    fireEvent.change(curator, { target: { value: 'field:dcterms:description' } });
    await waitFor(() => {
      expect(server.saved).toHaveLength(1);
    });
    expect(server.saved[0]?.['edits']).toBeUndefined();
    expect(asRecord(seriesIn(server.saved[0] ?? {}, 'Artifacts')['slots'], [])['item']).toEqual({ 'dcterms:description': CURATOR_ON_THE_ITEM });

    // A click on another folder saves the structure the proposer sends back, which carries the column's rule.
    fireEvent.click(within(select('Extras')).getByRole('button', { name: /^Leave out/ }));
    await waitFor(() => {
      expect(server.saved).toHaveLength(2);
    });
    expect(server.saved[1]?.['edits']).toEqual([LEAVE_OUT_EXTRAS]);
    expect(asRecord(seriesIn(server.saved[1] ?? {}, 'Artifacts')['slots'], [])['item']).toEqual({ 'dcterms:description': CURATOR_ON_THE_ITEM });
    expect(await screen.findByText('In use. Your changes: 1.')).toBeTruthy();

    fireEvent.click(within(select('Artifacts')).getByRole('button', { name: 'Where each column goes' }));
    expect(await within(inspectorOf()).findByRole('combobox', { name: 'Where the column Curator goes' }, { timeout: 5000 })).toHaveProperty('value', 'field:dcterms:description');
  });

  it('a folder opens Pair by hand and Every file with itself in the address, and fills in a list once the structure is used', async () => {
    structureServer();
    render(<ImportPage param="1" rest={['1', 'simple', 'structure']} />);
    await screen.findByRole('listbox', { name: 'Your archive' }, { timeout: 5000 });
    const artifacts = select('Artifacts');
    const segment = (value: Record<string, string>): string => encodeURIComponent(JSON.stringify(value));
    expect(await within(artifacts).findByRole('link', { name: 'Pair rows and files by hand' })).toHaveAttribute('href', `#/import/1/simple/pair/${segment({ node: 'series:Artifacts' })}`);
    expect(within(artifacts).getByRole('link', { name: 'Every file' })).toHaveAttribute('href', `#/import/1/simple/all-files/${segment({ node: 'series:Artifacts', folder: 'Artifacts' })}`);

    // A folder with no list has no columns to place and no rows to pair, but a list can be filled in for it.
    const audio = select('Audio Interviews (AI)');
    expect(await within(audio).findByRole('link', { name: 'Every file' })).toBeTruthy();
    expect(within(audio).queryByRole('button', { name: 'Where each column goes' })).toBeNull();
    expect(within(audio).queryByRole('link', { name: 'Pair rows and files by hand' })).toBeNull();
    fireEvent.click(within(audio).getByRole('button', { name: 'Fill in a list yourself' }));
    expect(within(audio).getByText('Use this structure first, then fill in a list here.')).toBeTruthy();
  });
});

const OWN_ARTIFACTS = { edit: 'own', scope: { source: 's1', folder: 'Artifacts' } };
const OWN_DOCUMENTS = { edit: 'own', scope: { source: 's1', folder: 'Documents (D)' } };
const DOCUMENTS_PLACE_4 = { edit: 'addSet', from: { from: 'namePlace', scope: { source: 's1', folder: 'Documents (D)' }, place: 4 } };
const ITEMS_WHOLE = { edit: 'items', scope: { source: 's1', folder: 'Artifacts' }, cut: { cut: 'whole' } };
/** The pattern a mark of any letters glued after the number writes (`bhc006_tests::MARKED_PATTERN`). */
const MARKED_PATTERN = String.raw`^(?:[^/]*/)*(?<identifier>[^/]*?\d|[^/]*?[^/.][\p{L}\p{M}]*)(?:(?<part>[\p{L}\p{M}]+))?(?:\.[^./]*)?$`;
const LEAVE_OUT_EXTRAS = { edit: 'leaveOut', scope: { source: 's1', folder: 'Extras' } };
const CURATOR_ON_THE_ITEM = { sources: [{ from: 'column', sheet: 'BHC006_GMT_AF.xlsx', column: { index: 3, header: 'Curator' } }], combine: 'first' };

/** The Artifacts list as the replica reads it: its five columns and one row. */
const AF_SHEET = {
  id: 'BHC006_GMT_AF.xlsx',
  source: 's1',
  file: 'BHC006_GMT_AF.xlsx',
  found: true,
  error: null,
  guess: null,
  lines: ['Sl No,Identifier,Collection,Curator,Item Type', '1,BHC006_GMT_AF_001,Citizen Curators,M. Curator,Artifact'],
  read: {
    blankLines: 0,
    cellsCount: 5,
    cellsSuspect: 0,
    cellsWithSpaces: 0,
    columns: ['Sl No', 'Identifier', 'Collection', 'Curator', 'Item Type'].map((header, index) => ({ header, index, mappable: true })),
    delimiter: ',',
    delimiterGuessed: true,
    encoding: { confidence: 1, encoding: 'utf-8' },
    hash: 'b1ebb0f938343aa006efe0de3baeb6dcd5ea182c430e26c5efb249830737c18d',
    headerLine: 1,
    notes: [],
    rows: 1,
    workbook: null,
  },
};

const records = (value: unknown): Record<string, unknown>[] => (Array.isArray(value) ? value.filter(isRecord) : []);

const sameFile = (a: Record<string, unknown>, b: Record<string, unknown>): boolean => a['source'] === b['source'] && a['file'] === b['file'];

/**
 * The proposal with what the proposer copies from the plan it is sent into
 * each series it proposes (`preview/proposal.rs`, `Plan::of`): each list's
 * read and column options, by its file, and the slot rules but item sets of
 * the plan's series that holds one of its lists, else of the same id.
 */
const carried = (proposal: Record<string, unknown>, sent: Record<string, unknown>): Record<string, unknown> => {
  const plan = records(asRecord(sent['structure'], [])['levels']);
  const structure = asRecord(proposal['structure'], []);
  const levels = records(structure['levels']).map((level) => {
    const lists = records(level['lists']);
    const holder = plan.find((one) => records(one['lists']).some((held) => lists.some((list) => sameFile(list, held)))) ?? plan.find((one) => one['id'] === level['id']);
    if (holder === undefined) {
      return level;
    }
    const rules = (kind: string): Record<string, unknown> => ({
      ...asRecord(asRecord(level['slots'], [])[kind], []),
      ...Object.fromEntries(Object.entries(asRecord(asRecord(holder['slots'], [])[kind], [])).filter(([slot]) => slot !== 'sets')),
    });
    return {
      ...level,
      lists: lists.map((list) => {
        const held = records(holder['lists']).find((one) => sameFile(one, list));
        return held === undefined ? list : { ...list, read: held['read'], columns: held['columns'] };
      }),
      slots: { item: rules('item'), media: rules('media'), itemSet: rules('itemSet') },
    };
  });
  return { ...proposal, structure: { ...structure, levels } };
};
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
  [canonical([OWN_ARTIFACTS])]: 'import/proposal-bhc006-own.json',
  [canonical([ITEMS_WHOLE])]: 'import/proposal-bhc006-whole.json',
  [canonical([ITEMS_WHOLE, OWN_ARTIFACTS])]: 'import/proposal-bhc006-own-waiting.json',
  [canonical([DOCUMENTS_PLACE_4, OWN_DOCUMENTS])]: 'import/proposal-bhc006-own-set.json',
};

/** Two files of one Artifacts item, as the entries the picker asks for. */
const ENTRIES = {
  items: ['Artifacts/BHC006_GMT_AF_010a.jpg', 'Artifacts/BHC006_GMT_AF_010b.jpg'].map((path, at) => ({
    entry: { file: { source: 's1', path } },
    example: true,
    flags: [],
    left: null,
    record: at + 1,
    unmatched: false,
  })),
  page: 1,
  perPage: 24,
  total: 2,
};

const seriesIn = (plan: Record<string, unknown>, id: string): Record<string, unknown> => {
  const levels: unknown = asRecord(plan['structure'], ['structure'])['levels'];
  const found: unknown = (Array.isArray(levels) ? levels : []).find((level: unknown) => isRecord(level) && level['id'] === id);
  return asRecord(found, ['levels', id]);
};

/**
 * The import routes over one stored plan. A preview answers the recorded
 * proposal of the edits of the plan it is sent, else of the stored plan's,
 * and says the proposal matches the plan when their structures are equal,
 * as the server does. A save stores the plan and answers the next revision.
 */
const structureServer = ({
  failPreviewWithEdits = false,
  stored: plan = {},
  answer,
  carry = false,
}: {
  readonly failPreviewWithEdits?: boolean;
  readonly stored?: Record<string, unknown>;
  readonly answer?: Record<string, unknown>;
  /** The proposal copies the plan's carried members, as the proposer does; the recordings were made from plans that hold none. */
  readonly carry?: boolean;
} = {}): {
  readonly saved: Record<string, unknown>[];
  readonly asked: unknown[];
} => {
  const detail = asRecord(loadFixture('import/import-draft.json'), []);
  let stored: Record<string, unknown> = { ...detail, plan: { ...asRecord(detail['plan'], ['plan']), ...plan } };
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
        const include = body['include'];
        if (Array.isArray(include) && include.includes('entries')) {
          return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, entries: ENTRIES });
        }
        if (Array.isArray(include) && include.includes('sheets')) {
          return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, sheets: [AF_SHEET], columnMatches: [], listCandidates: [] });
        }
        if (Array.isArray(include) && include.includes('nameWords')) {
          return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, nameWords: { names: 0, parts: [], places: [] } });
        }
        if (answer !== undefined) {
          return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, proposal: answer });
        }
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
        const recorded = asRecord(loadFixture(name), []);
        const proposal = carry ? carried(recorded, plan) : recorded;
        const matchesPlan = canonical(proposal['structure']) === canonical(plan['structure']);
        return jsonResponse({ sources: SOURCES, pattern: { groups: ['identifier'] }, proposal: { ...proposal, matchesPlan } });
      },
      '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
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
