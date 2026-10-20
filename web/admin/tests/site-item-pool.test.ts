/**
 * The item pool codec. Upstream's Resources form edits `o:item_pool` as a query
 * string: it shows `http_build_query($site->itemPool())` and stores
 * `parse_str($formData['item_pool'])` (`SiteAdmin/IndexController::resourcesAction`).
 */
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import {
  itemPoolFromQuery,
  itemPoolQuery,
  itemSetPool,
  poolKind,
  syncSwitch,
} from '../src/pages/sites/itemPool';

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

describe('the pool the Items tab writes from item sets', () => {
  it('names each set as parse_str stores item_set_id[]', () => {
    const sets = [brandId<'ItemSetId'>(2), brandId<'ItemSetId'>(7)];
    expect(itemSetPool(sets)).toEqual({ item_set_id: ['2', '7'] });
    expect(itemPoolQuery(itemSetPool(sets))).toBe('item_set_id%5B0%5D=2&item_set_id%5B1%5D=7');
    expect(itemSetPool([])).toEqual([]);
  });

  it('tells what joins on create: every new item, the sets\' items, or a search\'s', () => {
    expect(poolKind([])).toBe('every');
    expect(poolKind({})).toBe('every');
    expect(poolKind(null)).toBe('every');
    expect(poolKind({ item_set_id: ['2'] })).toBe('sets');
    expect(poolKind({ item_set_id: ['2'], is_public: '1' })).toBe('search');
    expect(poolKind({ fulltext_search: 'fish' })).toBe('search');
  });

  it('the sync switch says what turning it on does, and stays off with no set', () => {
    expect(syncSwitch(false, [])).toEqual({ disabled: true, help: 'siteItems.syncNeedsSets' });
    expect(syncSwitch(false, { item_set_id: ['2'] })).toEqual({
      disabled: false,
      help: 'siteItems.syncOffSets',
    });
    expect(syncSwitch(false, { fulltext_search: 'fish' })).toEqual({
      disabled: false,
      help: 'siteItems.syncOffSearch',
    });
    expect(syncSwitch(true, { item_set_id: ['2'] })).toEqual({ disabled: false, help: 'siteItems.syncSets' });
    // A site already syncing everything can still turn it off.
    expect(syncSwitch(true, [])).toEqual({ disabled: false, help: 'siteItems.syncEvery' });
  });
});
