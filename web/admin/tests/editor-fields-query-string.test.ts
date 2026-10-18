/**
 * The browse query codec. The round-trip cases freeze the rule the block layout
 * depends on: a pair this builder does not edit keeps its place and its value,
 * and a cleared pair stays present so `BrowsePreview::render` does not apply its
 * default sort over it.
 */
import { describe, expect, it } from 'vitest';
import {
  QUERY_KEYS,
  formatQuery,
  parseQuery,
  queryId,
  queryValue,
  withQueryValue,
} from '../src/pages/sites/editor/fields/query';
import { at } from './editor-draft-helpers';

describe('the browse query codec', () => {
  it('reads the pairs PHP parse_str builds, decoding + and %XX', () => {
    const pairs = parseQuery('fulltext_search=cat+dog&item_set_id=3&sort_by=&flag');
    expect(pairs).toEqual([
      { key: 'fulltext_search', value: 'cat dog' },
      { key: 'item_set_id', value: '3' },
      { key: 'sort_by', value: '' },
      { key: 'flag', value: '' },
    ]);
  });

  it('formats the pairs back to the string it read', () => {
    const text = 'fulltext_search=cat%20dog&item_set_id=3&sort_by=title&sort_order=desc';
    expect(formatQuery(parseQuery(text))).toBe(text);
  });

  it('keeps the last value of a repeated name, as parse_str does', () => {
    expect(queryValue(parseQuery('sort_by=title&sort_by=created'), 'sort_by')).toBe('created');
  });

  it('keeps an edited pair in place and drops an earlier duplicate', () => {
    const pairs = withQueryValue(parseQuery('a=1&sort_by=title&b=2&sort_by=created'), 'sort_by', 'modified');
    expect(pairs).toEqual([
      { key: 'a', value: '1' },
      { key: 'b', value: '2' },
      { key: 'sort_by', value: 'modified' },
    ]);
  });

  it('appends a name the query does not carry', () => {
    const pairs = withQueryValue(parseQuery('sort_by=title'), 'fulltext_search', 'cat');
    expect(formatQuery(pairs)).toBe('sort_by=title&fulltext_search=cat');
  });

  it('keeps a cleared pair present so the layout default stays off', () => {
    const pairs = withQueryValue(parseQuery('sort_by=title'), 'sort_by', '');
    expect(queryValue(pairs, 'sort_by')).toBe('');
    expect(pairs).toHaveLength(1);
  });

  it('reads a stored id as a number and refuses anything else', () => {
    const pairs = parseQuery(`item_set_id=3&resource_class_id=&sort_by=title`);
    expect(queryId(pairs, QUERY_KEYS.itemSet)).toBe(3);
    expect(queryId(pairs, QUERY_KEYS.resourceClass)).toBeNull();
    expect(queryId(parseQuery('item_set_id=abc'), QUERY_KEYS.itemSet)).toBeNull();
    expect(at(pairs, 0)).toEqual({ key: 'item_set_id', value: '3' });
  });
});
