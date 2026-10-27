import type { ReactElement } from 'react';
import { useId, useState } from 'react';
import { api } from '../../api/client';
import { placesApi } from '../../api/places';
import type { ApiPage, ItemSet } from '../../api/types';
import { useSession } from '../../app/auth';
import { formatDateTime } from '../../app/format';
import { ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { setPlaceHref } from './facts';

const PER_PAGE = 25;

/** A set's item count: the number, absent from the answer (the set went or is hidden), or a failed read. */
type Count =
  | { readonly status: 'ready'; readonly items: number }
  | { readonly status: 'missing' }
  | { readonly status: 'error'; readonly message: string };

interface ItemSetRow {
  readonly itemSet: ItemSet;
  readonly ownerName: string | null;
  readonly count: Count;
}

type SetSort = 'oldest' | 'most' | 'fewest';

const SORTS: readonly SetSort[] = ['oldest', 'most', 'fewest'];

const SORT_QUERY = {
  oldest: { sortBy: 'id', sortOrder: 'asc' },
  most: { sortBy: 'item_count', sortOrder: 'desc' },
  fewest: { sortBy: 'item_count', sortOrder: 'asc' },
} as const;

const SORT_LABEL = {
  oldest: 'browse.sortOldest',
  most: 'itemSets.sortMost',
  fewest: 'itemSets.sortFewest',
} as const;

/**
 * `sort_by=item_count` orders by every item a set holds, as upstream does, while the column
 * counts only what the caller may see. The two agree for the roles that see every item
 * (`Actor::is_privileged` on the server), so only they are offered the order.
 */
const SEES_EVERY_ITEM: ReadonlySet<string> = new Set(['global_admin', 'site_admin', 'editor', 'reviewer']);

/** One counts request per page of sets. A failed one leaves the rows and marks each count. */
const countsOf = async (sets: readonly ItemSet[]): Promise<(set: ItemSet) => Count> => {
  try {
    const counts = await placesApi.setItemCounts(sets.map((set) => set.id));
    return (set) => {
      const items = counts.get(set.id);
      return items === undefined ? { status: 'missing' } : { status: 'ready', items };
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return () => ({ status: 'error', message });
  }
};

const loader =
  (sort: SetSort) =>
  async (page: number): Promise<ApiPage<ItemSetRow>> => {
    const [itemSets, users] = await Promise.all([
      api.listItemSets({ page, perPage: PER_PAGE, ...SORT_QUERY[sort] }),
      api.listUsers({ perPage: 100 }),
    ]);
    const names = userNames(users.items);
    const countOf = await countsOf(itemSets.items);
    return mapPage(itemSets, (itemSet) => ({
      itemSet,
      ownerName: ownerNameOf(names, itemSet.owner),
      count: countOf(itemSet),
    }));
  };

/** The count as a link to the set's place; a dash, with the reason, when there is no count. */
const CountCell = ({ row }: { readonly row: ItemSetRow }): ReactElement => {
  const { t, number } = useTranslator(resources);
  const { count } = row;
  if (count.status !== 'ready') {
    const why = count.status === 'error' ? count.message : t('itemSets.noCount');
    return (
      <span role="img" title={why} aria-label={why}>
        —
      </span>
    );
  }
  const words = count.items === 1 ? t('setPlace.countOne') : t('setPlace.countMany', { count: number(count.items) });
  return (
    <a href={setPlaceHref(row.itemSet.id)} aria-label={words}>
      {number(count.items)}
    </a>
  );
};

/** `Admin/ItemSetController::browseAction`: the item set browse table, with each set's item count. */
export const ItemSetList = (): ReactElement => {
  const { t, locale } = useTranslator(common, resources);
  const role = useSession()?.role ?? '';
  const [chosen, setChosen] = useState<SetSort>('oldest');
  const sortId = useId();
  const resource = t('resource.itemSets');
  const sortable = SEES_EVERY_ITEM.has(role);
  const sort = sortable ? chosen : 'oldest';
  return (
    <div>
      <div className="item-sets-head">
        <a href="#/item-sets/new">{t('itemSets.add')}</a>
        {sortable ? (
          <span className="item-sets-sort">
            <label htmlFor={sortId}>{t('browse.sort')}</label>
            <select
              id={sortId}
              value={sort}
              onChange={(event) => {
                setChosen(SORTS.find((entry) => entry === event.target.value) ?? 'oldest');
              }}
            >
              {SORTS.map((entry) => (
                <option key={entry} value={entry}>
                  {t(SORT_LABEL[entry])}
                </option>
              ))}
            </select>
          </span>
        ) : null}
      </div>
      <ListPage<ItemSetRow>
        key={sort}
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={loader(sort)}
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
          { header: t('itemSets.itemsColumn'), numeric: true, cell: (row) => <CountCell row={row} /> },
          { header: t('column.owner'), cell: (row) => row.ownerName ?? t('common.noOwner') },
          { header: t('column.created'), cell: (row) => formatDateTime(row.itemSet.created, locale) },
          { header: t('column.public'), cell: (row) => (row.itemSet.isPublic ? t('common.yes') : t('common.no')) },
          { header: t('column.open'), cell: (row) => (row.itemSet.isOpen ? t('common.yes') : t('common.no')) },
        ]}
      />
    </div>
  );
};
