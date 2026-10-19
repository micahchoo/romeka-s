/**
 * The reference lists every write form needs, every page of each, plus the
 * lookups the forms do on them. Ids come back through the parsers, so the
 * option lists are typed before any form sees them.
 */
import { api } from '../api/client';
import type {
  DataType,
  ItemSet,
  Property,
  ResourceClass,
  ResourceTemplate,
  User,
} from '../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from './all-pages';

export interface FormOptions {
  readonly users: readonly User[];
  readonly classes: readonly ResourceClass[];
  readonly templates: readonly ResourceTemplate[];
  readonly properties: readonly Property[];
  readonly itemSets: readonly ItemSet[];
  readonly dataTypes: readonly DataType[];
}

export const loadFormOptions = async (): Promise<FormOptions> => {
  const [users, classes, templates, properties, itemSets, dataTypes] = await Promise.all([
    allPages((page) => api.listUsers({ page, perPage: PER_PAGE, sortBy: 'name' })),
    allPages((page) => api.listResourceClasses({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listItemSets({ page, perPage: PER_PAGE, sortBy: 'title' })),
    allPages((page) => api.listDataTypes({ page, perPage: PER_PAGE })),
  ]);
  return { users, classes, templates, properties, itemSets, dataTypes };
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
