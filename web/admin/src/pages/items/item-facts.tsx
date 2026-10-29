/**
 * An item's item type and class as the browse rows and cards show them: each its own filter.
 * An item with neither shows nothing in its place.
 */
import type { ReactElement } from 'react';
import type { ClassId, Item } from '../../api/types';
import { useTranslator } from '../../i18n';
import { resources } from '../../i18n/namespaces/resources';
import type { BrowseOptions } from './browse-options';
import { FilterValue } from './filter-value';
import type { OnFilter } from './filter-value';

export const ItemTypeValue = ({
  item,
  options,
  onFilter,
}: {
  readonly item: Item;
  readonly options: BrowseOptions;
  readonly onFilter: OnFilter;
}): ReactElement | null => {
  const { t } = useTranslator(resources);
  const type = item.resourceTemplate;
  return type === null ? null : (
    <FilterValue
      label={options.templates.get(type.id) ?? `#${String(type.id)}`}
      title={t('browse.onlyTemplate')}
      change={{ templateId: type.id }}
      onFilter={onFilter}
    />
  );
};

export const ClassValue = ({
  item,
  classNames,
  onFilter,
}: {
  readonly item: Item;
  /** `classNamesOf` for the whole list, read once by the view. */
  readonly classNames: ReadonlyMap<ClassId, string>;
  readonly onFilter: OnFilter;
}): ReactElement | null => {
  const { t } = useTranslator(resources);
  const resourceClass = item.resourceClass;
  return resourceClass === null ? null : (
    <FilterValue
      label={classNames.get(resourceClass.id) ?? `#${String(resourceClass.id)}`}
      title={t('browse.onlyClass')}
      change={{ classId: resourceClass.id }}
      onFilter={onFilter}
    />
  );
};
