/**
 * The Items browse query: the URL codec and the API query it sends.
 */
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import {
  browseQuery,
  formatBrowse,
  NO_FILTERS,
  parseBrowse,
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
});
