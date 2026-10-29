/**
 * The item add form driven by a resource template, through the real client:
 * choosing a template lays out its rows, applies its class, and its rules
 * reach the values (`resource-form.js`).
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ItemsPage } from '../src/pages/items/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const property = (id: number, term: string, label: string): Record<string, unknown> => ({
  '@id': `http://localhost/api/properties/${String(id)}`,
  '@type': 'o:Property',
  'o:id': id,
  'o:local_name': term.split(':')[1] ?? term,
  'o:label': label,
  'o:comment': null,
  'o:term': term,
  'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
});

const PROPERTIES = [
  property(1, 'dcterms:title', 'Title'),
  property(2, 'dcterms:creator', 'Creator'),
  property(4, 'dcterms:description', 'Description'),
  property(7, 'dcterms:date', 'Date'),
  property(12, 'dcterms:relation', 'Relation'),
  property(30, 'foaf:name', 'name'),
];

const ORAL_HISTORY = {
  '@id': 'http://localhost/api/resource_templates/5',
  '@type': 'o:ResourceTemplate',
  'o:id': 5,
  'o:label': 'Oral history',
  'o:owner': null,
  'o:resource_class': { '@id': 'http://localhost/api/resource_classes/13', 'o:id': 13 },
  'o:title_property': null,
  'o:description_property': null,
  'o:resource_template_property': [
    {
      'o:property': { '@id': 'http://localhost/api/properties/2', 'o:id': 2 },
      'o:alternate_label': 'Interviewee',
      'o:alternate_comment': 'The person who tells the story.',
      'o:data_type': [],
      'o:is_required': true,
      'o:is_private': false,
      'o:default_lang': null,
    },
    {
      'o:property': { '@id': 'http://localhost/api/properties/1', 'o:id': 1 },
      'o:alternate_label': null,
      'o:alternate_comment': null,
      'o:data_type': ['literal'],
      'o:is_required': false,
      'o:is_private': false,
      'o:default_lang': 'fr',
    },
    {
      'o:property': { '@id': 'http://localhost/api/properties/7', 'o:id': 7 },
      'o:alternate_label': 'Interview notes',
      'o:alternate_comment': null,
      'o:data_type': ['literal'],
      'o:is_required': false,
      'o:is_private': true,
      'o:default_lang': null,
    },
    {
      'o:property': { '@id': 'http://localhost/api/properties/12', 'o:id': 12 },
      'o:alternate_label': null,
      'o:alternate_comment': null,
      'o:data_type': ['resource:item', 'customvocab:3'],
      'o:is_required': false,
      'o:is_private': false,
      'o:default_lang': null,
    },
  ],
};

const routes: Readonly<Record<string, () => Response>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
  '/api/resource_templates': () => jsonResponse([ORAL_HISTORY]),
  '/api/properties': () => jsonResponse(PROPERTIES),
  '/api/item_sets': () => jsonResponse([]),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  '/api/sites': () => jsonResponse([]),
};

const install = () =>
  installFetch((url, init) => {
    if (url.pathname === '/api/items' && init?.method === 'POST') {
      return jsonResponse(loadFixture('item-1.json'));
    }
    const handler = routes[url.pathname];
    return handler === undefined
      ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
      : handler();
  });

const fieldNamed = (label: string): HTMLElement => {
  const heading = screen.getByText(label, { selector: '.property-field-label' });
  const field = heading.closest('li');
  if (field === null) {
    throw new Error(`no field for ${label}`);
  }
  return field;
};

const chooseTemplate = async (): Promise<void> => {
  fireEvent.change(await screen.findByRole('combobox', { name: 'Item type' }), { target: { value: '5' } });
};

describe('an item form with a template', () => {
  it('starts with an empty title and description before a template is chosen', async () => {
    install();
    render(<ItemsPage param="new" />);
    expect(await screen.findByRole('textbox', { name: 'Title' })).toHaveValue('');
    expect(screen.getByRole('textbox', { name: 'Description' })).toHaveValue('');
  });

  it('lays out the template rows in order, with the alternate label, comment and term', async () => {
    install();
    render(<ItemsPage param="new" />);
    await chooseTemplate();

    const labels = screen
      .getAllByText(/.+/, { selector: '.property-field-label' })
      .map((node) => node.textContent);
    expect(labels).toEqual(['Interviewee', 'Title', 'Interview notes', 'Relation']);
    const interviewee = fieldNamed('Interviewee');
    expect(within(interviewee).getByText('dcterms:creator', { selector: '.property-field-term' })).toBeInTheDocument();
    expect(within(interviewee).getByText('The person who tells the story.')).toBeInTheDocument();
    expect(within(interviewee).getByText('Required')).toBeInTheDocument();
    expect(screen.getByLabelText('Class')).toHaveValue('13');
  });

  it('keeps a typed value of a property the template does not list, below the template rows', async () => {
    install();
    render(<ItemsPage param="new" />);
    fireEvent.change(await screen.findByRole('textbox', { name: 'Description' }), {
      target: { value: 'Recorded in 1972' },
    });
    await chooseTemplate();
    expect(screen.getByRole('textbox', { name: 'Description' })).toHaveValue('Recorded in 1972');

    fireEvent.change(screen.getByRole('combobox', { name: 'Item type' }), { target: { value: '' } });
    expect(screen.getByRole('textbox', { name: 'Description' })).toHaveValue('Recorded in 1972');
    // Without the template the creator row keeps its value and its own label.
    expect(screen.getByRole('textbox', { name: 'Creator' })).toBeInTheDocument();
  });

  it('refuses to save while a required row is empty, and names the field', async () => {
    const fetchMock = install();
    render(<ItemsPage param="new" />);
    await chooseTemplate();
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The following field is required: Interviewee',
    );
    expect(
      requestsFor(fetchMock, '/api/items').filter((request) => request.init?.method === 'POST'),
    ).toHaveLength(0);
  });

  it('sends the row defaults: private notes, the default language, the template class', async () => {
    const fetchMock = install();
    render(<ItemsPage param="new" />);
    await chooseTemplate();

    expect(screen.getByRole('checkbox', { name: 'Interview notes value public' })).not.toBeChecked();
    expect(within(fieldNamed('Interview notes')).getByText('Private')).toBeInTheDocument();
    expect(screen.getByLabelText('Title language')).toHaveValue('fr');

    fireEvent.change(screen.getByRole('textbox', { name: 'Interviewee' }), {
      target: { value: 'Rosa Parks' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'Title' }), {
      target: { value: 'Entretien' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'Interview notes' }), {
      target: { value: 'Ask about 1955' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items').filter((request) => request.init?.method === 'POST'),
      ).toHaveLength(1);
    });
    const post = requestsFor(fetchMock, '/api/items').find(
      (request) => request.init?.method === 'POST',
    );
    if (post === undefined) {
      throw new Error('expected a POST');
    }
    const body = requestBody(post);
    expect(body['o:resource_template']).toEqual({ 'o:id': 5 });
    expect(body['o:resource_class']).toEqual({ 'o:id': 13 });
    expect(body['dcterms:creator']).toEqual([
      { property_id: 2, type: 'literal', '@value': 'Rosa Parks', is_public: true },
    ]);
    expect(body['dcterms:title']).toEqual([
      { property_id: 1, type: 'literal', '@value': 'Entretien', '@language': 'fr', is_public: true },
    ]);
    expect(body['dcterms:date']).toEqual([
      { property_id: 7, type: 'literal', '@value': 'Ask about 1955', is_public: false },
    ]);
    expect(body['dcterms:relation']).toBeUndefined();
    expect(body['dcterms:description']).toBeUndefined();
  });

  it('offers only the row data types, naming one it does not know by its id', async () => {
    install();
    render(<ItemsPage param="new" />);
    await chooseTemplate();
    const choice = screen.getByLabelText('Relation data type');
    expect(within(choice).getAllByRole('option').map((option) => option.getAttribute('value'))).toEqual([
      'resource:item',
      'customvocab:3',
    ]);
    fireEvent.change(choice, { target: { value: 'customvocab:3' } });
    expect(screen.getByRole('textbox', { name: 'Relation' })).toBeInTheDocument();
    expect(screen.queryByLabelText('Title data type')).toBeNull();
  });

  it('adds another field from a plain-words search over labels and terms', async () => {
    install();
    render(<ItemsPage param="new" />);
    const search = await screen.findByLabelText('Add another field');
    fireEvent.change(search, { target: { value: 'foaf' } });
    fireEvent.click(screen.getByRole('button', { name: /foaf:name$/ }));
    expect(screen.getByRole('textbox', { name: 'name' })).toHaveValue('');

    fireEvent.change(search, { target: { value: 'who wrote it' } });
    const matches = screen.getByRole('list', { name: 'Matches for “who wrote it”' });
    expect(within(matches).getAllByRole('button')[0]).toHaveTextContent(/dcterms:creator$/);
  });

  it('names the class of the type beside the picker and keeps the class under a closed Advanced fold', async () => {
    install();
    render(<ItemsPage param="new" />);
    await chooseTemplate();
    expect(screen.getByText('Standard class: Photograph or picture')).toBeInTheDocument();
    const fold = screen.getByText('Advanced').closest('details');
    expect(fold).not.toHaveAttribute('open');
    fireEvent.change(screen.getByLabelText('Class'), { target: { value: '51' } });
    expect(fold).toHaveAttribute('open');
    expect(screen.getByText('This class is not the one its item type suggests.')).toBeInTheDocument();
  });

  it('offers a type only on the forms its "Used for" hint names, and always the one in use', async () => {
    window.localStorage.setItem('romeka.itemTypeUse', JSON.stringify({ 5: ['media'] }));
    install();
    render(<ItemsPage param="new" />);
    const picker = await screen.findByRole('combobox', { name: 'Item type' });
    expect(within(picker).queryByRole('option', { name: 'Oral history' })).toBeNull();
    window.localStorage.clear();
  });

  it('shows the stored definition of a field on "?"', async () => {
    install();
    render(<ItemsPage param="new" />);
    await chooseTemplate();
    const help = within(fieldNamed('Title')).getByRole('button', { name: 'What is Title?' });
    fireEvent.click(help);
    expect(help).toHaveAttribute('aria-expanded', 'true');
  });

  it('creates a new field in the local vocabulary when no standard one fits', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/vocabularies' && init?.method === 'POST') {
        return jsonResponse(loadFixture('vocabulary-1.json'));
      }
      if (url.pathname === '/api/vocabularies') {
        return jsonResponse([]);
      }
      if (url.pathname === '/api/properties' && init?.method === 'POST') {
        return jsonResponse({
          ...PROPERTIES[0],
          'o:id': 99,
          'o:local_name': 'condition',
          'o:label': 'Condition',
          'o:term': 'local:condition',
        });
      }
      const handler = routes[url.pathname];
      return handler === undefined
        ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
        : handler();
    });
    render(<ItemsPage param="new" />);
    fireEvent.change(await screen.findByLabelText('Add another field'), {
      target: { value: 'Condition' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create a new field “Condition”' }));
    expect(await screen.findByRole('textbox', { name: 'Condition' })).toBeInTheDocument();
    const vocabulary = requestBody(
      requestsFor(fetchMock, '/api/vocabularies').find((request) => request.init?.method === 'POST') ??
        (() => {
          throw new Error('expected the vocabulary create');
        })(),
    );
    expect(vocabulary['o:prefix']).toBe('local');
    expect(vocabulary['o:namespace_uri']).toBe(`${window.location.origin}/vocab/local/`);
    const created = requestsFor(fetchMock, '/api/properties').find((request) => request.init?.method === 'POST');
    expect(created === undefined ? null : requestBody(created)).toMatchObject({
      'o:local_name': 'condition',
      'o:label': 'Condition',
    });
  });
});
