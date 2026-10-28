/**
 * What the Items browse filters offer and what its rows name: every class,
 * item set, template (with its fields), user, site and property. Each list
 * loads every page once, when the screen opens, so a page change or a filter
 * change costs one request.
 */
import { api } from '../../api/client';
import type { ResourceQuery } from '../../api/client';
import type {
  ApiPage,
  ClassId,
  ItemSetId,
  Property,
  PropertyId,
  ResourceClass,
  SiteId,
  TemplateId,
  UserId,
} from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { classLabels, userNames } from '../../app/labels';
import type { TermName } from '../../app/labels';
import type { TermLabels } from '../../i18n/termText';

/** One field of an item type: its property and the name the type gives it, if any. */
export interface TypeField {
  readonly property: PropertyId;
  readonly alternateLabel: string | null;
}

export interface BrowseOptions {
  readonly classes: readonly ResourceClass[];
  readonly classNames: ReadonlyMap<ClassId, TermName>;
  readonly itemSets: ReadonlyMap<ItemSetId, string>;
  readonly templates: ReadonlyMap<TemplateId, string>;
  readonly templateFields: ReadonlyMap<TemplateId, readonly TypeField[]>;
  readonly properties: ReadonlyMap<PropertyId, TermName>;
  readonly users: ReadonlyMap<UserId, string>;
  readonly sites: ReadonlyMap<SiteId, string>;
}

/** No names yet: the rows show `#id` until the lists arrive. */
export const NO_OPTIONS: BrowseOptions = {
  classes: [],
  classNames: new Map(),
  itemSets: new Map(),
  templates: new Map(),
  templateFields: new Map(),
  properties: new Map(),
  users: new Map(),
  sites: new Map(),
};

const every = <T>(load: (query: ResourceQuery) => Promise<ApiPage<T>>): Promise<readonly T[]> =>
  allPages((page) => load({ page, perPage: ALL_PAGES_PER_PAGE }));

const propertyNames = (properties: readonly Property[]): ReadonlyMap<PropertyId, TermName> =>
  new Map(properties.map((property) => [property.id, { term: property.term, label: property.label ?? property.term }]));

export const loadBrowseOptions = async (): Promise<BrowseOptions> => {
  const [classes, sets, templates, users, sites, properties] = await Promise.all([
    every((query) => api.listResourceClasses(query)),
    every((query) => api.listItemSets(query)),
    every((query) => api.listResourceTemplates(query)),
    every((query) => api.listUsers(query)),
    every((query) => api.listSites(query)),
    every((query) => api.listProperties(query)),
  ]);
  return {
    classes,
    classNames: classLabels(classes),
    itemSets: new Map(sets.map((set) => [set.id, set.title ?? `#${String(set.id)}`])),
    templates: new Map(templates.map((template) => [template.id, template.label])),
    templateFields: new Map(
      templates.map((template) => [
        template.id,
        template.properties.map((field) => ({ property: field.property.id, alternateLabel: field.alternateLabel })),
      ]),
    ),
    properties: propertyNames(properties),
    users: userNames(users),
    sites: new Map(sites.map((site) => [site.id, site.title])),
  };
};

/** A field as the field filter names it. */
export interface FieldChoice {
  readonly property: PropertyId;
  readonly name: string;
}

/**
 * A field's name: the one the filtered item type gives it, else its vocabulary label in the
 * reader's language, else `#id` until the lists arrive.
 */
export const fieldName = (
  options: BrowseOptions,
  template: TemplateId | null,
  property: PropertyId,
  text: TermLabels,
): string => {
  const own = template === null
    ? null
    : options.templateFields.get(template)?.find((field) => field.property === property)?.alternateLabel ?? null;
  if (own !== null) {
    return own;
  }
  const stored = options.properties.get(property);
  return stored === undefined ? `#${String(property)}` : text.label(stored.term, stored.label);
};

/**
 * The fields the field filter offers: the filtered item type's own, else every item type's, by
 * name, with the chosen field kept so the control always shows it.
 */
export const fieldChoices = (
  options: BrowseOptions,
  template: TemplateId | null,
  chosen: PropertyId | null,
  text: TermLabels,
): readonly FieldChoice[] => {
  const fields =
    template === null ? [...options.templateFields.values()].flat() : (options.templateFields.get(template) ?? []);
  const ids = new Set([...fields.map((field) => field.property), ...(chosen === null ? [] : [chosen])]);
  return [...ids]
    .map((property) => ({ property, name: fieldName(options, template, property, text) }))
    .sort((a, b) => a.name.localeCompare(b.name));
};
