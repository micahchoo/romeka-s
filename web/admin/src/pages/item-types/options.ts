/** The reference lists the item type pages read, every page of each. */
import { api } from '../../api/client';
import type { DataType, Property, ResourceClass, User } from '../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../app/all-pages';

export interface TypeOptions {
  readonly users: readonly User[];
  readonly classes: readonly ResourceClass[];
  readonly properties: readonly Property[];
  readonly dataTypes: readonly DataType[];
}

export const loadTypeOptions = async (): Promise<TypeOptions> => {
  const [users, classes, properties, dataTypes] = await Promise.all([
    allPages((page) => api.listUsers({ page, perPage: PER_PAGE, sortBy: 'name' })),
    allPages((page) => api.listResourceClasses({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listDataTypes({ page, perPage: PER_PAGE })),
  ]);
  return { users, classes, properties, dataTypes };
};
