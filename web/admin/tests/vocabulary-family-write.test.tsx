/**
 * Write tests for the vocabulary family: a vocabulary, a property, a resource
 * class. (The resource template editor is `item-types.test.tsx`.) Each form runs through the real client and
 * parsers against a stubbed fetch, so the method, URL and body a form sends are
 * the bytes the server would get, and every rendered validation error is the
 * API's own message or the form's own check.
 */
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Mock } from 'vitest';
import { PropertiesPage } from '../src/pages/properties/page';
import { ResourceClassesPage } from '../src/pages/resource-classes/page';
import { VocabulariesPage } from '../src/pages/vocabularies/page';
import { loadFixture } from './loadFixture';
import {
  detached,
  first,
  installFetch,
  jsonResponse,
  requestBody,
  requestsFor,
  type CapturedRequest,
  type Handler,
} from './stub';

afterEach(() => {
  window.location.hash = '';
});

const notFound = (): Response =>
  jsonResponse({ errors: { error: 'Unexpected request' } }, { status: 404 });

/** The reference lists every vocabulary-family form loads. */
const referenceRoutes: Readonly<Record<string, () => Response>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  '/api/vocabularies': () => jsonResponse(loadFixture('vocabularies-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
};

const withFixture =
  (routes: Readonly<Record<string, Handler>>): Handler =>
  (url, init) => {
    const handler = routes[url.pathname];
    return handler === undefined ? notFound() : handler(url, init);
  };

const sent = (
  fetchMock: Mock<typeof fetch>,
  pathname: string,
  method: string,
): CapturedRequest =>
  first(requestsFor(fetchMock, pathname).filter((request) => request.init?.method === method));

describe('vocabulary create', () => {
  it('vocabulary_create_posts_the_scalar_members_and_no_member_list', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/vocabularies': () => jsonResponse(loadFixture('vocabulary-3.json')),
      }),
    );
    render(<VocabulariesPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Namespace URI'), {
      target: { value: 'http://example.org/ns/' },
    });
    fireEvent.change(screen.getByLabelText('Prefix'), { target: { value: 'ex' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'Example' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add vocabulary' }));

    const post = await waitFor(() => sent(fetchMock, '/api/vocabularies', 'POST'));
    const body = requestBody(post);
    expect(body['o:namespace_uri']).toBe('http://example.org/ns/');
    expect(body['o:prefix']).toBe('ex');
    expect(body['o:label']).toBe('Example');
    expect(body['o:comment']).toBeNull();
    expect(body['o:owner']).toBeUndefined();
    expect(body['o:class']).toBeUndefined();
    expect(body['o:property']).toBeUndefined();
  });

  it('vocabulary_create_refuses_an_empty_label_before_any_request', async () => {
    const fetchMock = installFetch(withFixture(referenceRoutes));
    render(<VocabulariesPage param="new" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Add vocabulary' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('The label cannot be empty.');
    expect(requestsFor(fetchMock, '/api/vocabularies')).toHaveLength(0);
  });

  it('vocabulary_create_renders_the_api_validation_message', async () => {
    installFetch((url, init) => {
      if (url.pathname === '/api/vocabularies' && init?.method === 'POST') {
        return jsonResponse(
          { errors: { 'o:prefix': ['The prefix "bibo" is already taken.'] } },
          { status: 422 },
        );
      }
      return withFixture(referenceRoutes)(url, init);
    });
    render(<VocabulariesPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Namespace URI'), {
      target: { value: 'http://purl.org/ontology/bibo/' },
    });
    fireEvent.change(screen.getByLabelText('Prefix'), { target: { value: 'bibo' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'Bibliographic' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add vocabulary' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The prefix "bibo" is already taken. (HTTP 422)',
    );
  });
});

describe('vocabulary edit and delete', () => {
  const detailRoutes: Readonly<Record<string, () => Response>> = {
    ...referenceRoutes,
    '/api/vocabularies/3': () => jsonResponse(loadFixture('vocabulary-3.json')),
    '/api/properties': () => jsonResponse(loadFixture('vocabulary-3-properties.json')),
    '/api/resource_classes': () => jsonResponse(loadFixture('vocabulary-3-classes.json')),
  };

  it('vocabulary_edit_patches_only_its_scalars_so_the_member_lists_stay', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...detailRoutes,
        '/api/vocabularies/3': () => jsonResponse(loadFixture('vocabulary-3.json')),
      }),
    );
    render(<VocabulariesPage param="3" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this vocabulary' }));
    fireEvent.change(await screen.findByLabelText('Label'), {
      target: { value: 'Bibliographic Ontology revised' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const patch = await waitFor(() => sent(fetchMock, '/api/vocabularies/3', 'PATCH'));
    const body = requestBody(patch);
    expect(patch.url.pathname).toBe('/api/vocabularies/3');
    expect(body['o:namespace_uri']).toBe('http://purl.org/ontology/bibo/');
    expect(body['o:label']).toBe('Bibliographic Ontology revised');
    expect(body['o:comment']).toBe('The Bibliographic Ontology.');
    // The form never sends the member lists or the create-only prefix, so a
    // partial write keeps the stored classes and properties and the prefix.
    expect(body['o:class']).toBeUndefined();
    expect(body['o:property']).toBeUndefined();
    expect(body['o:prefix']).toBeUndefined();
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
  });

  it('vocabulary_delete_sends_delete_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...detailRoutes,
        '/api/vocabularies/3': (_url, init) =>
          jsonResponse(
            init?.method === 'DELETE'
              ? detached(loadFixture('vocabulary-3.json'))
              : loadFixture('vocabulary-3.json'),
          ),
      }),
    );
    render(<VocabulariesPage param="3" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this vocabulary' }));
    fireEvent.click(
      await screen.findByRole('button', { name: 'Delete Bibliographic Ontology' }),
    );

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/vocabularies/3').filter(
          (request) => request.init?.method === 'DELETE',
        ),
      ).toHaveLength(1);
    });
    expect(window.location.hash).toBe('#/standards/vocabularies');
  });

  it('a_permanent_vocabulary_offers_no_edit_or_delete', async () => {
    installFetch(
      withFixture({
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
        '/api/properties': () => jsonResponse([]),
        '/api/resource_classes': () => jsonResponse([]),
      }),
    );
    render(<VocabulariesPage param="1" />);

    expect(await screen.findByRole('heading', { name: 'Dublin Core' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Edit this vocabulary' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Delete this vocabulary' })).toBeNull();
  });
});

