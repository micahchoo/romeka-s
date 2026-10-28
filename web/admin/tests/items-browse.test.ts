/**
 * The Items browse query: the URL codec and the API query it sends.
 */
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import { matchingQuery } from '../src/api/bulk';
import {
  browseQuery,
  fieldPlaceHref,
  everyMatchQuery,
  formatBrowse,
  itemsHref,
  linksPlaceHref,
  NO_FILTERS,
  parseBrowse,
  tickedListQuery,
  withFilters,
  withoutFilter,
} from '../src/pages/items/browse';
import type { ItemBrowse } from '../src/pages/items/browse';

const parse = (query: string): ItemBrowse => parseBrowse(new URLSearchParams(query), 'table');

const PLAIN: ItemBrowse = { filters: NO_FILTERS, sort: 'newest', view: 'table', page: 1 };

describe('items browse URL', () => {
  it('a full query reads back to the same URL', () => {
    const query =
      'q=harbour+map&class=13&set=4&template=2&owner=3&visibility=private&site=7&media=image&sort=title&view=grid&page=3';
    expect(formatBrowse(parse(query)).toString()).toBe(query);
  });

  it('each special site and media value reads back', () => {
    for (const query of ['site=any', 'site=none', 'media=any', 'media=none', 'media=other']) {
      expect(formatBrowse(parse(query)).toString()).toBe(query);
    }
  });

  it('the plain list has an empty query', () => {
    expect(formatBrowse(PLAIN).toString()).toBe('');
    expect(parse('')).toEqual(PLAIN);
  });

  it('drops unknown and malformed values', () => {
    const state = parse(
      'class=abc&set=0&template=-2&owner=1.5&visibility=hidden&site=some&media=photo&sort=random&view=list&page=0&extra=1',
    );
    expect(state).toEqual(PLAIN);
  });

  it('takes the remembered view only when the URL names none', () => {
    expect(parseBrowse(new URLSearchParams(''), 'grid').view).toBe('grid');
    expect(parseBrowse(new URLSearchParams('view=table'), 'grid').view).toBe('table');
  });

  it('a blank search is no search', () => {
    expect(parse('q=%20%20').filters.search).toBe('');
  });
});

describe('items browse filters', () => {
  it('a filter change goes back to page 1', () => {
    const onPage3 = parse('page=3');
    expect(withFilters(onPage3, { visibility: 'private' }).page).toBe(1);
    expect(withoutFilter(parse('class=13&page=2'), 'classId')).toEqual(PLAIN);
  });
});

describe('browseQuery', () => {
  it('the plain list asks for 24 items, newest first', () => {
    expect(browseQuery(PLAIN)).toEqual({ page: 1, perPage: 24, sortBy: 'id', sortOrder: 'desc' });
  });

  it('every filter becomes its server parameter', () => {
    const state = parse(
      'q=map&class=13&set=4&template=2&owner=3&visibility=public&site=7&media=image&sort=modified&page=2',
    );
    expect(browseQuery(state)).toEqual({
      page: 2,
      perPage: 24,
      sortBy: 'modified',
      sortOrder: 'desc',
      search: 'map',
      resourceClassId: brandId<'ClassId'>(13),
      itemSetId: brandId<'ItemSetId'>(4),
      resourceTemplateId: brandId<'TemplateId'>(2),
      ownerId: brandId<'UserId'>(3),
      isPublic: 1,
      siteId: brandId<'SiteId'>(7),
      hasMediaKind: ['local-image', 'iiif-image'],
    });
  });

  it('private, no site and no media are the zero flags', () => {
    expect(browseQuery(parse('visibility=private&site=none&media=none'))).toMatchObject({
      isPublic: 0,
      inSites: 0,
      hasMedia: 0,
    });
    expect(browseQuery(parse('site=any&media=any'))).toMatchObject({ inSites: 1, hasMedia: 1 });
  });

  it('each sort names its column and order', () => {
    expect(browseQuery(parse('sort=oldest'))).toMatchObject({ sortBy: 'id', sortOrder: 'asc' });
    expect(browseQuery(parse('sort=title'))).toMatchObject({ sortBy: 'title', sortOrder: 'asc' });
  });

  it('a field sort names the term in the URL and sends it as the sort, first to last', () => {
    const state = parse('set=4&sort=dcterms:identifier');
    expect(state.sort).toBe('dcterms:identifier');
    expect(formatBrowse(state).toString()).toBe('set=4&sort=dcterms%3Aidentifier');
    expect(browseQuery(state)).toMatchObject({ sortBy: 'dcterms:identifier', sortOrder: 'asc' });
    expect(everyMatchQuery(state)).toMatchObject({ sortBy: 'dcterms:identifier', sortOrder: 'asc' });
  });

  it('a sort that is neither a column nor a term opens newest first', () => {
    expect(parse('sort=dcterms:').sort).toBe('newest');
    expect(parse('sort=no term').sort).toBe('newest');
    expect(parse('sort=a:b:c').sort).toBe('newest');
  });
});

describe('the download queries', () => {
  it('every match is the page query without its window', () => {
    const state = parse('q=map&set=4&links=42&via=2&media=none&sort=modified&page=3');
    const { page, perPage, ...filters } = browseQuery(state);
    expect([page, perPage]).toEqual([3, 24]);
    expect(everyMatchQuery(state)).toEqual(filters);
  });

  it('the ticked items keep the order and drop the filters', () => {
    const ticked = [brandId<'ItemId'>(12), brandId<'ItemId'>(9)];
    expect(tickedListQuery(parse('q=map&set=4&sort=title'), ticked)).toEqual({
      sortBy: 'title',
      sortOrder: 'asc',
      id: '12,9',
    });
  });
});

