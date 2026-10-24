/**
 * What the Items browse filters offer and what its rows name: every class,
 * item set, template, user and site. Each list loads every page once, when the
 * screen opens, so a page change or a filter change costs one request.
 */
import { api } from '../../api/client';
import type { ResourceQuery } from '../../api/client';
import type {
  ApiPage,
  ClassId,
  ItemSetId,
  ResourceClass,
  SiteId,
  TemplateId,
  UserId,
} from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { classLabels, userNames } from '../../app/labels';
import type { TermName } from '../../app/labels';

export interface BrowseOptions {
  readonly classes: readonly ResourceClass[];
  readonly classNames: ReadonlyMap<ClassId, TermName>;
  readonly itemSets: ReadonlyMap<ItemSetId, string>;
  readonly templates: ReadonlyMap<TemplateId, string>;
  readonly users: ReadonlyMap<UserId, string>;
  readonly sites: ReadonlyMap<SiteId, string>;
}

/** No names yet: the rows show `#id` until the lists arrive. */
export const NO_OPTIONS: BrowseOptions = {
  classes: [],
  classNames: new Map(),
  itemSets: new Map(),
  templates: new Map(),
  users: new Map(),
  sites: new Map(),
};

const every = <T>(load: (query: ResourceQuery) => Promise<ApiPage<T>>): Promise<readonly T[]> =>
  allPages((page) => load({ page, perPage: ALL_PAGES_PER_PAGE }));

export const loadBrowseOptions = async (): Promise<BrowseOptions> => {
  const [classes, sets, templates, users, sites] = await Promise.all([
    every((query) => api.listResourceClasses(query)),
    every((query) => api.listItemSets(query)),
    every((query) => api.listResourceTemplates(query)),
    every((query) => api.listUsers(query)),
    every((query) => api.listSites(query)),
  ]);
  return {
    classes,
    classNames: classLabels(classes),
    itemSets: new Map(sets.map((set) => [set.id, set.title ?? `#${String(set.id)}`])),
    templates: new Map(templates.map((template) => [template.id, template.label])),
    users: userNames(users),
    sites: new Map(sites.map((site) => [site.id, site.title])),
  };
};