describe('property create, edit and delete', () => {
  it('property_create_posts_the_member_with_its_vocabulary', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/properties': () => jsonResponse(loadFixture('property-1.json')),
      }),
    );
    render(<PropertiesPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Vocabulary'), {
      target: { value: '2' },
    });
    fireEvent.change(screen.getByLabelText('Local name'), { target: { value: 'isbn' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'ISBN' } });
    fireEvent.change(screen.getByLabelText('Comment'), {
      target: { value: 'An identifier.' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add property' }));

    const post = await waitFor(() => sent(fetchMock, '/api/properties', 'POST'));
    const body = requestBody(post);
    expect(body['o:local_name']).toBe('isbn');
    expect(body['o:label']).toBe('ISBN');
    expect(body['o:comment']).toBe('An identifier.');
    expect(body['o:vocabulary']).toEqual({ 'o:id': 2 });
    expect(body['o:owner']).toBeUndefined();
  });

  it('property_create_refuses_a_missing_vocabulary_before_any_request', async () => {
    const fetchMock = installFetch(withFixture(referenceRoutes));
    render(<PropertiesPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Local name'), { target: { value: 'isbn' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'ISBN' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add property' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('A vocabulary must be set.');
    expect(requestsFor(fetchMock, '/api/properties')).toHaveLength(0);
  });

  it('property_edit_puts_the_three_members_and_leaves_the_vocabulary_alone', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/properties/1': () => jsonResponse(loadFixture('property-1.json')),
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
      }),
    );
    render(<PropertiesPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this property' }));
    fireEvent.change(await screen.findByLabelText('Label'), { target: { value: 'Name' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const put = await waitFor(() => sent(fetchMock, '/api/properties/1', 'PUT'));
    const body = requestBody(put);
    expect(body['o:local_name']).toBe('title');
    expect(body['o:label']).toBe('Name');
    expect(body['o:comment']).toBe('A name given to the resource.');
    // `vocab_member::hydrate` reads `o:vocabulary` on create only, and the read
    // carries no owner, so an update sends neither.
    expect(body['o:vocabulary']).toBeUndefined();
    expect(body['o:owner']).toBeUndefined();
  });

  it('property_edit_renders_the_api_validation_message', async () => {
    installFetch((url, init) => {
      if (url.pathname === '/api/properties/1' && init?.method === 'PUT') {
        return jsonResponse(
          { errors: { 'o:local_name': ['The local name "title" is already taken.'] } },
          { status: 422 },
        );
      }
      return withFixture({
        ...referenceRoutes,
        '/api/properties/1': () => jsonResponse(loadFixture('property-1.json')),
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
      })(url, init);
    });
    render(<PropertiesPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this property' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The local name "title" is already taken. (HTTP 422)',
    );
  });

  it('property_delete_sends_delete_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/properties/1': (_url, init) =>
          jsonResponse(
            init?.method === 'DELETE'
              ? detached(loadFixture('property-1.json'))
              : loadFixture('property-1.json'),
          ),
        '/api/vocabularies/1': () => jsonResponse(loadFixture('vocabulary-1.json')),
      }),
    );
    render(<PropertiesPage param="1" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this property' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Title' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/properties/1').filter(
          (request) => request.init?.method === 'DELETE',
        ),
      ).toHaveLength(1);
    });
    expect(window.location.hash).toBe('#/standards/properties');
  });
});

