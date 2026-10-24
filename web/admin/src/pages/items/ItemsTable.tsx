/**
 * The Items browse table: one row per item, with the class, item sets, owner
 * and the Private cue as values that filter the list.
 */
import type { ReactElement } from 'react';
import type { Item, ItemId } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { classLabelOf, ownerNameOf } from '../../app/labels';
import { TableScroll } from '../../app/table-scroll';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import type { BrowseOptions } from './browse-options';
import { FilterValue } from './filter-value';
import type { OnFilter } from './filter-value';
import { PrivateCue } from './PrivateCue';

export interface ItemsViewProps {
  readonly items: readonly Item[];
  readonly options: BrowseOptions;
  readonly picked: readonly ItemId[];
  readonly onPick: (id: ItemId, on: boolean) => void;
  readonly onPickPage: (on: boolean) => void;
  readonly onFilter: OnFilter;
}

const ItemRow = ({
  item,
  options,
  picked,
  onPick,
  onFilter,
}: Omit<ItemsViewProps, 'items' | 'picked' | 'onPickPage'> & {
  readonly item: Item;
  readonly picked: boolean;
}): ReactElement => {
  const { t, locale } = useTranslator(common, resources);
  const text = useTermLabels();
  const title = item.title ?? t('common.untitled');
  const className = classLabelOf(options.classNames, item.resourceClass, text);
  const owner = ownerNameOf(options.users, item.owner);
  return (
    <tr>
      <td>
        <input
          type="checkbox"
          aria-label={t('items.selectItem', { title })}
          checked={picked}
          onChange={(event) => {
            onPick(item.id, event.target.checked);
          }}
        />
      </td>
      <td>
        <a href={`#/items/${String(item.id)}`}>{title}</a>
        {item.isPublic ? null : <> <PrivateCue onFilter={onFilter} /></>}
      </td>
      <td>
        {className === null || item.resourceClass === null ? null : (
          <FilterValue
            label={className}
            title={t('browse.onlyClass')}
            change={{ classId: item.resourceClass.id }}
            onFilter={onFilter}
          />
        )}
      </td>
      <td>
        <ul className="filter-value-list">
          {item.itemSets.map((set) => (
            <li key={set.id}>
              <FilterValue
                label={options.itemSets.get(set.id) ?? `#${String(set.id)}`}
                title={t('browse.onlyItemSet')}
                change={{ itemSetId: set.id }}
                onFilter={onFilter}
              />
            </li>
          ))}
        </ul>
      </td>
      <td>
        {owner === null || item.owner === null ? (
          t('common.noOwner')
        ) : (
          <FilterValue
            label={owner}
            title={t('browse.onlyOwner')}
            change={{ ownerId: item.owner.id }}
            onFilter={onFilter}
          />
        )}
      </td>
      <td>{formatDateTime(item.created, locale)}</td>
    </tr>
  );
};

export const ItemsTable = ({
  items,
  options,
  picked,
  onPick,
  onPickPage,
  onFilter,
}: ItemsViewProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const allPicked = items.length > 0 && items.every((item) => picked.includes(item.id));
  return (
    <TableScroll label={t('list.tableLabel', { resource: t('resource.items') })}>
      <table>
        <thead>
          <tr>
            <th>
              <input
                type="checkbox"
                aria-label={t('items.selectAll')}
                checked={allPicked}
                onChange={(event) => {
                  onPickPage(event.target.checked);
                }}
              />
            </th>
            <th>{t('column.title')}</th>
            <th>{t('column.resourceClass')}</th>
            <th>{t('items.setsLegend')}</th>
            <th>{t('column.owner')}</th>
            <th>{t('column.created')}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((item) => (
            <ItemRow
              key={item.id}
              item={item}
              options={options}
              picked={picked.includes(item.id)}
              onPick={onPick}
              onFilter={onFilter}
            />
          ))}
        </tbody>
      </table>
    </TableScroll>
  );
};
