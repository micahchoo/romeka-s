/**
 * Term lists in the admin: the `custom_vocabs` parser, the Term lists page, the
 * term picker. Every test runs through the real client and parsers against a
 * stubbed fetch.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { parseTermList } from '../src/api/parse';
import type { TermListName } from '../src/api/types';
import { TermPicker } from '../src/pages/term-lists/term-picker';
import type { TermChoice } from '../src/pages/term-lists/term-picker';
import { TermListsPage } from '../src/pages/term-lists/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, requestBody, requestsFor, route } from './stub';

const places = (overrides: Record<string, unknown> = {}): Record<string, unknown> => ({
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/custom_vocabs/3',
  '@type': 'o:CustomVocab',
  'o:id': 3,
  'o:label': 'Places',
  'o:lang': 'en',
  'o:terms': ['Cork', 'Dublin', 'Dún Laoghaire'],
  'o:uris': null,
  'o:item_set': null,
  'o:owner': { '@id': 'http://localhost/api/users/1', 'o:id': 1 },
  ...overrides,
});

const TERM_LIST: TermListName = (() => {
  const list = parseTermList(places());
  return list.dataType;
})();

describe('term list parser', () => {
  it('reads_the_source_by_the_module_precedence', () => {
    const terms = parseTermList(places());
    expect(terms.dataType).toBe('customvocab:3');
    expect(terms.source).toEqual({ kind: 'terms', terms: ['Cork', 'Dublin', 'Dún Laoghaire'] });

    const links = parseTermList(
      places({ 'o:uris': { 'http://www.wikidata.org/entity/Q36405': 'Cork', 'http://x/b': '' } }),
    );
    expect(links.source).toEqual({
      kind: 'uris',
      uris: [
        { uri: 'http://www.wikidata.org/entity/Q36405', label: 'Cork' },
        { uri: 'http://x/b', label: '' },
      ],
    });

    const items = parseTermList(
      places({ 'o:item_set': { '@id': 'http://localhost/api/item_sets/4', 'o:id': 4 } }),
    );
    expect(items.source.kind).toBe('items');

    expect(parseTermList(places({ 'o:terms': null })).source).toEqual({ kind: 'empty' });
  });
});

const PickerHarness = (): ReactElement => {
  const [picked, setPicked] = useState<TermChoice | null>(null);
  return (
    <>
      <TermPicker
        id="spatial"
        termList={TERM_LIST}
        value={picked?.value ?? ''}
        onPick={setPicked}
      />
      <output aria-label="picked">{picked === null ? 'none' : `${picked.kind}:${picked.value}`}</output>
    </>
  );
};

describe('term picker', () => {
  it('filters_as_the_user_types_and_picks_by_click_or_keyboard', async () => {
    installFetch(route({ '/api/custom_vocabs/3': () => jsonResponse(places()) }));
    render(<PickerHarness />);

    const input = await screen.findByRole('combobox', { name: 'Term list: Places' });
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'du' } });
    expect(screen.getAllByRole('option').map((option) => option.textContent)).toEqual([
      'Dublin',
      'Dún Laoghaire',
    ]);
    fireEvent.mouseDown(screen.getByRole('option', { name: 'Dublin' }));
    expect(screen.getByLabelText('picked')).toHaveTextContent('literal:Dublin');
    expect(input).toHaveValue('Dublin');

    fireEvent.change(input, { target: { value: 'co' } });
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(screen.getByLabelText('picked')).toHaveTextContent('literal:Cork');

    fireEvent.change(input, { target: { value: 'Paris' } });
    expect(screen.getByText('No entry matches "Paris".')).toBeInTheDocument();
    fireEvent.blur(input);
    expect(input).toHaveValue('Cork');
    expect(screen.getByLabelText('picked')).toHaveTextContent('literal:Cork');
  });

  it('says_when_the_stored_value_is_not_in_the_list', async () => {
    installFetch(route({ '/api/custom_vocabs/3': () => jsonResponse(places()) }));
    render(<TermPicker id="spatial" termList={TERM_LIST} value="Paris" onPick={() => undefined} />);
    expect(
      await screen.findByText('"Paris" is not in this list. Pick an entry to replace it.'),
    ).toBeInTheDocument();
  });
});

describe('term lists page', () => {
  it('lists_each_list_with_its_kind_entries_and_usage', async () => {
    const fetchMock = installFetch(
      route({
        '/api/custom_vocabs': () => listResponse([places()], 1),
        '/api/users': () => jsonResponse(loadFixture('users-page.json')),
        '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
        '/api/items': () => listResponse([], 7),
      }),
    );
    render(<TermListsPage />);

    const link = await screen.findByRole('link', { name: 'Places' });
    expect(link).toHaveAttribute('href', '#/term-lists/3');
    const row = link.closest('tr');
    expect(row?.textContent).toContain('A list of words');
    expect(row?.textContent).toContain('3');
    expect(row?.textContent).toContain('7');
    const usage = requestsFor(fetchMock, '/api/items')[0];
    expect(usage?.url.searchParams.get('property[0][property]')).toBe('');
    expect(usage?.url.searchParams.get('property[0][type]')).toBe('dt');
    expect(usage?.url.searchParams.get('property[0][text]')).toBe('customvocab:3');
  });

  it('creates_a_list_of_words_sending_only_that_source', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/custom_vocabs' && init?.method === 'POST') {
        return jsonResponse(places());
      }
      return route({ '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')) })(
        url,
        init,
      );
    });
    render(<TermListsPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Label'), { target: { value: 'Places' } });
    fireEvent.change(screen.getByLabelText('Words, one per line'), {
      target: { value: 'Cork\n\n Dublin \n' },
    });
    fireEvent.change(screen.getByLabelText('Language of the words'), { target: { value: 'en' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add term list' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/custom_vocabs')).toHaveLength(1);
    });
    const post = requestsFor(fetchMock, '/api/custom_vocabs')[0];
    if (post === undefined) {
      throw new Error('expected a POST');
    }
    expect(requestBody(post)).toEqual({
      'o:label': 'Places',
      'o:lang': 'en',
      'o:item_set': null,
      'o:terms': ['Cork', 'Dublin'],
      'o:uris': null,
    });
    window.location.hash = '';
  });

  it('sends_links_as_the_uri_to_label_map', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/custom_vocabs' && init?.method === 'POST') {
        return jsonResponse(places());
      }
      return route({ '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')) })(
        url,
        init,
      );
    });
    render(<TermListsPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Label'), { target: { value: 'Places' } });
    fireEvent.click(screen.getByLabelText('Links (URIs) with labels'));
    fireEvent.change(screen.getByLabelText('Links, one per line: the URI, a space, then the label'), {
      target: { value: 'http://www.wikidata.org/entity/Q36405 Cork city\nhttp://x/b' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add term list' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, '/api/custom_vocabs')).toHaveLength(1);
    });
    const post = requestsFor(fetchMock, '/api/custom_vocabs')[0];
    if (post === undefined) {
      throw new Error('expected a POST');
    }
    const body = requestBody(post);
    expect(body['o:uris']).toEqual({
      'http://www.wikidata.org/entity/Q36405': 'Cork city',
      'http://x/b': '',
    });
    expect(body['o:terms']).toBeNull();
    window.location.hash = '';
  });
});
