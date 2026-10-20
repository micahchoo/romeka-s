/**
 * Every property, by label, for the controls that name one: the property
 * select, the property list and the sort select. The list loads when such a
 * control first shows, so a form with none never asks for it.
 */
import { api } from '../../../../api/client';
import type { Property } from '../../../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../../../app/all-pages';
import { useAsync } from '../../../../app/useAsync';
import type { AsyncState } from '../../../../app/useAsync';

const loadProperties = (): Promise<readonly Property[]> =>
  allPages((page) => api.listProperties({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'label' }));

export const useProperties = (): AsyncState<readonly Property[]> => useAsync(loadProperties, []);

/** A property's name in a select: its label and its term. */
export const propertyName = (property: Property): string =>
  `${property.label ?? property.term} (${property.term})`;
