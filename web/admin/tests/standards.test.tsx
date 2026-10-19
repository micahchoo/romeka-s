/**
 * Advanced > Standards: the three tabs, their counts, the "?" definitions, the
 * routes upstream's vocabulary pages used, and the vocabulary page that builds
 * a vocabulary in place. A stubbed fetch serves the fixtures, so every view
 * runs through the real client and parsers.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { parseHash } from '../src/app/router';
import { Shell } from '../src/app/shell';
import { StandardsPage } from '../src/pages/standards/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, requestBody, requestsFor, route } from './stub';

afterEach(() => {
  window.location.hash = '';
});

/** A handler whose list total depends on one query parameter. */
const totalBy =
  (param: string, totals: Readonly<Record<string, number>>) =>
  (url: URL): Response =>
    listResponse([], totals[url.searchParams.get(param) ?? ''] ?? 0);

/** The cells of the table row that holds `text`. */
const rowCells = (text: string): readonly string[] => {
  const row = screen.getByText(text).closest('tr');
  if (row === null) {
    throw new Error(`no row holds ${text}`);
  }
  return within(row)
    .getAllByRole('cell')
    .map((cell) => cell.textContent);
};

describe('standards routes', () => {
  it('upstream_vocabulary_routes_open_the_matching_standards_tab', () => {
    expect(parseHash('#/vocabularies')).toEqual({
      page: 'standards',
      param: 'vocabularies',
      rest: ['vocabularies'],
    });
    expect(parseHash('#/properties/12')).toEqual({
      page: 'standards',
      param: 'properties',
      rest: ['properties', '12'],
    });
    expect(parseHash('#/resource-classes/new')).toEqual({
      page: 'standards',
      param: 'classes',
      rest: ['classes', 'new'],
    });
  });

  it('the_nav_marks_standards_under_describe_for_an_upstream_route', async () => {
    window.location.hash = '#/properties/1';
    installFetch(() => jsonResponse({ errors: { error: 'offline' } }, { status: 503 }));
    render(<Shell />);

    const nav = screen.getByRole('navigation', { name: 'Admin pages' });
    const describe = within(nav).getByRole('heading', { name: 'Describe' }).closest('section');
    if (describe === null) {
      throw new Error('no Describe group');
    }
    expect(within(describe).getByRole('link', { name: 'Item types' })).toHaveAttribute(
      'href',
      '#/resource-templates',
    );
    expect(within(describe).getByRole('heading', { name: 'Advanced' })).toBeInTheDocument();
    expect(within(describe).getByRole('link', { name: 'Standards' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(await screen.findByRole('link', { name: 'Properties' })).toHaveAttribute(
      'aria-current',
      'true',
    );
  });
});

describe('standards tabs', () => {
  it('the_vocabularies_tab_counts_each_vocabularys_properties_and_classes', async () => {
    const fetchMock = installFetch(
      route({
        '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2),
        '/api/properties': totalBy('vocabulary_id', { '1': 55 }),
        '/api/resource_classes': totalBy('vocabulary_id', { '2': 12 }),
      }),
    );
    render(<StandardsPage />);

    expect(await screen.findByRole('link', { name: 'Dublin Core' })).toHaveAttribute(
      'href',
      '#/standards/vocabularies/1',
    );
    expect(screen.getByRole('link', { name: 'Vocabularies' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(rowCells('dcterms')).toEqual(['Dublin Core?', 'dcterms', 'http://purl.org/dc/terms/', '55', '0']);
    expect(rowCells('dctype').slice(3)).toEqual(['0', '12']);
    expect(requestsFor(fetchMock, '/api/properties')).toHaveLength(2);
  });

  it('the_properties_tab_shows_the_item_types_using_each_and_its_definition', async () => {
    installFetch(
      route({
        '/api/properties': () => listResponse(loadFixture('properties-page.json'), 2),
        '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2),
        '/api/resource_templates': () =>
          listResponse(loadFixture('resource-templates-page.json'), 2),
      }),
    );
    render(<StandardsPage rest={['properties']} />);

    expect(await screen.findByRole('link', { name: 'dcterms:title' })).toHaveAttribute(
      'href',
      '#/standards/properties/1',
    );
    expect(rowCells('dcterms:description').slice(1)).toEqual(['Description', 'Dublin Core', '1']);

    expect(screen.queryByText('A name given to the resource.')).toBeNull();
    const define = screen.getByRole('button', { name: 'What dcterms:title means' });
    fireEvent.click(define);
    expect(define).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('A name given to the resource.')).toBeInTheDocument();
    // A term with no stored comment has nothing to define.
    expect(screen.queryByRole('button', { name: 'What dcterms:description means' })).toBeNull();
  });

  it('the_classes_tab_counts_the_item_types_and_the_items_of_each_class', async () => {
    installFetch(
      route({
        '/api/resource_classes': () => listResponse(loadFixture('resource-classes-page.json'), 2),
        '/api/vocabularies': () => listResponse(loadFixture('vocabularies-page.json'), 2),
        '/api/resource_templates': () =>
          listResponse(loadFixture('resource-templates-page.json'), 2),
        '/api/items': totalBy('resource_class_id', { '13': 7 }),
      }),
    );
    render(<StandardsPage rest={['classes']} />);

    await screen.findByRole('link', { name: 'dctype:StillImage' });
    await waitFor(() => {
      expect(rowCells('dctype:StillImage').slice(3)).toEqual(['1', '7']);
    });
    await waitFor(() => {
      expect(rowCells('bibo:Book').slice(3)).toEqual(['0', '0']);
    });
  });

  it('an_unknown_tab_reports_no_such_page', () => {
    installFetch(() => jsonResponse({}));
    render(<StandardsPage rest={['nope']} />);
    expect(screen.getByRole('alert')).toHaveTextContent('No such page.');
  });
});

describe('vocabulary creator', () => {
  it('a_vocabulary_page_adds_a_property_to_itself_in_place', async () => {
    window.location.hash = '#/standards/vocabularies/3';
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/properties' && init?.method === 'POST') {
        return jsonResponse(loadFixture('property-1.json'));
      }
      return route({
        '/api/vocabularies/3': () => jsonResponse(loadFixture('vocabulary-3.json')),
        '/api/properties': () => listResponse(loadFixture('vocabulary-3-properties.json'), 2),
        '/api/resource_classes': () => listResponse(loadFixture('vocabulary-3-classes.json'), 2),
      })(url, init);
    });
    render(<StandardsPage rest={['vocabularies', '3']} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Add property', expanded: false }));
    fireEvent.change(screen.getByLabelText('Local name'), { target: { value: 'title' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'Title' } });
    const form = screen.getByLabelText('Local name').closest('form');
    if (form === null) {
      throw new Error('no add form');
    }
    fireEvent.submit(form);

    const post = await waitFor(() =>
      first(
        requestsFor(fetchMock, '/api/properties').filter((request) => request.init?.method === 'POST'),
      ),
    );
    expect(requestBody(post)['o:vocabulary']).toEqual({ 'o:id': 3 });
    expect(await screen.findByRole('link', { name: 'Title' })).toHaveAttribute(
      'href',
      '#/standards/properties/1',
    );
    expect(screen.getByRole('link', { name: 'Abstract' })).toBeInTheDocument();
    expect(window.location.hash).toBe('#/standards/vocabularies/3');
    // The form clears for the next member.
    expect(screen.getByLabelText('Local name')).toHaveValue('');
  });

  it('a_permanent_vocabulary_takes_no_new_members', async () => {
    installFetch(
      route({
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
        '/api/properties': () => listResponse([], 0),
        '/api/resource_classes': () => listResponse([], 0),
      }),
    );
    render(<StandardsPage rest={['vocabularies', '1']} />);

    expect(await screen.findByText('This vocabulary has no properties.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add property' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Add resource class' })).toBeNull();
  });
});
