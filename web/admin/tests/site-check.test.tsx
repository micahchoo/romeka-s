/**
 * The site check on the site hub: each line's count, read through the real
 * client and parsers against a stubbed fetch, so the requests it makes are the
 * ones the server would get.
 */
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
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

/** A hero block whose words are `settings`, as the editor stores it. */
const hero = (settings: Readonly<Record<string, string>>): Json => ({
  'o:layout': 'html',
  'o:data': { html: '' },
  'o:layout_data': { romeka: { block: 'hero', v: 2, settings } },
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

const browsePreview = (query: string, resource = 'items'): Json => ({
  'o:layout': 'browsePreview',
  'o:data': { resource_type: resource, query, limit: 12 },
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
const siteWith = (pages: readonly Json[], isPublic = true): Json => ({
  ...record(loadFixture('site-admin.json')),
  'o:is_public': isPublic,
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
  /**
   * A media list's visitor count by its fulltext search: every public media it
   * matches, and how many of those belong to a private item.
   */
  readonly listedMedia?: Readonly<Record<string, { readonly shown: number; readonly ofPrivateItems: number }>>;
  readonly sitePublic?: boolean;
}

/** The server for one site, answering the hub's reads and the check's. */
const serve = ({ pages, items = [], media: files = [], listed = {}, listedMedia = {}, sitePublic = true }: Stub) =>
  installFetch((url) => {
    switch (url.pathname) {
      case '/api/sites/1':
        return jsonResponse(siteWith(pages, sitePublic));
      case '/api/site_pages':
        return listResponse(pages, pages.length);
      case '/api/media': {
        const fulltext = url.searchParams.get('fulltext_search');
        if (fulltext !== null) {
          const counts = listedMedia[fulltext] ?? { shown: 0, ofPrivateItems: 0 };
          return listResponse([], url.searchParams.get('romeka_item_is_public') === '0' ? counts.ofPrivateItems : counts.shown);
        }
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

  it('counts a media list whose public media belong to a private item, as visitors cannot open them', async () => {
    const shelf = sitePage(3, 'Shelf', true, [
      browsePreview('fulltext_search=hidden', 'media'),
      browsePreview('fulltext_search=open', 'media'),
      browsePreview('fulltext_search=none', 'media'),
    ]);
    const fetchMock = serve({
      pages: [welcome, shelf],
      listedMedia: { hidden: { shown: 3, ofPrivateItems: 1 }, open: { shown: 2, ofPrivateItems: 0 } },
    });
    const region = await runCheck();

    expect(within(region).getByText('1 list shows nothing to visitors, on 1 public page')).toBeInTheDocument();
    expect(
      within(region).getByText('1 block points to private items or media, on 1 public page'),
    ).toBeInTheDocument();
    const asked = urlsFor(fetchMock, '/api/media').filter((url) => url.searchParams.get('romeka_item_is_public') === '0');
    // A list that shows a visitor nothing has no media to ask about.
    expect(asked.map((url) => url.searchParams.get('fulltext_search')).sort()).toEqual(['hidden', 'open']);
    expect(asked.every((url) => url.searchParams.get('is_public') === '1')).toBe(true);
  });

  it('on a private site says first that visitors see none of it, and keeps the other lines', async () => {
    const about = sitePage(3, 'About', true, [html('')]);
    serve({ pages: [welcome, about], sitePublic: false });
    const region = await runCheck();

    const first = within(region).getAllByRole('listitem')[0];
    expect(first).toHaveTextContent('This site is private: visitors see none of it.');
    expect(within(first ?? region).getByRole('link', { name: 'Site settings' })).toHaveAttribute(
      'href',
      '#/sites/1/settings',
    );
    expect(within(region).getByText('Once the site is public, visitors meet these:')).toBeInTheDocument();
    expect(within(region).getByText('1 block not written, on 1 public page')).toBeInTheDocument();
  });

  it('on a private site with nothing else to check says only that it is private', async () => {
    serve({ pages: [welcome], sitePublic: false });
    const region = await runCheck();

    expect(within(region).getByText('This site is private: visitors see none of it.', { exact: false })).toBeInTheDocument();
    expect(region).not.toHaveTextContent('Nothing to check.');
    expect(region).not.toHaveTextContent('Once the site is public');
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

  it('counts blocks still holding template text on public pages, right after the blocks not written', async () => {
    const heroPage = sitePage(3, 'Hero', true, [
      hero({ heading: 'Your exhibit title', subheading: 'A sentence about what this exhibit holds' }),
      html('<p>A line worth remembering.</p>'),
      html(''),
      html('<p>Our own words.</p>'),
    ]);
    const draft = sitePage(2, 'Draft', false, [html('<p>A line worth remembering.</p>')]);
    serve({ pages: [welcome, draft, heroPage] });
    const region = await runCheck();

    const line = within(region).getByText('2 blocks still show template text, on 1 public page').closest('li');
    expect(line).not.toBeNull();
    expect(line).toHaveTextContent('Visitors read this text as written. Clear it or write over it in the page editor.');
    expect(
      within(line ?? region)
        .getAllByRole('link')
        .map((link) => [link.textContent, link.getAttribute('href')]),
    ).toEqual([['Hero', '#/sites/1/pages/3']]);
    const texts = [
      '1 block not written, on 1 public page',
      '2 blocks still show template text, on 1 public page',
      '1 public page is not in the menu',
    ].map((text) => within(region).getByText(text));
    const inOrder = texts.every(
      (text, index) => index === 0 || (texts[index - 1]?.compareDocumentPosition(text) ?? 0) & Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(inOrder).toBe(true);
  });

  it('counts template text on a private page in no line', async () => {
    const draft = sitePage(2, 'Draft', false, [hero({ heading: 'Your exhibit title' })]);
    serve({ pages: [welcome, draft] });
    const region = await runCheck();

    expect(await within(region).findByText('Nothing to check.')).toBeInTheDocument();
    expect(region).not.toHaveTextContent('template text');
  });

  it('counts a block that is not written and holds template text only as not written', async () => {
    // Visitors meet no block whose heading is empty, so its old subheading reaches no one.
    const heroPage = sitePage(3, 'Hero', true, [
      hero({ heading: '', subheading: 'A sentence about what this exhibit holds' }),
    ]);
    serve({ pages: [welcome, heroPage] });
    const region = await runCheck();

    expect(within(region).getByText('1 block not written, on 1 public page')).toBeInTheDocument();
    expect(region).not.toHaveTextContent('template text');
  });

  it('says template text in its one-block and several-page words', async () => {
    const heroPage = sitePage(3, 'Hero', true, [hero({ heading: 'Your exhibit title' })]);
    serve({ pages: [welcome, heroPage] });
    const one = await runCheck();
    expect(within(one).getByText('1 block still shows template text, on 1 public page')).toBeInTheDocument();
    cleanup();

    const about = sitePage(4, 'About', true, [
      html('<p>About this site. Replace this text with the story behind the collection.</p>'),
      html('<p>A line worth remembering.</p>'),
    ]);
    serve({ pages: [welcome, heroPage, about] });
    const several = await runCheck();
    expect(within(several).getByText('3 blocks still show template text, on 2 public pages')).toBeInTheDocument();
  });
});
