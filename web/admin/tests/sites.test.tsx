/**
 * Site and site page write tests. Each form runs through the real client and
 * parsers against a stubbed fetch, so the method, URL and body a form sends are
 * the bytes the server would get.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isUnknownArray } from '../src/api/parse';
import { parseHash } from '../src/app/router';
import { Shell } from '../src/app/shell';
import { SitesPage } from '../src/pages/sites/page';
import { editorReads } from './editorStubs';
import { loadFixture } from './loadFixture';
import { first, installFetch, jsonResponse, requestBody, requestsFor } from './stub';

const siteFixture = (index: number): unknown => {
  const page = loadFixture('sites-page.json');
  if (!isUnknownArray(page)) {
    throw new Error('expected a site list fixture');
  }
  const entry = page[index];
  if (entry === undefined) {
    throw new Error(`expected a site fixture at index ${String(index)}`);
  }
  return entry;
};

const pageFixture = (): unknown => loadFixture('site-page-1.json');

const usersRoute = (url: URL): Response | null =>
  url.pathname === '/api/users' ? jsonResponse(loadFixture('users-page.json')) : null;

const notFound = (): Response =>
  jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });

const NAVIGATION = [{ type: 'browse', data: { label: 'Browse', query: '' }, links: [] }];

describe('site routing', () => {
  it('router_keeps_the_segments_of_a_nested_site_page_route', () => {
    expect(parseHash('#/sites/1/pages/2')).toEqual({
      page: 'sites',
      param: '1',
      rest: ['1', 'pages', '2'],
    });
  });

  it('shell_routes_a_nested_site_page_url_to_the_page_editor', async () => {
    window.location.hash = '#/sites/1/pages/1';
    installFetch((url) =>
      url.pathname === '/api/site_pages/1'
        ? jsonResponse(pageFixture())
        : (editorReads(url) ?? notFound()),
    );
    render(<Shell />);

    expect(await screen.findByLabelText('Title')).toHaveValue('Welcome');
    expect(await screen.findByRole('listitem', { name: /Block 1: HTML/ })).toBeInTheDocument();
    const nav = screen.getByRole('navigation', { name: 'Admin pages' });
    expect(within(nav).getByRole('link', { name: 'Sites' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    window.location.hash = '';
  });
});

describe('site browse and create', () => {
  it('sites_browse_lists_each_site_and_links_to_its_form', async () => {
    installFetch((url) =>
      url.pathname === '/api/sites'
        ? jsonResponse(loadFixture('sites-page.json'))
        : notFound(),
    );
    render(<SitesPage />);

    expect(await screen.findByRole('link', { name: 'My Site' })).toHaveAttribute(
      'href',
      '#/sites/1',
    );
    expect(screen.getByRole('link', { name: 'Second Site' })).toHaveAttribute(
      'href',
      '#/sites/2',
    );
  });

  it('site_create_posts_the_site_members_and_navigates_to_the_new_site', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites' && init?.method === 'POST') {
        return jsonResponse(siteFixture(0));
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="new" />);

    fireEvent.change(await screen.findByLabelText('Title'), { target: { value: 'New site' } });
    fireEvent.change(screen.getByLabelText('Slug'), { target: { value: 'new-site' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add site' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const post = first(
      requestsFor(fetchMock, '/api/sites').filter((request) => request.init?.method === 'POST'),
    );
    expect(post.init?.method).toBe('POST');
    expect(post.url.pathname).toBe('/api/sites');
    expect(requestBody(post)).toEqual({
      'o:slug': 'new-site',
      'o:title': 'New site',
      'o:theme': 'default',
      'o:summary': null,
      'o:navigation': NAVIGATION,
      'o:homepage': { 'o:id': null },
      'o:item_pool': [],
      'o:is_public': true,
      'o:assign_new_items': true,
    });
    expect(window.location.hash).toBe('#/sites/1');
    window.location.hash = '';
  });
});

describe('site edit', () => {
  it('site_edit_puts_the_members_and_lists_the_pages', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/sites/1' && init?.method === 'PUT') {
        return jsonResponse(siteFixture(0));
      }
      if (url.pathname === '/api/sites/1') {
        return jsonResponse(siteFixture(0));
      }
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(pageFixture());
      }
      return usersRoute(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1']} />);

    const title = await screen.findByLabelText('Title');
    expect(title).toHaveValue('My Site');
    expect(await screen.findByRole('link', { name: 'Welcome' })).toHaveAttribute(
      'href',
      '#/sites/1/pages/1',
    );

    fireEvent.change(title, { target: { value: 'Renamed site' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const put = first(
      requestsFor(fetchMock, '/api/sites/1').filter((request) => request.init?.method === 'PUT'),
    );
    expect(put.init?.method).toBe('PUT');
    expect(requestBody(put)).toEqual({
      'o:slug': 'my-site',
      'o:title': 'Renamed site',
      'o:theme': 'default',
      'o:summary': null,
      'o:navigation': NAVIGATION,
      'o:homepage': { 'o:id': 1 },
      'o:item_pool': [],
      'o:is_public': true,
      'o:assign_new_items': false,
      'o:owner': { 'o:id': 1 },
    });
  });
});

describe('site page edit', () => {
  it('site_page_edit_reorders_the_blocks_it_saves', async () => {
    const fetchMock = installFetch((url) => {
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(pageFixture());
      }
      return editorReads(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'pages', '1']} />);

    const card = await screen.findByRole('listitem', { name: /Block 2: Page title/ });
    fireEvent.keyDown(card, { key: ' ' });
    fireEvent.keyDown(card, { key: 'ArrowUp' });
    fireEvent.keyDown(card, { key: ' ' });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const put = first(
      requestsFor(fetchMock, '/api/site_pages/1').filter(
        (request) => request.init?.method === 'PUT',
      ),
    );
    const body = requestBody(put);
    expect(body['o:block']).toEqual([
      { 'o:layout': 'pageTitle', 'o:data': [], 'o:attachment': [] },
      {
        'o:layout': 'html',
        'o:data': { html: '<p>First block.</p>' },
        'o:attachment': [],
      },
    ]);
  });
});

describe('site page create', () => {
  it('site_page_create_posts_the_site_reference_and_the_page', async () => {
    const fetchMock = installFetch((url, init) => {
      if (url.pathname === '/api/site_pages' && init?.method === 'POST') {
        return jsonResponse(pageFixture());
      }
      if (url.pathname === '/api/site_pages/1') {
        return jsonResponse(pageFixture());
      }
      return editorReads(url) ?? notFound();
    });
    render(<SitesPage param="1" rest={['1', 'pages', 'new']} />);

    fireEvent.change(await screen.findByLabelText('Title'), {
      target: { value: 'Second page' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add page' }));

    expect(await screen.findByText('Saved.')).toBeInTheDocument();
    const post = first(
      requestsFor(fetchMock, '/api/site_pages').filter(
        (request) => request.init?.method === 'POST',
      ),
    );
    expect(post.init?.method).toBe('POST');
    expect(post.url.pathname).toBe('/api/site_pages');
    expect(requestBody(post)).toEqual({
      'o:site': { 'o:id': 1 },
      'o:title': 'Second page',
      'o:slug': '',
      'o:is_public': true,
      'o:layout': null,
      'o:layout_data': {},
      'o:block': [],
    });
    expect(window.location.hash).toBe('#/sites/1/pages/1');
    window.location.hash = '';
  });
});
