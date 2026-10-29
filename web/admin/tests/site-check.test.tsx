/**
 * The site check on the site hub: each line's count, read through the real
 * client and parsers against a stubbed fetch, so the requests it makes are the
 * ones the server would get.
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { SitesPage } from '../src/pages/sites/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, urlsFor } from './stub';

type Json = Record<string, unknown>;

const record = (value: unknown): Json => {
  if (!isRecord(value)) {
    throw new Error('expected a record fixture');
  }
  return value;
};

const html = (text: string): Json => ({
  'o:layout': 'html',
  'o:data': { html: text },
  'o:layout_data': null,
  'o:attachment': [],
});

const featuredItem = (item: number): Json => ({
  'o:layout': 'html',
  'o:data': { html: '' },
  'o:layout_data': { romeka: { block: 'featuredItem', v: 1, settings: { item } } },
  'o:attachment': [],
});

const mediaBlock = (item: number, media: number): Json => ({
  'o:layout': 'media',
  'o:data': {},
  'o:layout_data': null,
  'o:attachment': [
    {
      'o:item': { '@id': `http://localhost/api/items/${String(item)}`, 'o:id': item },
      'o:media': { '@id': `http://localhost/api/media/${String(media)}`, 'o:id': media },
      'o:caption': '',
    },
  ],
});

const browsePreview = (query: string): Json => ({
  'o:layout': 'browsePreview',
  'o:data': { resource_type: 'items', query, limit: 12 },
  'o:layout_data': null,
  'o:attachment': [],
});

/** A site page: id, title, visibility and blocks. */
const sitePage = (id: number, title: string, isPublic: boolean, blocks: readonly Json[]): Json => ({
  ...record(loadFixture('site-page-1.json')),
  '@id': `http://localhost/api/site_pages/${String(id)}`,
  'o:id': id,
  'o:slug': title.toLowerCase(),
  'o:title': title,
  'o:is_public': isPublic,
  'o:block': blocks,
});

/** The fixture site holding `pages` in that order; Welcome (1) is its homepage and its menu. */
const siteWith = (pages: readonly Json[]): Json => ({
  ...record(loadFixture('site-admin.json')),
  'o:page': pages.map((page) => ({ '@id': page['@id'], 'o:id': page['o:id'] })),
});

const item = (id: number, isPublic: boolean): Json => ({
  ...record(loadFixture('item-1.json')),
  '@id': `http://localhost/api/items/${String(id)}`,
  'o:id': id,
  'o:is_public': isPublic,
});

const media = (id: number, itemId: number, isPublic: boolean): Json => ({
  ...record(loadFixture('media-951.json')),
  '@id': `http://localhost/api/media/${String(id)}`,
  'o:id': id,
  'o:is_public': isPublic,
  'o:item': { '@id': `http://localhost/api/items/${String(itemId)}`, 'o:id': itemId },
});

const ids = (url: URL): readonly number[] => url.searchParams.getAll('id[]').map(Number);

interface Stub {
  readonly pages: readonly Json[];
  readonly items?: readonly Json[];
  readonly media?: readonly Json[];
  /** How many items a list query matches for visitors, by its fulltext search. */
  readonly listed?: Readonly<Record<string, number>>;
}

/** The server for one site, answering the hub's reads and the check's. */
const serve = ({ pages, items = [], media: files = [], listed = {} }: Stub) =>
  installFetch((url) => {
    switch (url.pathname) {
      case '/api/sites/1':
        return jsonResponse(siteWith(pages));
      case '/api/site_pages':
        return listResponse(pages, pages.length);
      case '/api/media': {
        const found = files.filter((entry) => ids(url).includes(Number(entry['o:id'])));
        return listResponse(found, found.length);
      }
      case '/api/items': {
        if (url.searchParams.has('id[]')) {
          const privateOnly = url.searchParams.get('is_public') === '0';
          const found = items.filter(
            (entry) =>
              ids(url).includes(Number(entry['o:id'])) && (!privateOnly || entry['o:is_public'] === false),
          );
          return listResponse(found, found.length);
        }
        const fulltext = url.searchParams.get('fulltext_search');
        return listResponse([], fulltext === null ? 7 : (listed[fulltext] ?? 0));
      }
      default:
        return jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 });
    }
  });

/** Open the hub, press the check, and answer the check's region once it has read. */
const runCheck = async (): Promise<HTMLElement> => {
  render(<SitesPage param="1" rest={['1']} />);
  fireEvent.click(await screen.findByRole('button', { name: 'Check this site' }));
  const region = screen.getByRole('region', { name: 'Before you share this site' });
  await within(region).findByRole('button', { name: 'Check again' });
  return region;
};

