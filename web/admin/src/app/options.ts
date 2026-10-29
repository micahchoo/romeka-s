/**
 * The reference lists every write form needs, every page of each, plus the
 * lookups the forms do on them. Ids come back through the parsers, so the
 * option lists are typed before any form sees them.
 */
import { api } from '../api/client';
import type { TermLabels } from '../i18n';
import type {
  DataType,
  ItemSet,
  Property,
  ResourceClass,
  ResourceTemplate,
  User,
  Vocabulary,
} from '../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from './all-pages';
import { classNames, loadClassesAndStandards, namedClasses } from './class-choices';
import type { ClassName } from './class-choices';

export interface FormOptions {
  readonly users: readonly User[];
  readonly classes: readonly ResourceClass[];
  /** Every standard, so a class whose label another class shares is named with its standard. */
  readonly vocabularies: readonly Vocabulary[];
  readonly templates: readonly ResourceTemplate[];
  readonly properties: readonly Property[];
  readonly itemSets: readonly ItemSet[];
  readonly dataTypes: readonly DataType[];
}

export const loadFormOptions = async (): Promise<FormOptions> => {
  const [users, { classes, vocabularies }, templates, properties, itemSets, dataTypes] = await Promise.all([
    allPages((page) => api.listUsers({ page, perPage: PER_PAGE, sortBy: 'name' })),
    loadClassesAndStandards(),
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listItemSets({ page, perPage: PER_PAGE, sortBy: 'title' })),
    allPages((page) => api.listDataTypes({ page, perPage: PER_PAGE })),
  ]);
  return { users, classes, vocabularies, templates, properties, itemSets, dataTypes };
};

export interface Option<IdT extends number> {
  readonly id: IdT;
  readonly label: string;
}

export const userOptions = (users: readonly User[]): readonly Option<User['id']>[] =>
  users.map((user) => ({ id: user.id, label: user.name }));

/**
 * Each class by its label in the locale `text` speaks, with its standard when
 * another class shares that label: "Image (Dublin Core Type)".
 */
export const classOptions = (
  classes: readonly ResourceClass[],
  vocabularies: readonly Vocabulary[],
  text: TermLabels,
  words: (name: ClassName) => string,
): readonly Option<ResourceClass['id']>[] => {
  const names = classNames(namedClasses(classes, vocabularies), text);
  return classes.map((entry) => {
    const name = names.get(entry.id);
    return { id: entry.id, label: name === undefined ? entry.term : words(name) };
  });
};

export const templateOptions = (
  templates: readonly ResourceTemplate[],
): readonly Option<ResourceTemplate['id']>[] =>
  templates.map((entry) => ({ id: entry.id, label: entry.label }));

export const selectedOption = <IdT extends number>(
  options: readonly Option<IdT>[],
  value: string,
): IdT | null => options.find((option) => String(option.id) === value)?.id ?? null;
