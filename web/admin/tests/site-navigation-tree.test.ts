/**
 * The navigation tree model: the stored `o:navigation` round trip, and every
 * edit the Navigation screen makes going through one reducer.
 */
import { describe, expect, it } from 'vitest';
import type { JsonValue, PageId } from '../src/api/types';
import { pageIdFromParam } from '../src/api/parse';
import {
  firstLinkedPage,
  keyboardMove,
  linkIssues,
  linkedPages,
  navigationJson,
  navigationLinks,
  navigationReducer,
} from '../src/pages/sites/navigationTree';
import type { NavLink, NavigationAction } from '../src/pages/sites/navigationTree';

const pageId = (value: number): PageId => {
  const id = pageIdFromParam(String(value));
  if (id === null) {
    throw new Error(`bad page id ${String(value)}`);
  }
  return id;
};

const browse = (label: string, links: JsonValue[] = []): JsonValue => ({
  type: 'browse',
  data: { label, query: '' },
  links,
});

/** The tree's labels as nested arrays, the shape a reader checks by eye. */
const shape = (links: readonly NavLink[]): unknown[] =>
  links.map((link) => (link.links.length === 0 ? link.label : [link.label, shape(link.links)]));

const keyOf = (links: readonly NavLink[], label: string): number => {
  for (const link of links) {
    if (link.label === label) {
      return link.key;
    }
    const found = keyOf(link.links, label);
    if (found !== -1) {
      return found;
    }
  }
  return -1;
};

const apply = (links: readonly NavLink[], ...actions: NavigationAction[]): readonly NavLink[] =>
  actions.reduce(navigationReducer, links);

describe('navigation tree', () => {
  it('the_stored_tree_round_trips_including_an_unknown_link_type', () => {
    const stored: JsonValue[] = [
      browse('Browse', [{ type: 'page', data: { id: 1, label: null }, links: [] }]),
      { type: 'url', data: { label: 'Home', url: '/', target_blank: true }, links: [] },
      { type: 'thirdParty', data: { keep: 'this' }, links: [] },
    ];
    expect(navigationJson(navigationLinks(stored))).toEqual(stored);
  });

  it('a_drop_before_after_and_inside_a_link_moves_the_whole_subtree', () => {
    const links = navigationLinks([browse('A', [browse('A1')]), browse('B'), browse('C')]);
    const a = keyOf(links, 'A');
    const c = keyOf(links, 'C');
    expect(shape(apply(links, { type: 'move', key: a, target: c, place: 'after' }))).toEqual([
      'B',
      'C',
      ['A', ['A1']],
    ]);
    expect(shape(apply(links, { type: 'move', key: c, target: a, place: 'before' }))).toEqual([
      'C',
      ['A', ['A1']],
      'B',
    ]);
    expect(shape(apply(links, { type: 'move', key: c, target: a, place: 'inside' }))).toEqual([
      ['A', ['A1', 'C']],
      'B',
    ]);
  });

  it('a_link_never_moves_into_its_own_subtree', () => {
    const links = navigationLinks([browse('A', [browse('A1')]), browse('B')]);
    const a = keyOf(links, 'A');
    const a1 = keyOf(links, 'A1');
    expect(apply(links, { type: 'move', key: a, target: a1, place: 'inside' })).toBe(links);
    expect(apply(links, { type: 'move', key: a, target: a, place: 'after' })).toBe(links);
  });

  it('the_keyboard_moves_up_down_in_and_out', () => {
    const links = navigationLinks([browse('A', [browse('A1')]), browse('B'), browse('C')]);
    const step = (tree: readonly NavLink[], label: string, direction: 'up' | 'down' | 'in' | 'out') => {
      const action = keyboardMove(tree, keyOf(tree, label), direction);
      return action === null ? tree : navigationReducer(tree, action);
    };
    const down = step(links, 'B', 'down');
    expect(shape(down)).toEqual([['A', ['A1']], 'C', 'B']);
    const nested = step(links, 'B', 'in');
    expect(shape(nested)).toEqual([['A', ['A1', 'B']], 'C']);
    expect(shape(step(nested, 'B', 'out'))).toEqual([['A', ['A1']], 'B', 'C']);
    expect(shape(step(nested, 'B', 'up'))).toEqual([['A', ['B', 'A1']], 'C']);
    expect(keyboardMove(links, keyOf(links, 'A'), 'up')).toBeNull();
    expect(keyboardMove(links, keyOf(links, 'A'), 'in')).toBeNull();
    expect(keyboardMove(links, keyOf(links, 'A'), 'out')).toBeNull();
  });

  it('add_patch_and_remove_edit_one_link_at_any_depth', () => {
    const links = navigationLinks([browse('A', [browse('A1')])]);
    const a1 = keyOf(links, 'A1');
    const added = apply(links, { type: 'add', parent: a1, linkType: 'url' });
    const child = added[0]?.links[0]?.links[0];
    expect(child?.type).toBe('url');
    const patched = apply(added, {
      type: 'patch',
      key: child?.key ?? -1,
      patch: { label: 'Docs', url: 'https://example.org' },
    });
    expect(navigationJson(patched)).toEqual([
      browse('A', [
        browse('A1', [
          {
            type: 'url',
            data: { label: 'Docs', url: 'https://example.org', target_blank: false },
            links: [],
          },
        ]),
      ]),
    ]);
    expect(shape(apply(patched, { type: 'remove', key: a1 }))).toEqual(['A']);
  });

  it('changing_an_unknown_link_type_drops_its_stored_data', () => {
    const links = navigationLinks([{ type: 'thirdParty', data: { keep: 'this' }, links: [] }]);
    const changed = apply(links, {
      type: 'patch',
      key: links[0]?.key ?? -1,
      patch: { type: 'browse' },
    });
    expect(navigationJson(changed)).toEqual([
      { type: 'browse', data: { label: null, query: '' }, links: [] },
    ]);
  });

  it('issues_name_a_nested_page_link_without_a_page_and_a_url_link_without_a_url', () => {
    const links = navigationLinks([
      browse('A', [{ type: 'page', data: { id: null }, links: [] }]),
      { type: 'url', data: { label: 'X', url: '' }, links: [] },
      { type: 'page', data: { id: 2 }, links: [] },
    ]);
    const issues = linkIssues(links);
    expect(issues.size).toBe(2);
    expect(issues.has(links[0]?.links[0]?.key ?? -1)).toBe(true);
    expect(issues.has(links[1]?.key ?? -1)).toBe(true);
  });

  it('linked_pages_and_the_first_linked_page_read_the_whole_tree_in_order', () => {
    const stored: JsonValue[] = [
      browse('A', [{ type: 'page', data: { id: 7 }, links: [] }]),
      { type: 'page', data: { id: 3 }, links: [] },
    ];
    expect([...linkedPages(stored)]).toEqual([pageId(7), pageId(3)]);
    expect(firstLinkedPage(stored)).toBe(pageId(7));
    expect(firstLinkedPage([browse('A')])).toBeNull();
  });
});
