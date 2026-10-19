/**
 * The item pool codec. Upstream's Resources form edits `o:item_pool` as a query
 * string: it shows `http_build_query($site->itemPool())` and stores
 * `parse_str($formData['item_pool'])` (`SiteAdmin/IndexController::resourcesAction`).
 */
import { describe, expect, it } from 'vitest';
import { itemPoolFromQuery, itemPoolQuery } from '../src/pages/sites/itemPool';

describe('item pool codec', () => {
  it('an_empty_pool_is_an_empty_query_and_back', () => {
    expect(itemPoolQuery([])).toBe('');
    expect(itemPoolQuery(null)).toBe('');
    expect(itemPoolFromQuery('')).toEqual([]);
  });

  it('a_flat_pool_reads_as_pairs_and_parses_back_to_strings', () => {
    expect(itemPoolQuery({ fulltext_search: 'fish soup', item_set_id: 3 })).toBe(
      'fulltext_search=fish%20soup&item_set_id=3',
    );
    expect(itemPoolFromQuery('fulltext_search=fish+soup&item_set_id=3')).toEqual({
      fulltext_search: 'fish soup',
      item_set_id: '3',
    });
  });

  it('nested_keys_are_brackets_like_http_build_query', () => {
    const pool = {
      property: [{ joiner: 'and', property: '1', type: 'eq', text: 'Map' }],
      resource_class_id: ['4', '5'],
    };
    const query = itemPoolQuery(pool);
    expect(decodeURIComponent(query)).toBe(
      'property[0][joiner]=and&property[0][property]=1&property[0][type]=eq&property[0][text]=Map' +
        '&resource_class_id[0]=4&resource_class_id[1]=5',
    );
    expect(itemPoolFromQuery(query)).toEqual(pool);
  });

  it('an_empty_bracket_appends_like_parse_str', () => {
    expect(itemPoolFromQuery('item_set_id[]=1&item_set_id[]=2')).toEqual({
      item_set_id: ['1', '2'],
    });
  });

  it('non_sequential_numeric_keys_stay_an_object', () => {
    expect(itemPoolFromQuery('a[2]=x&a[0]=y')).toEqual({ a: { '2': 'x', '0': 'y' } });
  });

  it('booleans_are_one_and_zero_and_null_is_dropped', () => {
    expect(itemPoolQuery({ has_media: true, is_public: false, owner_id: null })).toBe(
      'has_media=1&is_public=0',
    );
  });
});
