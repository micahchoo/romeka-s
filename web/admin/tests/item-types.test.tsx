/**
 * The item type editor through the real client: what it sends for a stored
 * template, each override a field and the type offer, the starter kinds, and
 * the preview being the real item form.
 */
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Mock } from 'vitest';
import { isUnknownArray, parseUser } from '../src/api/parse';
import { setSession } from '../src/api/session';
import { setLocale } from '../src/i18n';
import { hi } from '../src/i18n/namespaces/item-types/locales/hi';
import { ItemTypesPage } from '../src/pages/item-types/page';
import { loadFixture } from './loadFixture';
import {
  first,
  installFetch,
  jsonResponse,
  listResponse,
  requestBody,
  requestsFor,
  type Handler,
} from './stub';

afterEach(() => {
  window.location.hash = '';
  window.localStorage.clear();
});

const property = (id: number, term: string, label: string, comment: string | null = null) => ({
  '@id': `http://localhost/api/properties/${String(id)}`,
  '@type': 'o:Property',
  'o:id': id,
  'o:local_name': term.split(':')[1] ?? term,
  'o:label': label,
  'o:comment': comment,
  'o:term': term,
  'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
});

const resourceClass = (id: number, term: string, label: string) => ({
  ...property(id, term, label),
  '@id': `http://localhost/api/resource_classes/${String(id)}`,
  '@type': 'o:ResourceClass',
});

const PROPERTIES = [
  property(1, 'dcterms:title', 'Title', 'A name given to the resource.'),
  property(2, 'dcterms:creator', 'Creator', 'An entity responsible for making the resource.'),
  property(4, 'dcterms:description', 'Description'),
  property(5, 'dcterms:created', 'Date Created'),
  property(6, 'dcterms:spatial', 'Spatial Coverage'),
  property(13, 'dcterms:relation', 'Relation'),
  property(20, 'bibo:recipient', 'recipient'),
  property(21, 'bibo:numPages', 'number of pages'),
  property(22, 'dcterms:extent', 'Extent'),
];

const CLASSES = [
  resourceClass(13, 'dctype:StillImage', 'Still Image'),
  resourceClass(60, 'bibo:Letter', 'Letter'),
  resourceClass(61, 'dctype:PhysicalObject', 'Physical Object'),
];

