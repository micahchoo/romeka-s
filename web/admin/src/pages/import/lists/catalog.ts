/**
 * What the install already has that a change to many can name: its item sets
 * and item types, every page of each. A set or a type is sent by its id, which
 * the server reads before any title (`slots/named.rs`), so two sets with one
 * title never make the change ambiguous.
 */
import { api } from '../../../api/client';
import type { ItemSet, Property, ResourceTemplate } from '../../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../../app/all-pages';

export interface Catalog {
  readonly itemSets: readonly ItemSet[];
  readonly templates: readonly ResourceTemplate[];
}

export const loadCatalog = async (): Promise<Catalog> => {
  const [itemSets, templates] = await Promise.all([
    allPages((page) => api.listItemSets({ page, perPage: PER_PAGE, sortBy: 'title' })),
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
  ]);
  return { itemSets, templates };
};

/** Every field of the install, for the columns of the list to fill in. */
export const loadProperties = (): Promise<readonly Property[]> =>
  allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' }));
