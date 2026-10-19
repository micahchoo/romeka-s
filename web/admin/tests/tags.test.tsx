/**
 * Tags in the admin: the Tags field on the item form, and the Term lists
 * page's Tags panel with its promotion and merge flows. Every test runs
 * through the real client and parsers against a stubbed fetch.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord, isUnknownArray } from '../src/api/parse';
import { ItemsPage } from '../src/pages/items/page';
import { TermListsPage } from '../src/pages/term-lists/page';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, listResponse, requestBody, requestsFor } from './stub';
import type { CapturedRequest } from './stub';

const record = (value: unknown): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new Error('expected a record');
  }
  return value;
};

const SUBJECT = {
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/properties/40',
  '@type': ['o:Property'],
  'o:id': 40,
  'o:local_name': 'subject',
  'o:label': 'Subject',
  'o:comment': 'The topic of the resource.',
  'o:term': 'dcterms:subject',
  'o:vocabulary': { '@id': 'http://localhost/api/vocabularies/1', 'o:id': 1 },
};

const properties = (): unknown[] => {
  const page = loadFixture('properties-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected a properties page');
  }
  return [...page, SUBJECT];
};

/** The item fixture with a tag beside its URI subject heading. */
const taggedItem = (): Record<string, unknown> => {
  const item = record(loadFixture('item-1.json'));
  const subjects = item['dcterms:subject'];
  return {
    ...item,
    'dcterms:subject': [
      ...(isUnknownArray(subjects) ? subjects : []),
      { type: 'literal', property_id: 40, property_label: 'Subject', is_public: true, '@value': 'emigration' },
    ],
  };
};

const census = (values: readonly (readonly [string, number])[], distinct: string[][] = []) => ({
  values: values.map(([value, resources]) => ({ value, resources, editable: resources })),
  distinct,
});

const itemRoutes = (url: URL): Response => {
  switch (url.pathname) {
    case '/api/items/1':
      return jsonResponse(taggedItem());
    case '/api/media':
      return listResponse([], 0);
    case '/api/properties':
      return jsonResponse(properties());
    case '/api/users':
      return jsonResponse(loadFixture('users-page.json'));
    case '/api/resource_classes':
      return jsonResponse(loadFixture('resource-classes-page.json'));
    case '/api/resource_templates':
      return jsonResponse(loadFixture('resource-templates-page.json'));
    case '/api/item_sets':
      return jsonResponse(loadFixture('item-sets-page.json'));
    case '/api/data_types':
      return jsonResponse(loadFixture('data-types-page.json'));
    case '/api/tags':
      return jsonResponse(census([['emigration', 14], ['wartime', 5], ['christmas', 6]]));
    case '/api/sites':
      return jsonResponse(loadFixture('sites-page.json'));
    default:
      return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
  }
};

const posts = (fetchMock: ReturnType<typeof installFetch>, path: string): CapturedRequest[] =>
  requestsFor(fetchMock, path).filter((request) => request.init?.method === 'POST');

describe('Tags field', () => {
  it('shows_plain_subject_text_as_tags_and_writes_new_tags_beside_the_heading', async () => {
    const fetchMock = installFetch(itemRoutes);
    render(<ItemsPage param="1" />);
    fireEvent.click(await screen.findByText('Edit this item'));

    const tags = await screen.findByRole('group', { name: 'Tags' });
    expect(within(tags).getByText('emigration')).toBeInTheDocument();
    expect(screen.queryByDisplayValue('emigration')).toBeNull();
    expect(screen.getByDisplayValue('http://example.org/subjects/fixtures')).toBeInTheDocument();

    const entry = within(tags).getByLabelText('Add a tag');
    fireEvent.change(entry, { target: { value: 'wartime' } });
    fireEvent.keyDown(entry, { key: 'Enter' });
    fireEvent.change(entry, { target: { value: 'the farm, emigration,' } });
    expect(within(tags).getAllByRole('listitem').map((item) => item.textContent)).toEqual([
      'emigration×',
      'wartime×',
      'the farm×',
    ]);
    fireEvent.click(within(tags).getByRole('button', { name: 'Remove the tag “wartime”' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/items/1').filter((request) => request.init?.method === 'PUT'),
      ).toHaveLength(1);
    });
    const put = requestsFor(fetchMock, '/api/items/1').find(
      (request) => request.init?.method === 'PUT',
    );
    if (put === undefined) {
      throw new Error('expected a PUT');
    }
    expect(requestBody(put)['dcterms:subject']).toEqual([
      {
        property_id: 4,
        type: 'uri',
        '@id': 'http://example.org/subjects/fixtures',
        'o:label': 'Fixtures',
        is_public: true,
      },
      { property_id: 40, type: 'literal', '@value': 'emigration', is_public: true },
      { property_id: 40, type: 'literal', '@value': 'the farm', is_public: true },
    ]);
  });

  it('suggests_the_tags_already_in_the_archive_once_the_entry_is_focused', async () => {
    const fetchMock = installFetch(itemRoutes);
    render(<ItemsPage param="1" />);
    fireEvent.click(await screen.findByText('Edit this item'));
    const entry = await screen.findByLabelText('Add a tag');
    expect(requestsFor(fetchMock, '/api/tags')).toHaveLength(0);

    fireEvent.focus(entry);
    const list = entry.getAttribute('list');
    await waitFor(() => {
      const options = document.getElementById(list ?? '')?.querySelectorAll('option') ?? [];
      expect([...options].map((option) => option.value)).toEqual(['wartime', 'christmas']);
    });
  });
});

