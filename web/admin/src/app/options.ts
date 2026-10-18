/**
 * The reference lists every write form needs, in one request batch, plus the
 * lookups the forms do on them. Ids come back through the parsers, so the
 * option lists are typed before any form sees them.
 */
import { api } from '../api/client';
import type { ItemSet, Property, ResourceClass, ResourceTemplate, User } from '../api/types';

const PER_PAGE = 200;

export interface FormOptions {
  readonly users: readonly User[];
  readonly classes: readonly ResourceClass[];
  readonly templates: readonly ResourceTemplate[];
  readonly properties: readonly Property[];
  readonly itemSets: readonly ItemSet[];
}

export const loadFormOptions = async (): Promise<FormOptions> => {
  const [users, classes, templates, properties, itemSets] = await Promise.all([
    api.listUsers({ perPage: PER_PAGE, sortBy: 'name' }),
    api.listResourceClasses({ perPage: PER_PAGE, sortBy: 'label' }),
    api.listResourceTemplates({ perPage: PER_PAGE, sortBy: 'label' }),
    api.listProperties({ perPage: PER_PAGE, sortBy: 'label' }),
    api.listItemSets({ perPage: PER_PAGE, sortBy: 'title' }),
  ]);
  return {
    users: users.items,
    classes: classes.items,
    templates: templates.items,
    properties: properties.items,
    itemSets: itemSets.items,
  };
};

export interface Option<IdT extends number> {
  readonly id: IdT;
  readonly label: string;
}

export const userOptions = (users: readonly User[]): readonly Option<User['id']>[] =>
  users.map((user) => ({ id: user.id, label: user.name }));

export const classOptions = (
  classes: readonly ResourceClass[],
): readonly Option<ResourceClass['id']>[] =>
  classes.map((entry) => ({ id: entry.id, label: entry.label ?? entry.term }));

export const templateOptions = (
  templates: readonly ResourceTemplate[],
): readonly Option<ResourceTemplate['id']>[] =>
  templates.map((entry) => ({ id: entry.id, label: entry.label }));

export const selectedOption = <IdT extends number>(
  options: readonly Option<IdT>[],
  value: string,
): IdT | null => options.find((option) => String(option.id) === value)?.id ?? null;

export const propertyById = (properties: readonly Property[], id: number): Property | null =>
  properties.find((property) => property.id === id) ?? null;

export const propertyByTerm = (properties: readonly Property[], term: string): Property | null =>
  properties.find((property) => property.term === term) ?? null;

/**
 * The property a resource's title is stored under: the template's title
 * property, or `dcterms:title` when no template is set. Mirrors
 * `ResourceTitleHydrator`.
 */
export const titleProperty = (
  properties: readonly Property[],
  templates: readonly ResourceTemplate[],
  templateId: number | null,
): Property | null => {
  const template = templates.find((entry) => entry.id === templateId) ?? null;
  const ref = template?.titleProperty ?? null;
  const fromTemplate = ref === null ? null : propertyById(properties, ref.id);
  return fromTemplate ?? propertyByTerm(properties, 'dcterms:title');
};
