/**
 * The values board (`#/fields/<id>`): a value moves to another field only after the server's dry
 * run says what will change, and the last move of the visit can be undone exactly.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isRecord } from '../src/api/parse';
import { FieldsPage } from '../src/pages/fields/page';
import { ValuesPage } from '../src/pages/fields/values-page';
import { PAGE_GROUPS } from '../src/app/pages';
import { installFetch, jsonResponse, listResponse, requestBody, requestsFor, route } from './stub';

const property = (id: number, term: string, label: string): Record<string, unknown> => ({
  '@id': `http://localhost/api/properties/${String(id)}`,
  '@type': 'o:Property',
  'o:id': id,
  'o:local_name': term.split(':')[1],
  'o:label': label,
  'o:comment': null,
  'o:term': term,
  'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
});

const PROPERTIES = [property(3, 'dcterms:subject', 'Subject'), property(40, 'dcterms:spatial', 'Spatial Coverage')];

const census = (property: number, values: readonly [string, number][]): unknown => ({
  property,
  values: values.map(([value, items]) => ({ value, items, media: 0, item_sets: 0, editable: items })),
});

const moved = {
  changed: 72,
  skipped: 0,
  moved: [10, 11],
  merged: [12],
  copied: [],
  not_in_form: [{ template: 7, label: 'Photo Narrative', resources: 70 }],
};

const template = {
  '@id': 'http://localhost/api/resource_templates/7',
  '@context': 'http://localhost/api-context',
  '@type': ['o:ResourceTemplate'],
  'o:id': 7,
  'o:label': 'Photo Narrative',
  'o:owner': null,
  'o:resource_class': null,
  'o:title_property': null,
  'o:description_property': null,
  'o:resource_template_property': [
    {
      'o:property': { '@id': 'http://localhost/api/properties/3', 'o:id': 3 },
      'o:alternate_label': null,
      'o:alternate_comment': null,
      'o:data_type': [],
      'o:default_lang': null,
      'o:is_required': false,
      'o:is_private': false,
    },
  ],
};

const routes = (state: { placed: boolean }) =>
  route({
    '/api/properties': () => listResponse(PROPERTIES, PROPERTIES.length),
    '/api/resource_templates': () => listResponse([], 0),
    '/api/romeka/field-values/3': () =>
      jsonResponse(census(3, state.placed ? [['Old City', 39]] : [['Bidar', 72], ['Old City', 39]])),
    '/api/romeka/field-values/40': () => jsonResponse(census(40, state.placed ? [['Bidar', 72]] : [])),
    '/api/romeka/field-values/move': (_url, init) => {
      const raw = typeof init?.body === 'string' ? init.body : '{}';
      const body: unknown = JSON.parse(raw);
      if (isRecord(body) && body['dry_run'] === false) {
        state.placed = body['to'] === 40;
      }
      return jsonResponse(moved);
    },
  });

afterEach(() => {
  vi.unstubAllGlobals();
  window.localStorage.clear();
});

describe('the values board', () => {
  it('checks a move, names the item type whose form lacks the field, moves, and undoes it', async () => {
    const state = { placed: false };
    const fetchMock = installFetch(routes(state));
    render(<FieldsPage param="3" />);

    expect(await screen.findByRole('heading', { name: 'Values of Subject' })).toBeTruthy();
    const place = await screen.findByRole('region', { name: 'Place' });
    expect(await within(place).findByText('Drop values here to move them to Place.')).toBeTruthy();

    fireEvent.change(await screen.findByRole('combobox', { name: 'Move “Bidar” to' }), {
      target: { value: '40' },
    });
    const check = await screen.findByRole('region', { name: 'Check the move' });
    expect(await within(check).findByText('Move “Bidar” from Subject to Place on 72 records.')).toBeTruthy();
    expect(within(check).getByText('1 of them already hold it under Place; they only lose it from Subject.')).toBeTruthy();
    expect(within(check).getByRole('link', { name: 'Add Place to Photo Narrative' }).getAttribute('href')).toBe(
      '#/resource-templates/7',
    );
    expect(state.placed).toBe(false);

    fireEvent.click(within(check).getByRole('button', { name: 'Move' }));
    expect(await screen.findByText(/Moved “Bidar” to Place on 72 records\./)).toBeTruthy();
    expect(await within(screen.getByRole('region', { name: 'Place' })).findByText('Bidar')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    await waitFor(() => {
      expect(state.placed).toBe(false);
    });
    const writes = requestsFor(fetchMock, '/api/romeka/field-values/move').map(requestBody);
    expect(writes.at(-1)).toEqual({ value: 'Bidar', from: 40, to: 3, dry_run: false, only: [10, 11], copy: [12] });
    expect(await screen.findByText(/Put “Bidar” back under Subject\./)).toBeTruthy();
  });

  it('opens no board for a field number that names no field', async () => {
    installFetch(routes({ placed: false }));
    render(<FieldsPage param="999" />);
    expect(await screen.findByText('No field has the number 999.')).toBeTruthy();
  });
});

describe('the way to the values board', () => {
  it('is a Values page under Describe, after Fields', () => {
    const describe = PAGE_GROUPS.find((group) => group.id === 'describe');
    expect(describe?.pages).toEqual(['resource-templates', 'term-lists', 'fields', 'values']);
  });

  it('lists the fields item types use, each opening its board, and finds any other field', async () => {
    installFetch(
      route({
        '/api/properties': () => listResponse(PROPERTIES, PROPERTIES.length),
        '/api/resource_templates': () => listResponse([template], 1),
      }),
    );
    render(<ValuesPage param={null} />);
    const used = await screen.findByRole('list', { name: 'Fields your item types use' });
    const subject = within(used).getByRole('link', { name: /^Subject/ });
    expect(subject.getAttribute('href')).toBe('#/values/3');
    expect(within(used).queryByRole('link', { name: /^Place/ })).toBeNull();

    fireEvent.change(screen.getByRole('searchbox', { name: 'Find another field' }), { target: { value: 'where' } });
    const found = await screen.findByRole('list', { name: 'Matches for “where”' });
    expect(within(found).getByRole('link', { name: /^Place/ }).getAttribute('href')).toBe('#/values/40');
  });

  it('opens the board of the field its number names', async () => {
    installFetch(routes({ placed: false }));
    render(<ValuesPage param="3" />);
    expect(await screen.findByRole('heading', { name: 'Values of Subject' })).toBeTruthy();
  });

  it('is a button on each Fields row', async () => {
    installFetch(
      route({
        '/api/properties': () => listResponse(PROPERTIES, PROPERTIES.length),
        '/api/resource_templates': () => listResponse([template], 1),
      }),
    );
    render(<FieldsPage param={null} />);
    const links = await screen.findAllByRole('link', { name: 'Its values' });
    expect(links.map((link) => link.getAttribute('href'))).toContain('#/values/3');
    expect(links[0]?.className).toContain('button-link');
  });
});

