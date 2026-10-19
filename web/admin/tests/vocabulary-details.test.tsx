/**
 * The vocabulary family detail views. A stubbed fetch serves the fixtures, so
 * each page runs through the real typed client and its parsers. No test touches
 * the network.
 */
import { render, screen, within } from '@testing-library/react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { PropertiesPage } from '../src/pages/properties/page';
import { ResourceClassesPage } from '../src/pages/resource-classes/page';
import { ResourceTemplatesPage } from '../src/pages/resource-templates/page';
import { VocabulariesPage } from '../src/pages/vocabularies/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, route, urlsFor } from './stub';

/** The row a detail table cell belongs to, found by the link in its first cell. */
const rowOf = (label: string): HTMLElement => {
  const rows = screen.getAllByRole('link', { name: label }).flatMap((link) => {
    const row = link.closest('tr');
    return row === null ? [] : [row];
  });
  const row = rows[0];
  if (row === undefined) {
    throw new Error(`no table row for ${label}`);
  }
  return row;
};

/** The value of a definition row, found by its term. */
const definitionOf = (term: string): HTMLElement => {
  const value = screen.getByText(term).nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`no definition value for ${term}`);
  }
  return value;
};

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
      '#/standards/properties/60',
    );
    expect(screen.getByRole('link', { name: 'Book' })).toHaveAttribute(
      'href',
      '#/standards/classes/51',
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
      '#/standards/vocabularies/1',
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
      '#/standards/vocabularies/2',
    );
  });
});

describe('resource template detail', () => {
  const routes = {
    '/api/resource_templates/2': () => jsonResponse(loadFixture('resource-template-2.json')),
    '/api/properties': () => listResponse(loadFixture('template-properties.json'), 3),
    '/api/users': () => listResponse(loadFixture('users-page.json'), 2),
    '/api/resource_classes': () => listResponse(loadFixture('resource-classes-page.json'), 2),
  };

  it('resource_template_detail_shows_every_member_and_links_its_references', async () => {
    installFetch(route(routes));
    render(<ResourceTemplatesPage param="2" />);

    expect(await screen.findByRole('heading', { name: 'Photograph' })).toBeInTheDocument();
    expect(within(definitionOf('Owner')).getByRole('link', { name: 'Ada Lovelace' })).toHaveAttribute(
      'href',
      '#/users/3',
    );
    expect(
      within(definitionOf('Class')).getByRole('link', { name: 'Still Image' }),
    ).toHaveAttribute('href', '#/resource-classes/13');
    expect(within(definitionOf('Title property')).getByRole('link', { name: 'Title' })).toHaveAttribute(
      'href',
      '#/properties/1',
    );
    expect(
      within(definitionOf('Description property')).getByRole('link', { name: 'Description' }),
    ).toHaveAttribute('href', '#/properties/4');
  });

  it('resource_template_detail_shows_each_template_property_member', async () => {
    installFetch(route(routes));
    render(<ResourceTemplatesPage param="2" />);
    await screen.findByRole('heading', { name: 'Photograph' });

    const required = rowOf('Title');
    expect(within(required).getByText('literal')).toBeInTheDocument();
    expect(within(required).getByText('Yes')).toBeInTheDocument();
    expect(within(required).getByText('No')).toBeInTheDocument();

    const captioned = rowOf('Description');
    expect(within(captioned).getByText('Caption')).toBeInTheDocument();
    expect(within(captioned).getByText('en')).toBeInTheDocument();

    const related = rowOf('Relation');
    expect(within(related).getByText('resource:item')).toBeInTheDocument();
  });
});

describe('vocabulary family browse rows', () => {
  const pages: readonly (readonly [string, () => ReactElement, Readonly<Record<string, () => Response>>])[] =
    [
      [
        'resource templates',
        () => <ResourceTemplatesPage />,
        {
          '/api/resource_templates': () =>
            listResponse(loadFixture('resource-templates-page.json'), 2),
          '/api/users': () => listResponse(loadFixture('users-page.json'), 2),
          '/api/resource_classes': () => listResponse(loadFixture('resource-classes-page.json'), 2),
        },
      ],
    ];

  it.each(pages)('%s list links a row to its detail view', async (name, Page, handlers) => {
    installFetch(route(handlers));
    render(<Page />);

    const links: Readonly<Record<string, readonly [string, string]>> = {
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
      () => <ResourceTemplatesPage param="abc" />,
      '"abc" is not a valid resource template id.',
    ],
  ] as const)('%s detail reports an unparseable id', (_name, Page, message) => {
    installFetch(() => jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 }));
    render(<Page />);

    expect(screen.getByRole('alert')).toHaveTextContent(message);
  });
});
