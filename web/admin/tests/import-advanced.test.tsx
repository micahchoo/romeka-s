/**
 * The Advanced screens, one section at a time: an edit there is saved in the
 * plan, shows in Simple, and an answer given in Simple shows back in
 * Advanced, through the one store both modes edit.
 *
 * Simple's screens are stand-ins that read and edit the plan through
 * `useDraft()`, the seam every real body uses: each shows the plan member
 * its screen answers and gives that answer the way the Simple screen does.
 * An old step's address opens Structure at the question it asked, so the
 * Structure stand-in answers that question.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { asRecord, isRecord, isUnknownArray, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { patternOf } from '../src/pages/import/example';
import { ImportPage } from '../src/pages/import/page';
import { advancedServer, PATH } from './import-advanced-server';
import { loadFixture } from './loadFixture';
import { installFetch, requestBody, requestsFor } from './stub';

vi.mock('../src/pages/import/simple/bodies', async () => {
  const { useDraft } = await import('../src/pages/import/draft-context');
  const Stand = ({ show, answer }: { readonly show: (plan: import('../src/api/import-plan').ImportPlan) => unknown; readonly answer: (plan: import('../src/api/import-plan').ImportPlan) => import('../src/api/import-plan').ImportPlan }) => {
    const draft = useDraft();
    return (
      <div>
        <p aria-label="In Simple">{JSON.stringify(show(draft.plan))}</p>
        <button type="button" onClick={() => { draft.change(answer); }}>
          Answer in Simple
        </button>
      </div>
    );
  };
  const { seriesOf, withSeries } = await import('../src/pages/import/series');
  /** Simple's Structure, standing in for the question an old step's address opens it at. */
  const Structure = () => {
    const { place } = useDraft();
    const focus = place.mode === 'simple' ? place.focus : undefined;
    return focus === 'list' ? (
      <Stand
        show={(plan) => seriesOf(plan, 'all').lists[0]?.join?.column}
        answer={(plan) =>
          withSeries(plan, 'all', (series) => ({
            ...series,
            lists: series.lists.map((sheet) => (sheet.join === null ? sheet : { ...sheet, join: { ...sheet.join, column: { index: 1, header: 'Title' } } })),
          }))
        }
      />
    ) : (
      <Stand show={(plan) => seriesOf(plan, 'all').grouping} answer={(plan) => withSeries(plan, 'all', (series) => ({ ...series, grouping: { ...series.grouping, caseFold: true } }))} />
    );
  };
  return {
    SIMPLE_BODIES: {
      structure: Structure,
      check: () => (
        <Stand
          show={(plan) => [plan.flags, plan.anomalies, plan.decisions.length]}
          answer={(plan) => ({ ...plan, flags: { ...plan.flags, 'cells-count': 'keep' }, decisions: [...plan.decisions, { kind: 'leave', entry: { file: { source: 's1', path: 'Letters/item_00008_back.jpg' } } }] })}
        />
      ),
      add: () => <Stand show={(plan) => plan.action.action} answer={(plan) => ({ ...plan, action: { ...plan.action, action: 'append' } })} />,
    },
  };
});

const at = (...rest: readonly string[]) => <ImportPage param="1" rest={['1', ...rest]} />;

const lastSaved = (saved: readonly Record<string, unknown>[]): Record<string, unknown> => saved[saved.length - 1] ?? {};

const savedAfter = async (saved: readonly Record<string, unknown>[], count: number): Promise<Record<string, unknown>> => {
  await waitFor(() => {
    expect(saved.length).toBeGreaterThanOrEqual(count);
  }, { timeout: 3000 });
  return lastSaved(saved);
};

const inSimple = (): unknown => JSON.parse(screen.getByLabelText('In Simple').textContent);

const pathOf = (value: unknown, ...keys: readonly (string | number)[]): unknown =>
  keys.reduce<unknown>((at, key) => (typeof key === 'number' ? (Array.isArray(at) ? at[key] : undefined) : asRecord(at, [])[key]), value);

afterEach(() => {
  window.location.hash = '';
});

