import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, ItemSet } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 25;

interface ItemSetRow {
  readonly itemSet: ItemSet;
  readonly ownerName: string | null;
}

const load = async (page: number): Promise<ApiPage<ItemSetRow>> => {
  const [itemSets, users] = await Promise.all([
    api.listItemSets({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listUsers({ perPage: 100 }),
  ]);
  const names = userNames(users.items);
  return mapPage(itemSets, (itemSet) => ({
    itemSet,
    ownerName: ownerNameOf(names, itemSet.owner),
  }));
};

/** `Admin/ItemSetController::browseAction`: the item set browse table. */
export const ItemSetList = (): ReactElement => {
  const { t } = useTranslator();
  const resource = t('resource.itemSets');
  return (
    <div>
      <p>
        <a href="#/item-sets/new">{t('itemSets.add')}</a>
      </p>
      <ListPage<ItemSetRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.itemSet.id)}
        columns={[
          {
            header: t('column.title'),
            cell: (row) => (
              <a href={`#/item-sets/${String(row.itemSet.id)}`}>
                {row.itemSet.title ?? t('common.untitled')}
              </a>
            ),
          },
          { header: t('column.owner'), cell: (row) => row.ownerName ?? t('common.noOwner') },
          { header: t('column.created'), cell: (row) => formatDateTime(row.itemSet.created) },
          { header: t('column.public'), cell: (row) => (row.itemSet.isPublic ? t('common.yes') : t('common.no')) },
          { header: t('column.open'), cell: (row) => (row.itemSet.isOpen ? t('common.yes') : t('common.no')) },
        ]}
      />
    </div>
  );
};