const listPageRoutes =
  (tags: () => Response) =>
  (url: URL, init: RequestInit | undefined): Response => {
    switch (url.pathname) {
      case '/api/custom_vocabs':
        return listResponse([], 0);
      case '/api/users':
        return jsonResponse(loadFixture('users-page.json'));
      case '/api/item_sets':
        return jsonResponse(loadFixture('item-sets-page.json'));
      case '/api/tags':
        return tags();
      case '/api/tags/promote': {
        const body = requestBody({ url, init });
        const created = body.dry_run !== true;
        return jsonResponse({
          list: created ? { 'o:id': 7, 'o:label': 'Topics' } : null,
          created: true,
          changed: 19,
          skipped: 2,
          templates: [{ 'o:id': 2, 'o:label': 'Letter' }],
          templates_skipped: [],
        });
      }
      case '/api/tags/merge':
        return jsonResponse({ changed: 3, skipped: 1 });
      case '/api/tags/distinct':
        return jsonResponse(census([['Cork', 6], ['Co. Cork', 3]], [['Co. Cork', 'Cork']]));
      default:
        return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    }
  };

describe('Tags panel', () => {
  it('previews_a_promotion_then_promotes_the_chosen_tags', async () => {
    const fetchMock = installFetch(
      listPageRoutes(() =>
        jsonResponse(census([['emigration', 14], ['wartime', 5], ['christmas', 6]])),
      ),
    );
    render(<TermListsPage />);

    const panel = await screen.findByRole('complementary', { name: 'Tags' });
    fireEvent.click(await within(panel).findByRole('checkbox', { name: /emigration/ }));
    fireEvent.click(within(panel).getByRole('checkbox', { name: /wartime/ }));
    const promote = within(panel).getByRole('button', { name: 'Promote to list' });
    expect(promote).toBeDisabled();
    fireEvent.change(within(panel).getByLabelText('List name'), { target: { value: ' Topics ' } });
    fireEvent.click(promote);

    const preview = await within(panel).findByRole('region', { name: 'Promote to “Topics”' });
    expect(within(preview).getByText('This makes a new list.')).toBeInTheDocument();
    expect(within(preview).getByText('19')).toBeInTheDocument();
    expect(within(preview).getByText('2')).toBeInTheDocument();
    expect(within(preview).getByText('Letter')).toBeInTheDocument();
    expect(requestBody(first(posts(fetchMock, '/api/tags/promote')))).toEqual({
      tags: ['emigration', 'wartime'],
      list: 'Topics',
      dry_run: true,
    });

    fireEvent.click(within(preview).getByRole('button', { name: 'Promote to list' }));
    expect(
      await within(panel).findByText('Promoted to “Topics”. Resources changed: 19.'),
    ).toBeInTheDocument();
    const sent = posts(fetchMock, '/api/tags/promote');
    expect(sent).toHaveLength(2);
    expect(requestBody(sent.at(-1) ?? first(sent)).dry_run).toBe(false);
  });

  it('offers_to_merge_alike_tags_and_previews_the_count', async () => {
    const fetchMock = installFetch(
      listPageRoutes(() => jsonResponse(census([['Cork', 6], ['Co. Cork', 3], ['Boston', 9]]))),
    );
    render(<TermListsPage />);

    const prompt = await screen.findByRole('region', { name: 'Similar spellings' });
    expect(within(prompt).getByRole('radio', { name: /Keep “Cork”/ })).toBeChecked();
    fireEvent.click(within(prompt).getByRole('radio', { name: /Keep “Co. Cork”/ }));
    fireEvent.click(within(prompt).getByRole('button', { name: 'Merge into “Co. Cork”' }));

    expect(await within(prompt).findByText('Resources that change')).toBeInTheDocument();
    expect(within(prompt).getByText('3')).toBeInTheDocument();
    expect(requestBody(first(posts(fetchMock, '/api/tags/merge')))).toEqual({ from: ['Cork'], to: 'Co. Cork', dry_run: true });

    fireEvent.click(within(prompt).getByRole('button', { name: 'Merge' }));
    expect(
      await screen.findByText('Merged into “Co. Cork”. Resources changed: 3.'),
    ).toBeInTheDocument();
    expect(posts(fetchMock, '/api/tags/merge')).toHaveLength(2);
  });

  it('remembers_that_alike_tags_are_different', async () => {
    const fetchMock = installFetch(
      listPageRoutes(() => jsonResponse(census([['Cork', 6], ['Co. Cork', 3]]))),
    );
    render(<TermListsPage />);

    const prompt = await screen.findByRole('region', { name: 'Similar spellings' });
    fireEvent.click(within(prompt).getByRole('button', { name: 'They are different' }));
    expect(
      await screen.findByText('Noted. These spellings will not be offered for merging again.'),
    ).toBeInTheDocument();
    expect(requestBody(first(posts(fetchMock, '/api/tags/distinct')))).toEqual({ values: ['Cork', 'Co. Cork'] });
  });
});
