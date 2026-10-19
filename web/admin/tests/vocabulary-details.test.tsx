/**
 * The vocabulary family detail views. A stubbed fetch serves the fixtures, so
 * each page runs through the real typed client and its parsers. No test touches
 * the network.
 */
import { render, screen } from '@testing-library/react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { PropertiesPage } from '../src/pages/properties/page';
import { ResourceClassesPage } from '../src/pages/resource-classes/page';
import { ItemTypesPage } from '../src/pages/item-types/page';
import { VocabulariesPage } from '../src/pages/vocabularies/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, route, urlsFor } from './stub';

/** The row a detail table cell belongs to, found by the link in its first cell. */

/** The value of a definition row, found by its term. */

describe('vocabulary detail', () => {
  it('vocabulary_detail_shows_every_member_and_links_its_properties_and_classes', async () => {
    const fetchMock = installFetch(
      route({
        '/api/vocabularies/3': () => jsonResponse(loadFixture('vocabulary-3.json')),
        '/api/properties': () => listResponse(loadFixture('vocabulary-3-properties.json'), 2),
        '/api/resource_classes': () => listResponse(loadFixture('vocabulary-3-classes.json'), 2),
      }),
    );
    render(<VocabulariesPage param="3" />);

    expect(
      await screen.findByRole('heading', { name: 'Bibliographic Ontology' }),
    ).toBeInTheDocument();
    expect(screen.getByText('bibo')).toBeInTheDocument();
    expect(screen.getByText('http://purl.org/ontology/bibo/')).toBeInTheDocument();
    expect(screen.getByText('The Bibliographic Ontology.')).toBeInTheDocument();
    expect(screen.getByText('[No owner]')).toBeInTheDocument();

    expect(screen.getByRole('link', { name: 'Abstract' })).toHaveAttribute(
      'href',
      '#/properties/60',
    );
    expect(screen.getByRole('link', { name: 'Book' })).toHaveAttribute(
      'href',
      '#/resource-classes/51',
    );
    expect(screen.getByText('bibo:abstract')).toBeInTheDocument();
    expect(screen.getByText('bibo:AcademicArticle')).toBeInTheDocument();

    // The members come from the API's `vocabulary_id` filter, not from every
    // row of the two collections.
    expect(first(urlsFor(fetchMock, '/api/properties')).searchParams.get('vocabulary_id')).toBe('3');
    expect(
      first(urlsFor(fetchMock, '/api/resource_classes')).searchParams.get('vocabulary_id'),
    ).toBe('3');
  });

  it('vocabulary_detail_reports_an_empty_member_list', async () => {
    installFetch(
      route({
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
        '/api/properties': () => listResponse([], 0),
        '/api/resource_classes': () => listResponse([], 0),
      }),
    );
    render(<VocabulariesPage param="1" />);

    expect(await screen.findByText('Dublin Core')).toBeInTheDocument();
    expect(screen.getByText('This vocabulary has no properties.')).toBeInTheDocument();
    expect(screen.getByText('This vocabulary has no classes.')).toBeInTheDocument();
  });
});

describe('property detail', () => {
  it('property_detail_shows_every_member_and_links_its_vocabulary', async () => {
    installFetch(
      route({
        '/api/properties/1': () => jsonResponse(loadFixture('property-1.json')),
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
      }),
    );
    render(<PropertiesPage param="1" />);

    expect(await screen.findByRole('heading', { name: 'Title' })).toBeInTheDocument();
    expect(screen.getByText('dcterms:title')).toBeInTheDocument();
    expect(screen.getByText('title')).toBeInTheDocument();
    expect(screen.getByText('A name given to the resource.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Dublin Core' })).toHaveAttribute(
      'href',
      '#/vocabularies/1',
    );
  });
});

describe('resource class detail', () => {
  it('resource_class_detail_shows_every_member_and_links_its_vocabulary', async () => {
    installFetch(
      route({
        '/api/resource_classes/13': () => jsonResponse(loadFixture('resource-class-13.json')),
        '/api/vocabularies/2': () => jsonResponse(loadFixture('vocabulary-2.json')),
      }),
    );
    render(<ResourceClassesPage param="13" />);

    expect(await screen.findByRole('heading', { name: 'Still Image' })).toBeInTheDocument();
    expect(screen.getByText('dctype:StillImage')).toBeInTheDocument();
    expect(screen.getByText('StillImage')).toBeInTheDocument();
    expect(screen.getByText('[None]')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Dublin Core Type' })).toHaveAttribute(
      'href',
      '#/vocabularies/2',
    );
  });
});

describe('vocabulary family browse rows', () => {
  const pages: readonly (readonly [string, () => ReactElement, Readonly<Record<string, () => Response>>])[] =
    [
      [
        'vocabularies',
        () => <VocabulariesPage />,
        { '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2) },
      ],
      [
        'properties',
        () => <PropertiesPage />,
        {
          '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
          '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2),
        },
      ],
      [
        'resource classes',
        () => <ResourceClassesPage />,
        {
          '/api/resource_classes': () => listResponse(loadFixture('resource-classes-page.json'), 2),
          '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2),
        },
      ],
      [
        'resource templates',
        () => <ItemTypesPage />,
        {
          '/api/resource_templates': () =>
            listResponse(loadFixture('resource-templates-page.json'), 2),
          '/api/items': () => listResponse([], 0),
          '/api/resource_classes': () => listResponse(loadFixture('resource-classes-page.json'), 2),
        },
      ],
    ];

  it.each(pages)('%s list links a row to its detail view', async (name, Page, handlers) => {
    installFetch(route(handlers));
    render(<Page />);

    const links: Readonly<Record<string, readonly [string, string]>> = {
      vocabularies: ['Dublin Core', '#/vocabularies/1'],
      properties: ['dcterms:title', '#/properties/1'],
      'resource classes': ['dctype:StillImage', '#/resource-classes/13'],
      'resource templates': ['Photograph', '#/resource-templates/2'],
    };
    const entry = links[name];
    if (entry === undefined) {
      throw new Error(`no expected link for ${name}`);
    }
    expect(await screen.findByRole('link', { name: entry[0] })).toHaveAttribute('href', entry[1]);
  });

  it.each([
    ['vocabularies', () => <VocabulariesPage param="abc" />, '"abc" is not a valid vocabulary id.'],
    ['properties', () => <PropertiesPage param="abc" />, '"abc" is not a valid property id.'],
    [
      'resource classes',
      () => <ResourceClassesPage param="abc" />,
      '"abc" is not a valid resource class id.',
    ],
    [
      'resource templates',
      () => <ItemTypesPage param="abc" />,
      '"abc" is not a valid resource template id.',
    ],
  ] as const)('%s detail reports an unparseable id', (_name, Page, message) => {
    installFetch(() => jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 }));
    render(<Page />);

    expect(screen.getByRole('alert')).toHaveTextContent(message);
  });
});
