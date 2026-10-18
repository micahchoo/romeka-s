import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ClassId, Item, UserId } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { classLabelOf, classLabels as buildClassLabels, ownerNameOf, userNames } from './labels';

const PER_PAGE = 25;

interface ItemsView {
  readonly items: readonly Item[];
  readonly total: number;
  readonly page: number;
  readonly lastPage: number;
  readonly names: ReadonlyMap<UserId, string>;
  readonly classLabels: ReadonlyMap<ClassId, string>;
}

export const ItemsList = (): ReactElement => {
  const { t } = useTranslator();
  const [page, setPage] = useState(1);

  const state = useAsync(async () => {
    // Upstream admin browse defaults: sort_by=id, sort_order=desc.
    const [items, users, classes] = await Promise.all([
      api.listItems({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'desc' }),
      api.listUsers({ perPage: 100 }),
      api.listResourceClasses({ perPage: 100 }),
    ]);
    const view: ItemsView = {
      items: items.items,
      total: items.total,
      page: items.page,
      lastPage: items.lastPage,
      names: userNames(users.items),
      classLabels: buildClassLabels(classes.items),
    };
    return view;
  }, [page]);

  if (state.status === 'loading') {
    return <p role="status">{t('list.loading', { resource: t('resource.items') })}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const view = state.data;
  return (
    <section>
      <p>
        <a href="#/items/new">{t('items.add')}</a>
      </p>
      <table>
        <thead>
          <tr>
            <th>{t('column.title')}</th>
            <th>{t('column.resourceClass')}</th>
            <th>{t('column.owner')}</th>
            <th>{t('column.created')}</th>
          </tr>
        </thead>
        <tbody>
          {view.items.length === 0 ? (
            <tr>
              <td colSpan={4}>{t('items.empty')}</td>
            </tr>
          ) : (
            view.items.map((item) => (
              <tr key={item.id}>
                <td>
                  <a href={`#/items/${String(item.id)}`}>{item.title ?? t('common.untitled')}</a>
                </td>
                <td>{classLabelOf(view.classLabels, item.resourceClass) ?? ''}</td>
                <td>{ownerNameOf(view.names, item.owner) ?? t('common.noOwner')}</td>
                <td>{formatDateTime(item.created)}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
      <p>
        {t('list.pager', {
          page: view.page,
          lastPage: view.lastPage,
          total: view.total,
          resource: t('resource.items'),
        })}
        <button
          type="button"
          disabled={view.page <= 1}
          onClick={() => {
            setPage(view.page - 1);
          }}
        >
          {t('action.previous')}
        </button>{' '}
        <button
          type="button"
          disabled={view.page >= view.lastPage}
          onClick={() => {
            setPage(view.page + 1);
          }}
        >
          {t('action.next')}
        </button>
      </p>
    </section>
  );
};
