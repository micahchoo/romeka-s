/**
 * The site navigation as a tree of links, and the one reducer every edit goes
 * through (`SiteAdmin/IndexController::navigationAction`).
 *
 * `o:navigation` is a nested list of `{type, data, links}` nodes; the port
 * parses and resolves it in `crates/omeka-site/src/nav.rs`. The model knows the
 * four registered link types (`page`, `url`, `browse`, `browseItemSets`) and
 * keeps the `data` of any other type verbatim, so a module's link survives an
 * edit of its neighbours.
 */
import { pageIdFromParam } from '../../api/parse';
import type { JsonValue, PageId } from '../../api/types';

/** The link types the port models (`src/Site/Navigation/Link/*`). */
export const NAVIGATION_TYPES = ['page', 'url', 'browse', 'browseItemSets'] as const;

export type NavigationType = (typeof NAVIGATION_TYPES)[number];

export const isNavigationType = (value: string): value is NavigationType =>
  NAVIGATION_TYPES.some((type) => type === value);

export interface NavLink {
  /** A key for this editing session only; it is not stored. */
  readonly key: number;
  readonly type: string;
  readonly label: string;
  readonly pageId: PageId | null;
  readonly url: string;
  readonly targetBlank: boolean;
  readonly query: string;
  readonly links: readonly NavLink[];
  /** The stored `data` of a link type the model does not know. */
  readonly rawData: JsonValue | null;
}

export type LinkPatch = Partial<
  Pick<NavLink, 'type' | 'label' | 'pageId' | 'url' | 'targetBlank' | 'query'>
>;

/** Where a moved link lands, relative to another link. */
export type Place = 'before' | 'after' | 'inside';

export type NavigationAction =
  | {
      readonly type: 'add';
      readonly parent: number | null;
      readonly linkType: NavigationType;
      readonly pageId?: PageId | null;
    }
  | { readonly type: 'remove'; readonly key: number }
  | { readonly type: 'patch'; readonly key: number; readonly patch: LinkPatch }
  | { readonly type: 'move'; readonly key: number; readonly target: number; readonly place: Place }
  | { readonly type: 'replace'; readonly links: readonly NavLink[] };

let lastKey = 0;

const newLink = (partial: Partial<NavLink> & { readonly type: string }): NavLink => {
  lastKey += 1;
  return {
    key: lastKey,
    label: '',
    pageId: null,
    url: '',
    targetBlank: false,
    query: '',
    links: [],
    rawData: null,
    ...partial,
  };
};

type JsonObject = { readonly [key: string]: JsonValue };

const isJsonObject = (value: JsonValue | undefined): value is JsonObject =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const text = (value: JsonValue | undefined): string =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';

const storedPageId = (value: JsonValue | undefined): PageId | null =>
  typeof value === 'number' || typeof value === 'string'
    ? pageIdFromParam(String(value).trim())
    : null;

const isJsonArray = (value: JsonValue | undefined): value is readonly JsonValue[] =>
  Array.isArray(value);

const childNodes = (node: JsonObject): readonly JsonValue[] =>
  isJsonArray(node.links) ? node.links : [];

const linkFromJson = (node: JsonValue): NavLink => {
  if (!isJsonObject(node)) {
    return newLink({ type: 'browse' });
  }
  const type = typeof node.type === 'string' ? node.type : 'browse';
  const data = isJsonObject(node.data) ? node.data : {};
  return newLink({
    type,
    label: text(data.label),
    pageId: storedPageId(data.id),
    url: text(data.url),
    targetBlank: data.target_blank === true,
    query: text(data.query),
    links: childNodes(node).map(linkFromJson),
    rawData: isNavigationType(type) ? null : (node.data ?? null),
  });
};

/** The stored tree as links. */
export const navigationLinks = (stored: readonly JsonValue[]): readonly NavLink[] =>
  stored.map(linkFromJson);

const labelOrNull = (value: string): string | null => {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
};

const linkJson = (link: NavLink): JsonValue => {
  const links = link.links.map(linkJson);
  if (!isNavigationType(link.type)) {
    return { type: link.type, data: link.rawData ?? {}, links };
  }
  switch (link.type) {
    case 'page':
      return { type: 'page', data: { id: link.pageId, label: labelOrNull(link.label) }, links };
    case 'url':
      return {
        type: 'url',
        data: { label: link.label.trim(), url: link.url.trim(), target_blank: link.targetBlank },
        links,
      };
    case 'browse':
    case 'browseItemSets':
      return {
        type: link.type,
        data: { label: labelOrNull(link.label), query: link.query.trim() },
        links,
      };
  }
};

/** The tree as the `o:navigation` member stores it. */
export const navigationJson = (links: readonly NavLink[]): readonly JsonValue[] =>
  links.map(linkJson);

/** A page link as `o:navigation` stores it, for an append outside the editor. */
export const pageLinkJson = (pageId: PageId): JsonValue => ({
  type: 'page',
  data: { id: pageId, label: null },
  links: [],
});

const mapLink = (
  links: readonly NavLink[],
  key: number,
  map: (link: NavLink) => NavLink,
): readonly NavLink[] =>
  links.map((link) =>
    link.key === key ? map(link) : { ...link, links: mapLink(link.links, key, map) },
  );