describe('Advanced, Files', () => {
  it('removes a mark and the pattern it wrote; Simple reads the change and answers back', async () => {
    const server = advancedServer(9);
    installFetch(server.handler);
    const view = render(at('advanced', 'files'));
    expect(await screen.findByText('Only the words “front”, “back”')).toBeInTheDocument();
    expect(screen.getByText('The pattern is the one these marks write. Removing a mark writes it again.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Remove the mark: Only the words “front”, “back”' }));
    const plan = await savedAfter(server.saved, 1);
    expect(plan['example']).toBeNull();
    expect(plan['grouping']).toEqual({ pattern: patternOf([]), caseFold: false });

    view.rerender(at('simple', 'found'));
    expect(inSimple()).toEqual({ pattern: patternOf([]), caseFold: false });
    fireEvent.click(screen.getByRole('button', { name: 'Answer in Simple' }));
    view.rerender(at('advanced', 'files'));
    expect(await screen.findByRole('checkbox', { name: 'Treat capital and small letters in the identifier as the same' })).toBeChecked();
    expect(screen.getByText(/^None\. Files are one item only when/)).toBeInTheDocument();
  });

  it('counts what a typed pattern fits before it is used, shows its groups on a real name, then uses it', async () => {
    const server = advancedServer(9);
    installFetch(server.handler);
    render(at('advanced', 'files'));
    expect(await screen.findByText('19 of 19 files fit. The files that fit make 10 items.')).toBeInTheDocument();
    const field = screen.getByLabelText('Regex');
    const strict = '^Letters/(?<identifier>item_\\d+)_(?<part>front|back)\\.jpg$';
    fireEvent.change(field, { target: { value: strict } });
    expect(await screen.findByText(/^18 of 19 files fit\. The files that fit make 9 items\./)).toBeInTheDocument();
    expect(screen.getByText(/1 does not fit\.$/)).toBeInTheDocument();
    expect(screen.getByText('Not used yet. The counts above are for the regex as you typed it.')).toBeInTheDocument();
    const key = screen.getByText('identifier').closest('li');
    expect(key).toHaveTextContent('identifier item_00001');
    expect(document.querySelector('mark.adv-group-part')).toHaveTextContent('front');
    expect(server.saved).toHaveLength(0);
    expect(server.previewed.some((sent) => pathOf(sent, 'grouping', 'pattern') === strict && pathOf(sent, 'anomalies', 'unmatchedFiles') === 'leave')).toBe(true);

    fireEvent.change(field, { target: { value: '(' } });
    expect(await screen.findByText(/^The regex cannot be read:/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Use this pattern' })).toBeDisabled();

    fireEvent.change(field, { target: { value: strict } });
    await screen.findByText(/1 does not fit\.$/);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Use this pattern' })).toBeEnabled();
    });
    fireEvent.click(screen.getByRole('button', { name: 'Use this pattern' }));
    const plan = await savedAfter(server.saved, 1);
    expect(plan['grouping']).toEqual({ pattern: strict, caseFold: false });
    expect(await screen.findByText('This is the pattern in use.')).toBeInTheDocument();
    expect(screen.getByText('The pattern was typed by hand. Removing a mark does not change it.')).toBeInTheDocument();
  });

  it('counts a pattern over 40,000 items without drawing them', async () => {
    const server = advancedServer(40_000);
    installFetch(server.handler);
    render(at('advanced', 'files'));
    expect(await screen.findByText('80,001 of 80,001 files fit. The files that fit make 40,001 items.', {}, { timeout: 10_000 })).toBeInTheDocument();
    expect(document.querySelectorAll('body *').length).toBeLessThan(1500);
  }, 30_000);
});

