/**
 * The Publish panel: a draft list publishes at the address the panel sends, a
 * published term deprecates, a persistent namespace and a link are checked
 * before they are sent, and a vocabulary that is not this archive's says why.
 * Every test runs through the real client and parser against a stubbed fetch.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { brandId, isRecord } from '../src/api/parse';
import { PublishPanel } from '../src/pages/publish/panel';
import { installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';

const PATH = '/api/custom_vocabs/3/publication';

const term = (key: string, uri: string, overrides: Record<string, unknown> = {}): Record<string, unknown> => ({
  key,
  label: key,
  uri,
  published: false,
  deprecated: false,
  deprecation_published: false,
  exact_match: null,
  close_match: null,
  ...overrides,
});

const draft = (overrides: Record<string, unknown> = {}): Record<string, unknown> => ({
  state: 'draft',
  kind: 'term_list',
  refusal: null,
  slug: 'irish-places',
  slug_fixed: false,
  namespace: 'http://localhost/vocab/irish-places/',
  namespace_fixed: false,
  url: 'http://localhost/vocab/irish-places/',
  versions: [],
  changed: true,
  terms: [
    term('Cork', 'http://localhost/vocab/irish-places/1'),
    term('Dublin', 'http://localhost/vocab/irish-places/2'),
  ],
  preview: '<http://localhost/vocab/irish-places/1> a skos:Concept .',
  ...overrides,
});

const published = (overrides: Record<string, unknown> = {}): Record<string, unknown> =>
  draft({
    state: 'published',
    slug: 'places',
    slug_fixed: true,
    namespace: 'http://localhost/vocab/places/',
    namespace_fixed: true,
    url: 'http://localhost/vocab/places/',
    versions: [{ version: 1, created: '2026-10-18T09:30:00', terms: 2 }],
    changed: false,
    terms: [
      term('Cork', 'http://localhost/vocab/places/1', { published: true }),
      term('Dublin', 'http://localhost/vocab/places/2', { published: true }),
    ],
    ...overrides,
  });

/** A stub that answers the GET with `first`, and every POST with `after(body)`. */
const stub = (first: Record<string, unknown>, after: (body: Record<string, unknown>) => Record<string, unknown>) =>
  installFetch(
    route({
      [PATH]: (_url, init) => {
        if (init?.method !== 'POST') {
          return jsonResponse(first);
        }
        const body: unknown = JSON.parse(typeof init.body === 'string' ? init.body : '{}');
        return jsonResponse(after(isRecord(body) ? body : {}));
      },
    }),
  );

const renderPanel = (): void => {
  render(<PublishPanel target={{ kind: 'term_list', id: brandId<'TermListId'>(3) }} />);
};

describe('publish panel', () => {
  it('publishes_a_draft_at_the_address_name_the_admin_chose', async () => {
    const fetchMock = stub(draft(), () => published());
    renderPanel();

    expect(await screen.findByText('Draft. Not published yet.')).toBeTruthy();
    expect(screen.getByText('http://localhost/vocab/irish-places/2')).toBeTruthy();
    const slug = screen.getByLabelText('Address name');
    fireEvent.change(slug, { target: { value: 'Irish Places' } });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));
    expect((await screen.findByRole('alert')).textContent).toBe(
      'Use lowercase letters, digits and single hyphens, up to 64 characters.',
    );
    expect(requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST')).toEqual([]);

    fireEvent.change(slug, { target: { value: 'places' } });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));

    expect(await screen.findByText('Published version 1.')).toBeTruthy();
    const posts = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST');
    expect(posts.map(requestBody)).toEqual([{ action: 'publish', slug: 'places' }]);
    expect(screen.getByText(/^Published\. Version 1, Oct 18, 2026, 9:30\s?AM\.$/)).toBeTruthy();
    expect(screen.getByRole('link', { name: 'http://localhost/vocab/places/' })).toBeTruthy();
  });

  it('sends_a_persistent_namespace_only_when_it_is_at_w3id_or_purl', async () => {
    const fetchMock = stub(draft(), () => published());
    renderPanel();

    fireEvent.click(await screen.findByRole('radio', { name: /A persistent address/ }));
    const namespace = screen.getByLabelText('Persistent namespace');
    fireEvent.change(namespace, { target: { value: 'https://example.org/places/' } });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));
    expect((await screen.findByRole('alert')).textContent).toBe(
      'Use an address at w3id.org or purl.org that ends with / or #.',
    );
    expect(screen.getByText(/RewriteRule \^\(\.\*\)\$ http:\/\/localhost\/vocab\/irish-places\//)).toBeTruthy();

    fireEvent.change(namespace, { target: { value: 'https://w3id.org/my-archive/places/' } });
    fireEvent.click(screen.getByRole('button', { name: 'Publish' }));

    await waitFor(() => {
      expect(requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST')).toHaveLength(1);
    });
    const [post] = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST');
    expect(post === undefined ? null : requestBody(post)).toEqual({
      action: 'publish',
      slug: 'irish-places',
      namespace: 'https://w3id.org/my-archive/places/',
    });
  });

  it('deprecates_a_published_term_and_waits_for_a_change_before_publishing', async () => {
    const fetchMock = stub(published(), () =>
      published({
        changed: true,
        terms: [
          term('Cork', 'http://localhost/vocab/places/1', { published: true, deprecated: true }),
          term('Dublin', 'http://localhost/vocab/places/2', { published: true }),
        ],
      }),
    );
    renderPanel();

    expect(await screen.findByText('Nothing changed since version 1.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Publish version 2' }).hasAttribute('disabled')).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Deprecate Cork' }));

    expect(await screen.findByText('Deprecated from the next version')).toBeTruthy();
    const posts = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST');
    expect(posts.map(requestBody)).toEqual([{ action: 'deprecate', key: 'Cork' }]);
    expect(screen.getByRole('button', { name: 'Keep Cork in use' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Publish version 2' }).hasAttribute('disabled')).toBe(false);
  });

  it('checks_a_link_before_it_sends_it', async () => {
    const fetchMock = stub(published(), () => published());
    renderPanel();

    const form = await screen.findByRole('form', { name: 'Links of Cork to an authority' });
    const exact = within(form).getByLabelText('The same term elsewhere (exactMatch)');
    fireEvent.change(exact, { target: { value: 'geonames 2965140' } });
    fireEvent.submit(form);
    expect(within(form).getByRole('alert').textContent).toBe(
      'Use an address that starts with http:// or https://.',
    );

    fireEvent.change(exact, { target: { value: 'https://sws.geonames.org/2965140/' } });
    fireEvent.submit(form);

    await waitFor(() => {
      expect(requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST')).toHaveLength(1);
    });
    const [post] = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'POST');
    expect(post === undefined ? null : requestBody(post)).toEqual({
      action: 'link',
      key: 'Cork',
      exact_match: 'https://sws.geonames.org/2965140/',
      close_match: null,
    });
  });

  it('says_why_a_vocabulary_that_is_not_this_archive_cannot_be_published', async () => {
    const refusal = 'The namespace http://purl.org/dc/terms/ is not this archive\'s.';
    installFetch(
      route({
        '/api/vocabularies/1/publication': () =>
          jsonResponse(
            draft({ kind: 'vocabulary', refusal, slug: '', namespace: '', url: '', terms: [], preview: '' }),
          ),
      }),
    );
    render(<PublishPanel target={{ kind: 'vocabulary', id: brandId<'VocabularyId'>(1) }} />);

    expect(await screen.findByText(refusal)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Publish' })).toBeNull();
  });
});
