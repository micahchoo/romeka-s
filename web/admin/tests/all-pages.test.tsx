/**
 * The pickers and label lookups read every page of a long list. A database
 * with more classes or properties than one page holds must not lose the rest.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { loadFormOptions } from '../src/app/options';
import { ItemTypesPage } from '../src/pages/item-types/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, route } from './stub';

const PROPERTY_COUNT = 450;
const CLASS_COUNT = 105;

const member = (kind: 'properties' | 'resource_classes', id: number): Record<string, unknown> => ({
  '@id': `http://localhost/api/${kind}/${String(id)}`,
  '@type': kind === 'properties' ? 'o:Property' : 'o:ResourceClass',
  'o:id': id,
  'o:local_name': `n${String(id)}`,
  'o:label': `${kind === 'properties' ? 'Property' : 'Class'} ${String(id)}`,
  'o:comment': null,
  'o:term': `ex:n${String(id)}`,
  'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
});

/** One page of `count` members, cut the way `per_page` and `page` cut it. */
const paged =
  (kind: 'properties' | 'resource_classes', count: number) =>
  (url: URL): Response => {
    const perPage = Number(url.searchParams.get('per_page') ?? '25');
    const page = Number(url.searchParams.get('page') ?? '1');
    const ids = Array.from({ length: count }, (_, index) => index + 1).slice(
      (page - 1) * perPage,
      page * perPage,
    );
    return listResponse(
      ids.map((id) => member(kind, id)),
      count,
    );
  };

const empty = (): Response => listResponse([], 0);

describe('every page of a long list', () => {
  it('loads every property and class into the form options', async () => {
    installFetch(
      route({
        '/api/users': empty,
        '/api/resource_classes': paged('resource_classes', CLASS_COUNT),
        '/api/vocabularies': empty,
        '/api/resource_templates': empty,
        '/api/properties': paged('properties', PROPERTY_COUNT),
        '/api/item_sets': empty,
        '/api/data_types': empty,
      }),
    );
    const options = await loadFormOptions();
    expect(options.properties).toHaveLength(PROPERTY_COUNT);
    expect(options.properties.at(-1)?.term).toBe(`ex:n${String(PROPERTY_COUNT)}`);
    expect(options.classes).toHaveLength(CLASS_COUNT);
  });

  it('names the class of an item type whose class is past the first page', async () => {
    installFetch(
      route({
        '/api/resource_templates': () =>
          listResponse(
            [
              {
                '@id': 'http://localhost/api/resource_templates/1',
                '@type': 'o:ResourceTemplate',
                'o:id': 1,
                'o:label': 'Late class',
                'o:owner': null,
                'o:resource_class': {
                  '@id': `http://localhost/api/resource_classes/${String(CLASS_COUNT)}`,
                  'o:id': CLASS_COUNT,
                },
                'o:title_property': null,
                'o:description_property': null,
                'o:resource_template_property': [],
              },
            ],
            1,
          ),
        '/api/items': empty,
        '/api/resource_classes': paged('resource_classes', CLASS_COUNT),
      }),
    );
    render(<ItemTypesPage />);
    const row = (await screen.findByRole('link', { name: 'Late class' })).closest('tr');
    if (row === null) {
      throw new Error('expected the template row');
    }
    expect(within(row).getByText(`Class ${String(CLASS_COUNT)}`)).toBeInTheDocument();
  });

  it('offers every property and class in the item type editor', async () => {
    installFetch(
      route({
        '/api/users': empty,
        '/api/resource_classes': paged('resource_classes', CLASS_COUNT),
        '/api/properties': paged('properties', PROPERTY_COUNT),
        '/api/data_types': empty,
        '/api/romeka/starters': () => jsonResponse(loadFixture('starters.json')),
      }),
    );
    render(<ItemTypesPage param="new" />);
    fireEvent.change(await screen.findByLabelText('What kind of thing is this?'), {
      target: { value: 'Spoon' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    const last = String(PROPERTY_COUNT);
    fireEvent.change(await screen.findByRole('searchbox', { name: 'Add a field' }), {
      target: { value: `ex:n${last}` },
    });
    const fields = screen.getByRole('list', { name: `Matches for “ex:n${last}”` });
    expect(within(fields).getAllByRole('button')[0]).toHaveTextContent(`ex:n${last}`);
    fireEvent.click(screen.getByRole('button', { name: 'Change the standard class' }));
    fireEvent.change(screen.getByRole('searchbox', { name: 'Find a standard class' }), {
      target: { value: `Class ${String(CLASS_COUNT)}` },
    });
    const classes = screen.getByRole('list', { name: `Matches for “Class ${String(CLASS_COUNT)}”` });
    expect(within(classes).getAllByRole('button')[0]).toHaveTextContent(`ex:n${String(CLASS_COUNT)}`);
  });
});