const welcome = sitePage(1, 'Welcome', true, [html('<p>First block.</p>')]);

describe('the site check', () => {
  it('asks nothing until the curator opens it, then says there is nothing to check', async () => {
    const draft = sitePage(2, 'Draft', false, [html(''), featuredItem(1)]);
    const fetchMock = serve({ pages: [welcome, draft], items: [item(1, false)] });
    render(<SitesPage param="1" rest={['1']} />);
    await screen.findByRole('button', { name: 'Check this site' });
    expect(urlsFor(fetchMock, '/api/site_pages')).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: 'Check this site' }));
    const region = screen.getByRole('region', { name: 'Before you share this site' });
    expect(await within(region).findByText('Nothing to check.')).toBeInTheDocument();
    // The private page's unwritten block and private item reach no visitor.
    expect(region).not.toHaveTextContent('not written');
    expect(urlsFor(fetchMock, '/api/media')).toHaveLength(0);
  });

  it('counts the blocks not written on public pages and links each page', async () => {
    const about = sitePage(3, 'About', true, [html(''), html('<p><br></p>'), html('<p>Kept</p>')]);
    const people = sitePage(4, 'People', true, [html('')]);
    const draft = sitePage(2, 'Draft', false, [html(''), html('')]);
    serve({ pages: [welcome, draft, about, people] });
    const region = await runCheck();

    const line = within(region).getByText('3 blocks not written, on 2 public pages').closest('li');
    expect(line).not.toBeNull();
    const links = within(line ?? region).getAllByRole('link');
    expect(links.map((link) => [link.textContent, link.getAttribute('href')])).toEqual([
      ['About', '#/sites/1/pages/3'],
      ['People', '#/sites/1/pages/4'],
    ]);
  });

  it('counts a list that matches nothing a visitor may see, one request per list', async () => {
    const lists = sitePage(3, 'Lists', true, [
      browsePreview('fulltext_search=nothing'),
      browsePreview('fulltext_search=tower'),
      browsePreview('fulltext_search=nothing'),
    ]);
    const hidden = sitePage(2, 'Hidden', false, [browsePreview('fulltext_search=gone')]);
    const fetchMock = serve({ pages: [welcome, hidden, lists], listed: { tower: 4 } });
    const region = await runCheck();

    expect(within(region).getByText('2 lists show nothing to visitors, on 1 public page')).toBeInTheDocument();
    expect(within(region).getByRole('link', { name: 'Lists' })).toHaveAttribute('href', '#/sites/1/pages/3');
    const counts = urlsFor(fetchMock, '/api/items').filter((url) => url.searchParams.has('fulltext_search'));
    expect(counts.map((url) => url.searchParams.get('fulltext_search')).sort()).toEqual(['nothing', 'tower']);
    expect(counts.every((url) => url.searchParams.get('is_public') === '1')).toBe(true);
    expect(counts.every((url) => url.searchParams.get('site_id') === '1')).toBe(true);
  });

  it('counts blocks on public pages that show a private item or media, in one read of each', async () => {
    const shelf = sitePage(3, 'Shelf', true, [
      featuredItem(5),
      mediaBlock(6, 60),
      mediaBlock(8, 80),
      featuredItem(6),
    ]);
    const draft = sitePage(2, 'Draft', false, [featuredItem(5)]);
    const fetchMock = serve({
      pages: [welcome, draft, shelf],
      items: [item(5, false), item(6, true), item(8, false)],
      media: [media(60, 6, false), media(80, 8, true)],
    });
    const region = await runCheck();

    // Item 5 is private; media 60 is private; media 80 is public but its item 8 is not.
    expect(
      within(region).getByText('3 blocks point to private items or media, on 1 public page'),
    ).toBeInTheDocument();
    expect(within(region).getByRole('link', { name: 'Shelf' })).toHaveAttribute('href', '#/sites/1/pages/3');
    expect(urlsFor(fetchMock, '/api/media')).toHaveLength(1);
    expect(urlsFor(fetchMock, '/api/items').filter((url) => url.searchParams.has('id[]'))).toHaveLength(1);
  });

  it('counts public pages outside the menu, not the homepage and not private pages', async () => {
    const about = sitePage(3, 'About', true, [html('<p>About us.</p>')]);
    const draft = sitePage(2, 'Draft', false, [html('<p>Soon.</p>')]);
    serve({ pages: [welcome, draft, about] });
    const region = await runCheck();

    expect(within(region).getByRole('link', { name: '1 public page is not in the menu' })).toHaveAttribute(
      'href',
      '#/sites/1/navigation',
    );
    expect(region).not.toHaveTextContent('not written');
  });
});