const without = (links: readonly NavLink[], key: number): readonly NavLink[] =>
  links
    .filter((link) => link.key !== key)
    .map((link) => ({ ...link, links: without(link.links, key) }));

const find = (links: readonly NavLink[], key: number): NavLink | null => {
  for (const link of links) {
    if (link.key === key) {
      return link;
    }
    const found = find(link.links, key);
    if (found !== null) {
      return found;
    }
  }
  return null;
};

const insert = (
  links: readonly NavLink[],
  moved: NavLink,
  target: number,
  place: Place,
): readonly NavLink[] =>
  links.flatMap((link): NavLink[] => {
    if (link.key !== target) {
      return [{ ...link, links: insert(link.links, moved, target, place) }];
    }
    if (place === 'inside') {
      return [{ ...link, links: [...link.links, moved] }];
    }
    return place === 'before' ? [moved, link] : [link, moved];
  });

const move = (
  links: readonly NavLink[],
  key: number,
  target: number,
  place: Place,
): readonly NavLink[] => {
  const moved = find(links, key);
  if (moved === null || find([moved], target) !== null || find(links, target) === null) {
    return links;
  }
  return insert(without(links, key), moved, target, place);
};

export const navigationReducer = (
  links: readonly NavLink[],
  action: NavigationAction,
): readonly NavLink[] => {
  switch (action.type) {
    case 'add': {
      const link = newLink({ type: action.linkType, pageId: action.pageId ?? null });
      return action.parent === null
        ? [...links, link]
        : mapLink(links, action.parent, (parent) => ({
            ...parent,
            links: [...parent.links, link],
          }));
    }
    case 'remove':
      return without(links, action.key);
    case 'patch':
      return mapLink(links, action.key, (link) => ({
        ...link,
        ...action.patch,
        rawData: action.patch.type === undefined ? link.rawData : null,
      }));
    case 'move':
      return move(links, action.key, action.target, action.place);
    case 'replace':
      return action.links;
  }
};

/** The siblings a link sits among, and its parent's key. */
const siblingsOf = (
  links: readonly NavLink[],
  key: number,
  parent: number | null = null,
): { readonly siblings: readonly NavLink[]; readonly parent: number | null } | null => {
  if (links.some((link) => link.key === key)) {
    return { siblings: links, parent };
  }
  for (const link of links) {
    const found = siblingsOf(link.links, key, link.key);
    if (found !== null) {
      return found;
    }
  }
  return null;
};

export type MoveDirection = 'up' | 'down' | 'in' | 'out';

/**
 * The move one arrow key makes while a link is picked up: up and down swap it
 * with a sibling, in nests it under the sibling above, out lifts it beside its
 * parent. `null` when the link cannot move that way.
 */
export const keyboardMove = (
  links: readonly NavLink[],
  key: number,
  direction: MoveDirection,
): NavigationAction | null => {
  const found = siblingsOf(links, key);
  if (found === null) {
    return null;
  }
  const index = found.siblings.findIndex((link) => link.key === key);
  const above = found.siblings[index - 1];
  const below = found.siblings[index + 1];
  switch (direction) {
    case 'up':
      return above === undefined ? null : { type: 'move', key, target: above.key, place: 'before' };
    case 'down':
      return below === undefined ? null : { type: 'move', key, target: below.key, place: 'after' };
    case 'in':
      return above === undefined ? null : { type: 'move', key, target: above.key, place: 'inside' };
    case 'out':
      return found.parent === null
        ? null
        : { type: 'move', key, target: found.parent, place: 'after' };
  }
};

/**
 * The links the server would refuse (`Link/Page.php::isValid`,
 * `Link/Url.php::isValid`): a page link with no page, a URL link with no URL
 * or no label.
 */
export const linkIssues = (links: readonly NavLink[]): ReadonlySet<number> => {
  const issues = new Set<number>();
  const visit = (link: NavLink): void => {
    if (
      (link.type === 'page' && link.pageId === null) ||
      (link.type === 'url' && (link.url.trim() === '' || link.label.trim() === ''))
    ) {
      issues.add(link.key);
    }
    link.links.forEach(visit);
  };
  links.forEach(visit);
  return issues;
};

/** Every page a stored tree links to, in reading order. */
export const linkedPages = (stored: readonly JsonValue[]): ReadonlySet<PageId> => {
  const pages = new Set<PageId>();
  const visit = (node: JsonValue): void => {
    if (!isJsonObject(node)) {
      return;
    }
    const id = node.type === 'page' && isJsonObject(node.data) ? storedPageId(node.data.id) : null;
    if (id !== null) {
      pages.add(id);
    }
    childNodes(node).forEach(visit);
  };
  stored.forEach(visit);
  return pages;
};

/**
 * The page the public site opens when no homepage is set: the first page link
 * in reading order (`Site\IndexController::indexAction`).
 */
export const firstLinkedPage = (stored: readonly JsonValue[]): PageId | null => {
  const [first] = linkedPages(stored);
  return first ?? null;
};
