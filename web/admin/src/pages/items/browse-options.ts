/**
 * What the Items browse filters offer and what its rows name: every class
 * (with its standard), the classes items use, item set, template (with its
 * fields), user, site and property. Each list loads every page once, when the
 * screen opens, so a page change or a filter change costs one request.
 */
import { api } from '../../api/client';
import type { ResourceQuery } from '../../api/client';
import type { ClassCount } from '../../api/places';
import { listVocabularies } from '../../api/vocabularies';
import type {
  ApiPage,
  ClassId,
  ItemSetId,
  Property,
  PropertyId,
  SiteId,
  TemplateId,
  UserId,
} from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { classNames, namedClasses, useClassNameText } from '../../app/class-choices';
import type { NamedClass } from '../../app/class-choices';
import { userNames } from '../../app/labels';
import type { TermName } from '../../app/labels';
import { useTermLabels } from '../../i18n/termText';
import type { TermLabels } from '../../i18n/termText';
import { plainName } from '../../app/plain-terms';

/** One field of an item type: its property and the name the type gives it, if any. */
export interface TypeField {
  readonly property: PropertyId;
  readonly alternateLabel: string | null;
}

export interface BrowseOptions {
  /** Every class, with its standard's label: what a bulk edit can assign. */
  readonly classes: readonly NamedClass[];
  /**
   * The classes items use, with their counts: what the Class filter offers.
   * The list reads them apart from the rest, again after a bulk change, and
   * `null` stands until they answer.
   */
  readonly usedClasses: readonly ClassCount[] | null;
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
  usedClasses: null,
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
  const [classes, vocabularies, sets, templates, users, sites, properties] = await Promise.all([
    every((query) => api.listResourceClasses(query)),
    every((query) => listVocabularies(api, query)),
    every((query) => api.listItemSets(query)),
    every((query) => api.listResourceTemplates(query)),
    every((query) => api.listUsers(query)),
    every((query) => api.listSites(query)),
    every((query) => api.listProperties(query)),
  ]);
  return {
    classes: namedClasses(classes, vocabularies),
    usedClasses: null,
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

/** Each class of `classes` in words, told apart from the others of `classes` that share its label. */
export const useClassNamesOf = (classes: readonly NamedClass[]): ReadonlyMap<ClassId, string> => {
  const text = useTermLabels();
  const words = useClassNameText();
  return new Map([...classNames(classes, text)].map(([id, name]) => [id, words(name)]));
};

/**
 * Each class as the Items screen names it in rows and chips: told apart within
 * the classes the Class filter offers (those items use, and `held`), so a row,
 * a chip and the filter's choice say the same. A class outside them is told
 * apart within every class.
 */
export const useItemClassNames = (options: BrowseOptions, held: ClassId | null): ReadonlyMap<ClassId, string> => {
  const used = options.usedClasses ?? [];
  const heldClass =
    held === null || used.some((entry) => entry.id === held) ? [] : options.classes.filter((entry) => entry.id === held);
  const offered = useClassNamesOf([...used, ...heldClass]);
  const every = useClassNamesOf(options.classes);
  return new Map([...every, ...offered]);
};

/** A field as the field filter names it. */
export interface FieldChoice {
  readonly property: PropertyId;
  readonly name: string;
}

/**
 * A field's name: the one the filtered item type gives it, else its plain name (`plainName`),
 * else `#id` until the lists arrive.
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
  const stored = options.properties.get(property);
  return stored === undefined ? (own ?? `#${String(property)}`) : plainName(stored, text, own);
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