describe('the links filter', () => {
  it('reads back to the same URL, with and without a field, beside the other filters', () => {
    for (const query of ['links=42', 'links=42&via=2', 'q=map&set=4&links=42&via=2&visibility=private']) {
      expect(formatBrowse(parse(query)).toString()).toBe(query);
    }
    expect(parse('links=42&via=2').filters.links).toEqual({ target: 42, via: 2 });
  });

  it('drops a field without a record, and a malformed record or field', () => {
    expect(parse('via=2')).toEqual(PLAIN);
    expect(parse('links=0&via=2')).toEqual(PLAIN);
    expect(parse('links=42&via=x').filters.links).toEqual({ target: 42, via: null });
  });

  it('a row of Linked from opens the address the URL reads back', () => {
    const href = linksPlaceHref(brandId<'ItemId'>(42), brandId<'PropertyId'>(2));
    expect(href).toBe('#/items?links=42&via=2');
    expect(parse(href.slice('#/items?'.length)).filters.links).toEqual({ target: 42, via: 2 });
  });

  it('is sent as the res property search, and removing it leaves the plain list', () => {
    expect(browseQuery(parse('links=42&via=2'))).toMatchObject({
      'property[0][type]': 'res',
      'property[0][text]': '42',
      'property[0][property]': '2',
    });
    // `buildPropertyQuery` skips a row without a `property` key, so "any field" sends it empty.
    expect(browseQuery(parse('links=42'))).toMatchObject({ 'property[0][property]': '' });
    expect(withoutFilter(parse('links=42&via=2'), 'links')).toEqual(PLAIN);
  });

  it('a bulk change over every match selects through the same search', () => {
    expect(matchingQuery(browseQuery(parse('set=4&links=42&via=2&page=3')))).toEqual({
      item_set_id: 4,
      'property[0][type]': 'res',
      'property[0][text]': '42',
      'property[0][property]': '2',
    });
  });
});

describe('the field filter', () => {
  it('reads back to the same URL, empty and filled, beside the other filters', () => {
    for (const query of ['field=7%3Aempty', 'field=7%3Afilled', 'q=map&links=42&via=2&template=2&field=7%3Aempty']) {
      expect(formatBrowse(parse(query)).toString()).toBe(query);
    }
    expect(parse('field=7:empty').filters.field).toEqual({ property: 7, presence: 'empty' });
    expect(parse('field=7:filled').filters.field).toEqual({ property: 7, presence: 'filled' });
  });

  it('drops a malformed field filter', () => {
    for (const query of ['field=7', 'field=7:blank', 'field=x:empty', 'field=0:filled', 'field=:empty']) {
      expect(parse(query)).toEqual(PLAIN);
    }
  });

  it('is sent as the nex or ex property search, and removing it leaves the plain list', () => {
    expect(browseQuery(parse('field=7:empty'))).toMatchObject({
      'property[1][property]': '7',
      'property[1][type]': 'nex',
    });
    expect(browseQuery(parse('field=7:filled'))).toMatchObject({
      'property[1][property]': '7',
      'property[1][type]': 'ex',
    });
    expect(withoutFilter(parse('field=7:empty&page=2'), 'field')).toEqual(PLAIN);
  });

  it('keeps its own property row beside the links filter, so both narrow the list', () => {
    expect(matchingQuery(browseQuery(parse('links=42&field=7:empty')))).toEqual({
      'property[0][property]': '',
      'property[0][type]': 'res',
      'property[0][text]': '42',
      'property[1][property]': '7',
      'property[1][type]': 'nex',
    });
  });

  it("an item type's missing count opens the address the URL reads back", () => {
    const href = fieldPlaceHref(brandId<'TemplateId'>(2), brandId<'PropertyId'>(7));
    expect(href).toBe('#/items?template=2&field=7%3Aempty');
    expect(parse(href.slice('#/items?'.length)).filters).toMatchObject({
      templateId: 2,
      field: { property: 7, presence: 'empty' },
    });
  });
});

describe('the dashboard filters', () => {
  const query = 'set=none&template=none&changed=2026-10-20T08%3A30%3A00Z';

  it('no set, no item type and changed by others read back to the same URL', () => {
    expect(formatBrowse(parse(query)).toString()).toBe(query);
    expect(parse(query).filters).toMatchObject({
      itemSetId: 'none',
      templateId: 'none',
      changedSince: '2026-10-20T08:30:00Z',
    });
  });

  it('drops a changed time that is not a time', () => {
    expect(parse('changed=last-week').filters.changedSince).toBeNull();
  });

  it('each is sent as its server parameter', () => {
    expect(everyMatchQuery(parse(query))).toEqual({
      sortBy: 'id',
      sortOrder: 'desc',
      romekaNoItemSet: 1,
      romekaNoResourceTemplate: 1,
      romekaChangedByOthersSince: '2026-10-20T08:30:00Z',
    });
  });

  it('a dashboard line opens the address the URL reads back', () => {
    const href = itemsHref({ itemSetId: 'none' });
    expect(href).toBe('#/items?set=none');
    expect(parse(href.slice('#/items?'.length)).filters.itemSetId).toBe('none');
  });
});