const routes = (extra: Readonly<Record<string, Handler>> = {}): Handler => {
  const table: Readonly<Record<string, Handler>> = {
    '/api/users': () => jsonResponse(loadFixture('users-page.json')),
    '/api/properties': () => listResponse(PROPERTIES, PROPERTIES.length),
    '/api/resource_classes': () => listResponse(CLASSES, CLASSES.length),
    '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
    '/api/romeka/starters': () => jsonResponse(loadFixture('starters.json')),
    '/api/resource_templates/2': () => jsonResponse(loadFixture('resource-template-2.json')),
    ...extra,
  };
  return (url, init) =>
    table[url.pathname]?.(url, init) ??
    jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

const sent = (fetchMock: Mock<typeof fetch>, pathname: string, method: string) =>
  first(requestsFor(fetchMock, pathname).filter((request) => request.init?.method === method));

const rowsSent = (body: Record<string, unknown>): readonly Record<string, unknown>[] => {
  const rows = body['o:resource_template_property'];
  if (!Array.isArray(rows)) {
    throw new Error('expected the row list');
  }
  return rows.filter(
    (row): row is Record<string, unknown> => typeof row === 'object' && row !== null,
  );
};

const fieldList = (): HTMLElement => {
  const list = document.querySelector<HTMLElement>('.type-field-list');
  if (list === null) {
    throw new Error('expected the field list');
  }
  return list;
};

const save = async (fetchMock: Mock<typeof fetch>) => {
  fireEvent.click(screen.getByRole('button', { name: 'Save item type' }));
  return requestBody(await waitFor(() => sent(fetchMock, '/api/resource_templates/2', 'PUT')));
};

const openPhotograph = async (extra: Readonly<Record<string, Handler>> = {}) => {
  const fetchMock = installFetch(routes(extra));
  render(<ItemTypesPage param="2" />);
  await screen.findByRole('textbox', { name: 'Item type name' });
  return fetchMock;
};

describe('a stored item type', () => {
  it('saves back every member as it was read when nothing changed', async () => {
    const fetchMock = await openPhotograph();
    const body = await save(fetchMock);
    expect(body).toEqual({
      'o:label': 'Photograph',
      'o:owner': { 'o:id': 3 },
      'o:resource_class': { 'o:id': 13 },
      'o:title_property': { 'o:id': 1 },
      'o:description_property': { 'o:id': 4 },
      'o:resource_template_property': [
        {
          'o:property': { 'o:id': 1 },
          'o:alternate_label': null,
          'o:alternate_comment': null,
          'o:data_type': ['literal'],
          'o:is_required': true,
          'o:is_private': false,
          'o:default_lang': null,
        },
        {
          'o:property': { 'o:id': 4 },
          'o:alternate_label': 'Caption',
          'o:alternate_comment': null,
          'o:data_type': ['literal'],
          'o:is_required': false,
          'o:is_private': false,
          'o:default_lang': 'en',
        },
        {
          'o:property': { 'o:id': 13 },
          'o:alternate_label': null,
          'o:alternate_comment': null,
          'o:data_type': ['resource:item'],
          'o:is_required': false,
          'o:is_private': true,
          'o:default_lang': null,
        },
      ],
    });
  });

  it('shows the class in plain words with its term, and the stored definition on "?"', async () => {
    await openPhotograph();
    expect(screen.getByText('Standard class: Photograph or picture')).toBeInTheDocument();
    const help = screen.getAllByRole('button', { name: 'What is Title?' })[0];
    if (help === undefined) {
      throw new Error('expected a "?" for Title');
    }
    fireEvent.click(help);
    expect(help).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getAllByText(/A name given to the resource\./)[0]).toBeVisible();
  });

  it('renames a field, sets it required and private, and adds an accepted kind after the stored ones', async () => {
    const fetchMock = await openPhotograph();
    fireEvent.change(
      screen.getByRole('textbox', { name: 'Name on the form for dcterms:description' }),
      {
        target: { value: 'What it shows' },
      },
    );
    fireEvent.click(screen.getByRole('checkbox', { name: 'What it shows is required' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'What it shows is private' }));
    fireEvent.click(screen.getByRole('button', { name: 'Change what What it shows accepts' }));
    const accepts = screen.getByRole('group', { name: 'What What it shows accepts' });
    fireEvent.click(within(accepts).getByRole('checkbox', { name: 'Another item' }));
    fireEvent.change(screen.getAllByLabelText('Help text on the form')[1] ?? document.body, {
      target: { value: 'What the photograph shows.' },
    });

    const rows = rowsSent(await save(fetchMock));
    expect(rows[1]).toEqual({
      'o:property': { 'o:id': 4 },
      'o:alternate_label': 'What it shows',
      'o:alternate_comment': 'What the photograph shows.',
      'o:data_type': ['literal', 'resource:item'],
      'o:is_required': true,
      'o:is_private': true,
      'o:default_lang': 'en',
    });
  });

  it('offers term lists as their own group, by label, and sends the list type', async () => {
    const places = {
      '@id': 'http://localhost/api/data_types/customvocab:3',
      '@type': ['o:DataType'],
      'o:id': 'customvocab:3',
      'o:label': 'Places',
    };
    const fetchMock = await openPhotograph({
      '/api/data_types': () => {
        const fixture: unknown = loadFixture('data-types-page.json');
        const types: readonly unknown[] = Array.isArray(fixture) ? fixture : [];
        return jsonResponse([...types, places]);
      },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Change what Caption accepts' }));
    const accepts = screen.getByRole('group', { name: 'What Caption accepts' });
    const lists = within(accepts).getByRole('group', { name: 'Term lists' });
    expect(within(lists).getAllByRole('checkbox').map((box) => box.closest('label')?.textContent)).toEqual([
      'Places',
    ]);
    expect(within(accepts).getAllByRole('checkbox', { name: 'Places' })).toHaveLength(1);
    expect(within(accepts).queryByRole('checkbox', { name: 'Text' })).not.toBeNull();
    expect(within(lists).queryByRole('checkbox', { name: 'Text' })).toBeNull();
    fireEvent.click(within(lists).getByRole('checkbox', { name: 'Places' }));
    expect(screen.getAllByText(/Term list: Places/).length).toBeGreaterThan(0);

    const rows = rowsSent(await save(fetchMock));
    expect(rows[1]?.['o:data_type']).toEqual(['literal', 'customvocab:3']);
  });

  it('moves a field picked up by its handle with the arrow keys and keeps the handle focused', async () => {
    const fetchMock = await openPhotograph();
    const handle = screen.getByRole('button', { name: 'Move Caption, 2 of 3' });
    fireEvent.click(handle);
    fireEvent.keyDown(handle, { key: 'ArrowUp' });
    const moved = await screen.findByRole('button', { name: 'Move Caption, 1 of 3' });
    await waitFor(() => {
      expect(moved).toHaveFocus();
    });
    expect(screen.getByText('Caption: position 1 of 3.')).toBeInTheDocument();
    const rows = rowsSent(await save(fetchMock));
    expect(rows.map((row) => row['o:property'])).toEqual([
      { 'o:id': 4 },
      { 'o:id': 1 },
      { 'o:id': 13 },
    ]);
  });

  it('removes a field and changes the term behind another from a plain-words search', async () => {
    const fetchMock = await openPhotograph();
    fireEvent.click(screen.getByRole('button', { name: 'Remove Relation' }));
    fireEvent.click(screen.getByRole('button', { name: 'Change the standard term of Caption' }));
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find a standard term' }), {
      target: { value: 'who wrote it' },
    });
    const matches = screen.getByRole('list', { name: 'Matches for “who wrote it”' });
    fireEvent.click(within(matches).getAllByRole('button')[0] ?? document.body);
    const rows = rowsSent(await save(fetchMock));
    expect(rows.map((row) => row['o:property'])).toEqual([{ 'o:id': 1 }, { 'o:id': 2 }]);
    expect(rows[1]?.['o:alternate_label']).toBe('Caption');
  });

  it('changes the standard class from a plain-words search', async () => {
    const fetchMock = await openPhotograph();
    fireEvent.click(screen.getByRole('button', { name: 'Change the standard class' }));
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find a standard class' }), {
      target: { value: 'letter' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^Letter/ }));
    expect((await save(fetchMock))['o:resource_class']).toEqual({ 'o:id': 60 });
  });

  it('previews the real item form, laid out by the rows being edited', async () => {
    await openPhotograph();
    const preview = screen.getByRole('form', { name: 'Preview of the form for a new Photograph' });
    const labels = () =>
      Array.from(preview.querySelectorAll('.property-field-label')).map((node) => node.textContent);
    expect(labels()).toEqual(['Title', 'Caption', 'Relation']);
    fireEvent.change(screen.getByRole('textbox', { name: 'Name on the form for dcterms:title' }), {
      target: { value: 'Headline' },
    });
    expect(labels()).toEqual(['Headline', 'Caption', 'Relation']);
    expect(within(preview).getByText('Required')).toBeInTheDocument();
  });

  it('refuses an empty name before any request and shows the server refusal', async () => {
    const fetchMock = await openPhotograph({
      '/api/resource_templates/2': (_url, init) =>
        init?.method === 'PUT'
          ? jsonResponse(
              { errors: { 'o:label': ['The label is already taken.'] } },
              { status: 422 },
            )
          : jsonResponse(loadFixture('resource-template-2.json')),
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'Item type name' }), {
      target: { value: ' ' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save item type' }));
    expect(screen.getByRole('alert')).toHaveTextContent('Give the item type a name.');
    expect(
      requestsFor(fetchMock, '/api/resource_templates/2').filter((r) => r.init?.method === 'PUT'),
    ).toHaveLength(0);

    fireEvent.change(screen.getByRole('textbox', { name: 'Item type name' }), {
      target: { value: 'Photo' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save item type' }));
    expect(await screen.findByText(/The label is already taken\./)).toBeInTheDocument();
  });

  it('keeps "Used for" in this browser only and sends nothing for it', async () => {
    const fetchMock = await openPhotograph();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Media' }));
    const body = await save(fetchMock);
    expect(Object.keys(body)).not.toContain('o:data');
    await waitFor(() => {
      expect(window.localStorage.getItem('romeka.itemTypeUse')).toBe('{"2":["items","item-sets"]}');
    });
  });
});

/** The server's field counts for the Photograph type; its fields are Title (1), Caption (4) and Relation (13). */
const fieldCounts = (total: number, fields: readonly (readonly [number, number])[]): Handler => () =>
  jsonResponse({ total, fields: fields.map(([id, filled]) => ({ property_id: id, filled })) });

const rowOf = (name: string): HTMLElement => {
  const row = within(fieldList())
    .getAllByRole('listitem')
    .find((item) => within(item).queryByDisplayValue(name) !== null || within(item).queryByPlaceholderText(name) !== null);
  if (row === undefined) {
    throw new Error(`expected the row of ${name}`);
  }
  return row;
};

describe("an item type's field counts", () => {
  it('states how many of its items fill each field, and links the ones without a value to Items', async () => {
    const fetchMock = await openPhotograph({
      '/api/romeka/field-counts': fieldCounts(690, [
        [1, 690],
        [4, 412],
        [99, 3],
      ]),
    });
    expect(await within(rowOf('Caption')).findByText('412 of 690 items have a value.')).toBeInTheDocument();
    expect(first(requestsFor(fetchMock, '/api/romeka/field-counts')).url.searchParams.get('resource_template_id')).toBe('2');
    expect(within(rowOf('Caption')).getByRole('link', { name: 'Open the 278 without a value' })).toHaveAttribute(
      'href',
      '#/items?template=2&field=4%3Aempty',
    );
    expect(within(rowOf('Title')).getByText('690 of 690 items have a value.')).toBeInTheDocument();
    expect(within(rowOf('Title')).queryByRole('link', { name: /without a value/ })).not.toBeInTheDocument();
    expect(within(rowOf('Relation')).getByText('0 of 690 items have a value.')).toBeInTheDocument();
    expect(within(rowOf('Relation')).getByRole('link', { name: 'Open the 690 without a value' })).toHaveAttribute(
      'href',
      '#/items?template=2&field=13%3Aempty',
    );
  });

  it('words one item and one empty item in the singular', async () => {
    await openPhotograph({ '/api/romeka/field-counts': fieldCounts(1, [[1, 1]]) });
    expect(await within(rowOf('Title')).findByText('1 of 1 item has a value.')).toBeInTheDocument();
    expect(within(rowOf('Relation')).getByText('0 of 1 item has a value.')).toBeInTheDocument();
    expect(within(rowOf('Relation')).getByRole('link', { name: 'Open the 1 without a value' })).toBeInTheDocument();
  });

  it('states nothing for a type with no items the reader may see, or when the counts cannot be read', async () => {
    for (const answer of [fieldCounts(0, []), () => jsonResponse({ errors: { error: 'down' } }, { status: 500 })]) {
      const fetchMock = await openPhotograph({ '/api/romeka/field-counts': answer });
      await waitFor(() => {
        expect(requestsFor(fetchMock, '/api/romeka/field-counts')).toHaveLength(1);
      });
      expect(screen.getByRole('textbox', { name: 'Name on the form for dcterms:description' })).toBeInTheDocument();
      expect(screen.queryByText(/have a value|has a value/)).not.toBeInTheDocument();
      cleanup();
    }
  });
});

/**
 * Upstream's template form has no owner select; `hydrateOwner` lets only the
 * two admin roles change an owner. The port offers the select to them alone,
 * and a save by anyone else sends the stored owner, which is no change.
 */
describe('the item type owner select', () => {
  const signIn = (index: number): void => {
    const users = loadFixture('users-page.json');
    if (!isUnknownArray(users)) {
      throw new Error('expected a users page');
    }
    setSession(parseUser(users[index]));
  };

  it('type_settings_offers_the_owner_to_an_admin', async () => {
    signIn(0);
    try {
      await openPhotograph();
      expect(screen.getByRole('combobox', { name: 'Owner' })).toHaveValue('3');
    } finally {
      setSession(null);
    }
  });

  it('type_settings_hides_the_owner_from_an_editor_and_keeps_the_stored_one', async () => {
    signIn(1);
    try {
      const fetchMock = await openPhotograph();
      expect(screen.queryByRole('combobox', { name: 'Owner' })).toBeNull();
      expect((await save(fetchMock))['o:owner']).toEqual({ 'o:id': 3 });
    } finally {
      setSession(null);
    }
  });
});

describe('a new item type', () => {
  const created = (_url: URL, init: RequestInit | undefined) =>
    init?.method === 'POST'
      ? jsonResponse(loadFixture('resource-template-2.json'))
      : listResponse([], 0);

  it('starts a letter with its class and starter fields, renaming them in plain words', async () => {
    const fetchMock = installFetch(routes({ '/api/resource_templates': created }));
    render(<ItemTypesPage param="new" />);
    fireEvent.change(await screen.findByLabelText('What kind of thing is this?'), {
      target: { value: 'Letter' },
    });
    expect(
      screen.getByRole('heading', { name: 'Start a Letter type with these fields' }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Use these fields' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create item type' }));

    const body = requestBody(
      await waitFor(() => sent(fetchMock, '/api/resource_templates', 'POST')),
    );
    expect(body['o:label']).toBe('Letter');
    expect(body['o:resource_class']).toEqual({ 'o:id': 60 });
    expect(
      rowsSent(body).map((row) => [
        row['o:property'],
        row['o:alternate_label'],
        row['o:is_required'],
      ]),
    ).toEqual([
      [{ 'o:id': 1 }, null, true],
      [{ 'o:id': 2 }, 'Sender', false],
      [{ 'o:id': 20 }, 'Recipient', false],
      [{ 'o:id': 5 }, 'Date written', false],
      [{ 'o:id': 6 }, 'Place', false],
      [{ 'o:id': 4 }, null, false],
    ]);
    await waitFor(() => {
      expect(window.location.hash).toBe('#/resource-templates/2');
    });
  });

  it('asks in the active locale and starts a letter with English labels where plain words are not curated', async () => {
    const fetchMock = installFetch(routes({ '/api/resource_templates': created }));
    await setLocale('hi');
    try {
      render(<ItemTypesPage param="new" />);
      fireEvent.change(await screen.findByLabelText(hi['ask.question']), {
        target: { value: 'Letter' },
      });
      expect(screen.queryByText('What kind of thing is this?')).toBeNull();
      fireEvent.click(screen.getByRole('button', { name: hi['ask.useFields'] }));
      fireEvent.click(await screen.findByRole('button', { name: hi['type.create'] }));
      const body = requestBody(
        await waitFor(() => sent(fetchMock, '/api/resource_templates', 'POST')),
      );
      expect(rowsSent(body).map((row) => row['o:alternate_label'])).toEqual([
        null,
        'Sender',
        'Recipient',
        'Date written',
        'Place',
        null,
      ]);
    } finally {
      await setLocale('en');
    }
  });

  it('starts a kind it does not know as a physical object with a title', async () => {
    const fetchMock = installFetch(routes({ '/api/resource_templates': created }));
    render(<ItemTypesPage param="new" />);
    fireEvent.change(await screen.findByLabelText('What kind of thing is this?'), {
      target: { value: 'Spoon' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create item type' }));
    const body = requestBody(
      await waitFor(() => sent(fetchMock, '/api/resource_templates', 'POST')),
    );
    expect(body['o:resource_class']).toEqual({ 'o:id': 61 });
    expect(rowsSent(body).map((row) => row['o:property'])).toEqual([{ 'o:id': 1 }]);
  });

  it('lists and creates the fields the server lists, not a copy of its own', async () => {
    const starters = loadFixture('starters.json');
    if (!Array.isArray(starters)) {
      throw new Error('expected the starter list');
    }
    const changed = starters.map((starter: unknown) =>
      typeof starter === 'object' && starter !== null && 'id' in starter && starter.id === 'letter'
        ? {
            ...starter,
            fields: [
              { term: 'dcterms:title', required: true, label: 'Title', dataTypes: ['literal'] },
              { term: 'dcterms:relation', required: false, label: 'Answers to', dataTypes: ['literal'] },
            ],
          }
        : starter,
    );
    const fetchMock = installFetch(
      routes({
        '/api/romeka/starters': () => jsonResponse(changed),
        '/api/resource_templates': created,
      }),
    );
    render(<ItemTypesPage param="new" />);
    fireEvent.change(await screen.findByLabelText('What kind of thing is this?'), {
      target: { value: 'Letter' },
    });
    expect(screen.getByText('Answers to')).toBeInTheDocument();
    expect(screen.queryByText('Sender')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Use these fields' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create item type' }));
    const body = requestBody(
      await waitFor(() => sent(fetchMock, '/api/resource_templates', 'POST')),
    );
    expect(rowsSent(body).map((row) => row['o:alternate_label'])).toEqual([null, 'Answers to']);
  });

  it('adds a field found from how people describe it', async () => {
    installFetch(routes());
    render(<ItemTypesPage param="new" />);
    fireEvent.change(await screen.findByLabelText('What kind of thing is this?'), {
      target: { value: 'Diary' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Start with Title only' }));
    fireEvent.change(await screen.findByRole('searchbox', { name: 'Add a field' }), {
      target: { value: 'how many pages' },
    });
    const matches = screen.getByRole('list', { name: 'Matches for “how many pages”' });
    expect(
      within(matches)
        .getAllByRole('button')
        .map((b) => b.querySelector('code')?.textContent ?? null),
    ).toEqual(['bibo:numPages', 'dcterms:extent', null]);
    fireEvent.click(within(matches).getAllByRole('button')[0] ?? document.body);
    expect(
      within(fieldList()).getByRole('textbox', { name: 'Name on the form for bibo:numPages' }),
    ).toBeInTheDocument();
  });
});
