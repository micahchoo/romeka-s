/**
 * The values board (`#/fields/<id>`): a value moves to another field only after the server's dry
 * run says what will change, and the last move of the visit can be undone exactly.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isRecord } from '../src/api/parse';
import { FieldsPage } from '../src/pages/fields/page';
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
