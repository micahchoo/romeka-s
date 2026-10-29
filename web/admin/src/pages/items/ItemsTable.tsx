/**
 * The Items browse table: one row per item, with the item type, the class (while any item has
 * one), item sets, owner and the Private cue as values that filter the list.
 */
import type { ReactElement } from 'react';
import type { ClassId, Item, ItemId } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { ownerNameOf } from '../../app/labels';
import { TableScroll } from '../../app/table-scroll';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useItemClassNames } from './browse-options';
import type { BrowseOptions } from './browse-options';
import { FilterValue } from './filter-value';
import type { OnFilter } from './filter-value';
import { ClassValue, ItemTypeValue } from './item-facts';
import { PrivateCue } from './PrivateCue';

export interface ItemsViewProps {
  readonly items: readonly Item[];
  readonly options: BrowseOptions;
  /** Whether any item the caller may see has a class: only then does the class show. */
  readonly showClass: boolean;
  readonly picked: readonly ItemId[];
  readonly onPick: (id: ItemId, on: boolean) => void;
  readonly onPickPage: (on: boolean) => void;
  readonly onFilter: OnFilter;
  /** Each row's item page, which keeps this list and the row's place in it. */
  readonly hrefOf: (item: Item) => string;
}

const ItemRow = ({
  item,
  options,
  classNames,
  picked,
  onPick,
  onFilter,
  hrefOf,
}: Omit<ItemsViewProps, 'items' | 'picked' | 'onPickPage' | 'showClass'> & {
  readonly item: Item;
  readonly picked: boolean;
  /** The class names, or `null` while no item has a class. */
  readonly classNames: ReadonlyMap<ClassId, string> | null;
}): ReactElement => {
  const { t, locale } = useTranslator(common, resources);
  const title = item.title ?? t('common.untitled');
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
        <a href={hrefOf(item)}>{title}</a>
        {item.isPublic ? null : <> <PrivateCue onFilter={onFilter} /></>}
      </td>
      <td>
        <ItemTypeValue item={item} options={options} onFilter={onFilter} />
      </td>
      {classNames === null ? null : (
        <td>
          <ClassValue item={item} classNames={classNames} onFilter={onFilter} />
        </td>
      )}
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
  showClass,
  picked,
  onPick,
  onPickPage,
  onFilter,
  hrefOf,
}: ItemsViewProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const names = useItemClassNames(options, null);
  const classNames = showClass ? names : null;
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
            <th>{t('browse.template')}</th>
            {classNames === null ? null : <th>{t('column.class')}</th>}
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
              classNames={classNames}
              picked={picked.includes(item.id)}
              onPick={onPick}
              onFilter={onFilter}
              hrefOf={hrefOf}
            />
          ))}
        </tbody>
      </table>
    </TableScroll>
  );
};
