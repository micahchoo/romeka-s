/**
 * The place reads (`src/api/places.ts`) against answers the real routes gave
 * (`tests/fixtures/places/*.json`, captured from `romeka serve` with an item "Person A" that two
 * items name as Creator and one item set as Subject, a set of 2 items and an empty set).
 */
import { afterEach, describe, expect, it } from 'vitest';
import { vi } from 'vitest';
import { brandId, ParseError } from '../src/api/parse';
import {
  fieldQuery,
  linksQuery,
  parseFieldCounts,
  parseLinkRows,
  parseRecordSummary,
  parseSetItemCounts,
  placesApi,
} from '../src/api/places';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, urlsFor } from './stub';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('set counts wire', () => {
  it('reads each set the answer names; a set it leaves out has no count', () => {
    const counts = parseSetItemCounts(loadFixture('places/set-counts.json'));
    expect([...counts]).toEqual([
      [2, 2],
      [3, 0],
    ]);
    expect(counts.has(brandId<'ItemSetId'>(999))).toBe(false);
  });

  it('refuses a member it does not know and a negative count', () => {
    expect(() => parseSetItemCounts({ item_sets: [{ id: 2, items: 1, hidden: 4 }] })).toThrow(ParseError);
    expect(() => parseSetItemCounts({ item_sets: [{ id: 2, items: -1 }] })).toThrow(ParseError);
  });

  it('asks once for every id on the page, and not at all for none', async () => {
    const fetchMock = installFetch(() => jsonResponse(loadFixture('places/set-counts.json')));
    await placesApi.setItemCounts([brandId<'ItemSetId'>(2), brandId<'ItemSetId'>(3)]);
    await placesApi.setItemCounts([]);
    const urls = urlsFor(fetchMock, '/api/romeka/item-sets/counts');
    expect(urls).toHaveLength(1);
    expect(urls[0]?.searchParams.getAll('id[]')).toEqual(['2', '3']);
  });
});

describe('links wire', () => {
  it('reads one row per kind and field, with the term its label translates by', () => {
    expect(parseLinkRows(loadFixture('places/links.json'))).toEqual([
      { kind: 'items', propertyId: 2, term: 'dcterms:creator', label: 'Creator', count: 2 },
      { kind: 'item_sets', propertyId: 3, term: 'dcterms:subject', label: 'Subject', count: 1 },
    ]);
    expect(parseLinkRows(loadFixture('places/links-none.json'))).toEqual([]);
  });

  it('refuses a kind no list opens', () => {
    expect(() =>
      parseLinkRows({ links: [{ kind: 'value_annotations', property_id: 2, term: 't', label: 'l', count: 1 }] }),
    ).toThrow(ParseError);
  });

  it('a missing target is a 404 with the error body', async () => {
    installFetch(() => jsonResponse(loadFixture('places/links-missing.json'), { status: 404 }));
    await expect(placesApi.links(brandId<'ResourceId'>(999))).rejects.toMatchObject({ status: 404 });
  });

  it('the list a row opens is the res search, through one field or any', () => {
    expect(linksQuery(brandId<'ResourceId'>(1), brandId<'PropertyId'>(2))).toEqual({
      'property[0][type]': 'res',
      'property[0][text]': '1',
      'property[0][property]': '2',
    });
    // `buildPropertyQuery` skips a row without a `property` key, so "any field" sends it empty.
    expect(linksQuery(brandId<'ResourceId'>(1), null)).toEqual({
      'property[0][property]': '',
      'property[0][type]': 'res',
      'property[0][text]': '1',
    });
  });
});

describe('field counts wire', () => {
  it('reads the total and each filled field, and refuses a member it does not know', () => {
    expect(parseFieldCounts({ total: 690, fields: [{ property_id: 7, filled: 412 }] })).toEqual({
      total: 690,
      filled: new Map([[7, 412]]),
    });
    expect(() => parseFieldCounts({ total: 690, fields: [], extra: 1 })).toThrow();
    expect(() => parseFieldCounts({ total: -1, fields: [] })).toThrow();
  });

  it('the list a count opens is the ex or nex search on its own property row', () => {
    expect(fieldQuery(brandId<'PropertyId'>(7), 'empty')).toEqual({
      'property[1][property]': '7',
      'property[1][type]': 'nex',
    });
    expect(fieldQuery(brandId<'PropertyId'>(7), 'filled')).toEqual({
      'property[1][property]': '7',
      'property[1][type]': 'ex',
    });
  });
});

describe('record summary', () => {
  it('names an item and an item set by kind and title', () => {
    expect(parseRecordSummary(loadFixture('places/record-item.json'))).toEqual({
      kind: 'items',
      id: 1,
      title: 'Person A',
    });
    expect(parseRecordSummary(loadFixture('places/record-item-set.json'))).toEqual({
      kind: 'item_sets',
      id: 2,
      title: 'Letters',
    });
  });

  it('reads a class beside the kind, and refuses a value annotation', () => {
    expect(parseRecordSummary({ '@type': ['o:Item', 'dctype:Text'], 'o:id': 5, 'o:title': null }).kind).toBe('items');
    expect(() => parseRecordSummary({ '@type': 'o:ValueAnnotation', 'o:id': 5 })).toThrow(ParseError);
  });
});