describe('Advanced, Lists', () => {
  it('reads, joins and maps a list; the join column chosen in Simple shows here', async () => {
    const server = advancedServer(9);
    installFetch(server.handler);
    const view = render(at('advanced', 'lists'));
    const join = await screen.findByRole('combobox', { name: 'Join column' });
    expect(await within(join).findByRole('option', { name: 'Identifier: 9 of 9 cells are an identifier, 0 a file name' })).toBeInTheDocument();
    fireEvent.change(screen.getByRole('combobox', { name: 'Delimiter' }), { target: { value: ';' } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Capital letters (A2 and a2)' }));
    const field = await screen.findByRole('combobox', { name: 'Field for Notes' });
    await within(field).findByRole('option', { name: 'Description (dcterms:description)' });
    fireEvent.change(field, { target: { value: 'dcterms:description' } });
    fireEvent.change(screen.getByRole('textbox', { name: 'Split Notes at' }), { target: { value: '|' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Type of Notes' }), { target: { value: 'uri' } });
    fireEvent.change(screen.getByRole('textbox', { name: 'Language of Notes' }), { target: { value: 'mr' } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Private values for Notes' }));
    fireEvent.change(screen.getByRole('combobox', { name: 'Field for Title' }), { target: { value: '' } });

    const plan = await savedAfter(server.saved, 1);
    await waitFor(() => {
      expect(pathOf(lastSaved(server.saved), 'sheets', 0, 'columns', 0, 'options', 'private')).toBe(true);
    }, { timeout: 3000 });
    const last = lastSaved(server.saved);
    expect(plan).toBeDefined();
    expect(pathOf(last, 'sheets', 0, 'read', 'delimiter')).toBe(';');
    expect(pathOf(last, 'sheets', 0, 'join', 'normalize')).toEqual({ caseFold: true, leadingZeros: false, separators: false });
    expect(pathOf(last, 'sheets', 0, 'columns')).toEqual([
      { column: { index: 3, header: 'Notes' }, options: { split: '|', type: 'uri', lang: 'mr', private: true } },
    ]);
    expect(pathOf(last, 'slots', 'item', 'dcterms:description')).toEqual({ sources: [{ from: 'column', sheet: 'register', column: { index: 3, header: 'Notes' } }], combine: 'first' });
    expect(pathOf(last, 'slots', 'item', 'dcterms:title')).toEqual({ sources: [{ from: 'group', name: 'identifier' }], combine: 'first' });

    view.rerender(at('simple', 'list'));
    expect(inSimple()).toEqual({ index: 0, header: 'Identifier' });
    fireEvent.click(screen.getByRole('button', { name: 'Answer in Simple' }));
    view.rerender(at('advanced', 'lists'));
    expect(await screen.findByRole('combobox', { name: 'Join column' })).toHaveValue('1');
    expect(await screen.findByText(/^Every row has a file\. 1 item has files but no row\./)).toBeInTheDocument();
  });
});

describe('Advanced, Fields', () => {
  it('orders sources, adds one, combines; item sets are no field rule', async () => {
    const server = advancedServer(9);
    installFetch(server.handler);
    render(at('advanced', 'fields'));
    const title = await screen.findByRole('article', { name: 'Title (dcterms:title)' });
    fireEvent.click(within(title).getByRole('button', { name: 'Move List register.csv, column “Title” down' }));
    fireEvent.change(within(title).getByRole('combobox', { name: 'Add a source to Title (dcterms:title)' }), { target: { value: JSON.stringify({ from: 'folder', level: 2 }) } });
    fireEvent.change(within(title).getByRole('combobox', { name: 'Combine for Title (dcterms:title)' }), { target: { value: 'all' } });
    // Item sets are a node's Set and the archive's own sets (P5 unit C3), never a field rule.
    expect(screen.queryByRole('article', { name: 'Item set' })).toBeNull();
    expect(within(screen.getByRole('combobox', { name: 'Add a rule' })).queryByRole('option', { name: 'Item set' })).toBeNull();
    // The defaults are the archive's: its node holds them.
    const [archive, letters] = within(screen.getByRole('listbox', { name: 'Your archive' })).getAllByRole('option');
    fireEvent.click(archive ?? document.body);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Visibility' }), { target: { value: 'public' } });

    await waitFor(() => {
      expect(pathOf(lastSaved(server.saved), 'defaults', 'visibility')).toBe('public');
    }, { timeout: 3000 });
    const plan = lastSaved(server.saved);
    expect(pathOf(plan, 'slots', 'item', 'dcterms:title')).toEqual({
      sources: [{ from: 'group', name: 'identifier' }, { from: 'column', sheet: 'register', column: { index: 1, header: 'Title' } }, { from: 'folder', level: 2 }],
      combine: 'all',
    });
    expect(pathOf(plan, 'slots', 'item', 'sets')).toEqual({ sources: [{ from: 'folder', level: 1 }], combine: 'first' });
    // Back on the folder, its Fields section is open again: the address named it.
    fireEvent.click(letters ?? document.body);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Add a rule' }), { target: { value: 'visibility' } });
    const visibility = await screen.findByRole('article', { name: 'Visibility' });
    expect(within(visibility).getByRole('option', { name: 'Combine: all' })).toBeDisabled();
    expect(within(visibility).getByText('This field keeps one value, so only “first” can combine its sources.')).toBeInTheDocument();
  });
});

/**
 * "Owner" in `docs/import-v2.md`: a global admin may name any user, a site
 * admin only themself. The default owner offers what the run would accept.
 */
describe('Advanced, default owner', () => {
  const signIn = (role: string): void => {
    const users = loadFixture('users-page.json');
    if (!isUnknownArray(users) || !isRecord(users[1])) {
      throw new Error('expected a users page');
    }
    setSession(parseUser({ ...users[1], 'o:role': role }));
  };
  const ownerChoices = async (): Promise<readonly string[]> => {
    const server = advancedServer(9);
    installFetch(server.handler);
    render(at('advanced', 'fields'));
    const [archive] = within(await screen.findByRole('listbox', { name: 'Your archive' })).getAllByRole('option');
    fireEvent.click(archive ?? document.body);
    const owner = await screen.findByRole('combobox', { name: 'Owner' });
    await waitFor(() => {
      expect(within(owner).getAllByRole('option').length).toBeGreaterThan(1);
    });
    return within(owner).getAllByRole('option').map((option) => option.textContent);
  };

  it('offers a global admin every user', async () => {
    signIn('global_admin');
    try {
      expect(await ownerChoices()).toEqual([
        'The person who runs the import',
        'Ada Lovelace (ada@example.org)',
        'Grace Hopper (grace@example.org)',
      ]);
    } finally {
      setSession(null);
    }
  });

  it('offers a site admin only themself', async () => {
    signIn('site_admin');
    try {
      expect(await ownerChoices()).toEqual(['The person who runs the import', 'Grace Hopper (grace@example.org)']);
    } finally {
      setSession(null);
    }
  });
});

describe('Advanced, Problems', () => {
  it('sets a policy per kind and an answer per anomaly with counts; a Simple answer shows here', async () => {
    const server = advancedServer(9);
    installFetch(server.handler);
    const view = render(at('advanced', 'problems'));
    const date = await screen.findByRole('combobox', { name: 'Policy for date-form' });
    expect(date.closest('tr')).toHaveTextContent('date-form');
    expect(date.closest('tr')).toHaveTextContent('A date not in a form Omeka can sort');
    fireEvent.change(date, { target: { value: 'leave' } });
    expect(within(screen.getByRole('combobox', { name: 'Policy for space-at-ends' })).getAllByRole('option').map((one) => one.textContent)).toEqual(['keep', 'repair', 'leave']);
    const listed = screen.getByRole('combobox', { name: 'Policy for listed-not-media' });
    expect(within(listed).getAllByRole('option').map((one) => one.textContent)).toEqual(['leave']);
    expect(listed).toHaveValue('leave');
    fireEvent.change(screen.getByRole('combobox', { name: 'Policy for unmatched' }), { target: { value: 'keep' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Identical files' }), { target: { value: 'keepAll' } });

    await waitFor(() => {
      expect(pathOf(lastSaved(server.saved), 'anomalies', 'identical')).toBe('keepAll');
    }, { timeout: 3000 });
    const plan = lastSaved(server.saved);
    expect(plan['flags']).toEqual({ 'date-form': 'leave' });
    expect(pathOf(plan, 'action', 'unmatched')).toBe('create');
    expect(plan['anomalies']).toEqual({ identical: 'keepAll', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' });

    view.rerender(at('simple', 'check'));
    expect(inSimple()).toEqual([{ 'date-form': 'leave' }, { identical: 'keepAll', unmatchedFiles: 'leave', itemsWithoutRow: 'keep' }, 4]);
    fireEvent.click(screen.getByRole('button', { name: 'Answer in Simple' }));
    view.rerender(at('advanced', 'problems'));
    expect(await screen.findByRole('combobox', { name: 'Policy for cells-count' })).toHaveValue('keep');
  });
});

describe('Advanced, Existing items', () => {
  it('chooses an action in one click, delete only after its count is typed; the action shows in Simple', async () => {
    const server = advancedServer(9);
    const fetchMock = installFetch(server.handler);
    const view = render(at('advanced', 'existing'));
    fireEvent.click(await screen.findByRole('radio', { name: /^Update\./ }));
    fireEvent.change(await screen.findByRole('combobox', { name: 'Unmatched' }), { target: { value: 'create' } });
    fireEvent.change(screen.getByRole('combobox', { name: 'Scope for Items' }), { target: { value: 'all' } });
    await waitFor(() => {
      expect(pathOf(lastSaved(server.saved), 'action', 'match', 'item', 'scope')).toBe('all');
    }, { timeout: 3000 });
    expect(pathOf(lastSaved(server.saved), 'action', 'action')).toBe('update');
    expect(pathOf(lastSaved(server.saved), 'action', 'unmatched')).toBe('create');

    const before = server.saved.length;
    fireEvent.click(screen.getByRole('radio', { name: /^Delete\./ }));
    expect(await screen.findByText('A delete removes 9 records and their media from Omeka. It cannot be undone.')).toBeInTheDocument();
    expect(screen.getByText('A delete takes no field rules, so the 1 rule under Fields is removed.')).toBeInTheDocument();
    const asked = requestsFor(fetchMock, `${PATH}/preview`).map(requestBody).filter((body) => JSON.stringify(body['include']) === '["check"]');
    expect(pathOf(asked[asked.length - 1], 'plan', 'action', 'action')).toBe('delete');
    const choose = screen.getByRole('button', { name: 'Choose delete: 9 records' });
    expect(choose).toBeDisabled();
    fireEvent.change(screen.getByLabelText('Type 9 to choose delete'), { target: { value: '8' } });
    expect(choose).toBeDisabled();
    fireEvent.change(screen.getByLabelText('Type 9 to choose delete'), { target: { value: '9' } });
    fireEvent.click(choose);
    await waitFor(() => {
      expect(pathOf(lastSaved(server.saved), 'action', 'action')).toBe('delete');
    }, { timeout: 3000 });
    expect(server.saved.length).toBe(before + 1);
    // The field rules go; the item sets are a free set, which a delete takes, so they stay.
    expect(lastSaved(server.saved)['slots']).toEqual({ item: { sets: { sources: [{ from: 'folder', level: 1 }], combine: 'first' } }, media: {}, itemSet: {} });

    view.rerender(at('simple', 'add'));
    expect(inSimple()).toBe('delete');
    fireEvent.click(screen.getByRole('button', { name: 'Answer in Simple' }));
    view.rerender(at('advanced', 'existing'));
    expect(await screen.findByRole('radio', { name: /^Append\./ })).toBeChecked();
  });
});

describe('Advanced, Changes', () => {
  it('lists every change in words, removes one, and shows a change made in Simple', async () => {
    const server = advancedServer(9);
    installFetch(server.handler);
    const view = render(at('advanced', 'changes'));
    expect(await screen.findByText('4 changes')).toBeInTheDocument();
    const rows = within(screen.getByRole('region', { name: 'Every change' })).getAllByRole('row').slice(1);
    expect(rows.map((row) => within(row).getAllByRole('cell')[1]?.textContent)).toEqual(['move', 'leave', 'pair', 'many']);
    expect(rows[0]).toHaveTextContent('Letters/item_00002_back.jpg: its own item');
    expect(rows[2]).toHaveTextContent('row register: item_00009#1 with Letters/item_00003_front.jpg');
    expect(rows[3]).toHaveTextContent('5 at once (Letters/item_00001_front.jpg, Letters/item_00004_front.jpg, Letters/item_00005_front.jpg and 2 more): left out');
    expect(await screen.findByText('It names a file or row that is not here any more.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Remove change 1' }));
    const plan = await savedAfter(server.saved, 1);
    expect(Array.isArray(plan['decisions']) ? plan['decisions'].map((one: unknown) => pathOf(one, 'kind')) : null).toEqual(['leave', 'pair', 'many']);

    view.rerender(at('simple', 'check'));
    fireEvent.click(screen.getByRole('button', { name: 'Answer in Simple' }));
    view.rerender(at('advanced', 'changes'));
    expect(await screen.findByText('4 changes')).toBeInTheDocument();
    expect(screen.getByText('Letters/item_00008_back.jpg: left out')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Remove all 4' }));
    fireEvent.click(screen.getByRole('button', { name: 'Yes, remove all 4' }));
    expect(await screen.findByText(/^No changes yet\./)).toBeInTheDocument();
    await waitFor(() => {
      expect(lastSaved(server.saved)['decisions']).toEqual([]);
    }, { timeout: 3000 });
  });
});