describe('resource class create, edit and delete', () => {
  it('resource_class_create_posts_the_member_with_its_vocabulary', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/resource_classes': () => jsonResponse(loadFixture('resource-class-13.json')),
      }),
    );
    render(<ResourceClassesPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Vocabulary'), { target: { value: '2' } });
    fireEvent.change(screen.getByLabelText('Local name'), { target: { value: 'Photograph' } });
    fireEvent.change(screen.getByLabelText('Label'), { target: { value: 'Photograph' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add resource class' }));

    const post = await waitFor(() => sent(fetchMock, '/api/resource_classes', 'POST'));
    const body = requestBody(post);
    expect(body['o:local_name']).toBe('Photograph');
    expect(body['o:label']).toBe('Photograph');
    expect(body['o:vocabulary']).toEqual({ 'o:id': 2 });
    expect(body['o:comment']).toBeNull();
  });

  it('resource_class_edit_puts_the_three_members_and_leaves_the_vocabulary_alone', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/resource_classes/13': () => jsonResponse(loadFixture('resource-class-13.json')),
        '/api/vocabularies/2': () => jsonResponse(loadFixture('vocabulary-2.json')),
      }),
    );
    render(<ResourceClassesPage param="13" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Edit this resource class' }));
    fireEvent.change(await screen.findByLabelText('Label'), { target: { value: 'Photograph' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const put = await waitFor(() => sent(fetchMock, '/api/resource_classes/13', 'PUT'));
    const body = requestBody(put);
    expect(body['o:local_name']).toBe('StillImage');
    expect(body['o:label']).toBe('Photograph');
    expect(body['o:comment']).toBeNull();
    // The member editor hydrates `o:vocabulary` on create only, and the read
    // carries no owner, so an update sends neither. The class path shares the
    // property path's editor, and this is the test that says so for the class.
    expect(body['o:vocabulary']).toBeUndefined();
    expect(body['o:owner']).toBeUndefined();
  });

  it('resource_class_delete_sends_delete_and_returns_to_the_list', async () => {
    const fetchMock = installFetch(
      withFixture({
        ...referenceRoutes,
        '/api/resource_classes/13': (_url, init) =>
          jsonResponse(
            init?.method === 'DELETE'
              ? detached(loadFixture('resource-class-13.json'))
              : loadFixture('resource-class-13.json'),
          ),
        '/api/vocabularies/2': () => jsonResponse(loadFixture('vocabulary-2.json')),
      }),
    );
    render(<ResourceClassesPage param="13" />);

    fireEvent.click(await screen.findByRole('button', { name: 'Delete this resource class' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete Still Image' }));

    await waitFor(() => {
      expect(
        requestsFor(fetchMock, '/api/resource_classes/13').filter(
          (request) => request.init?.method === 'DELETE',
        ),
      ).toHaveLength(1);
    });
    expect(window.location.hash).toBe('#/standards/classes');
  });
});
