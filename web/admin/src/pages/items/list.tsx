import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ClassId, Item, ItemId, UserId } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { useTermLabels } from '../../i18n/termText';
import { BulkSiteActions } from './BulkSiteActions';
import { classLabelOf, classLabels as buildClassLabels, ownerNameOf, userNames } from './labels';
import type { TermName } from './labels';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

const PER_PAGE = 25;

interface ItemsView {
  readonly items: readonly Item[];
  readonly total: number;
  readonly page: number;
  readonly lastPage: number;
  readonly names: ReadonlyMap<UserId, string>;
  readonly classLabels: ReadonlyMap<ClassId, TermName>;
}

export const ItemsList = (): ReactElement => {
  const { t, locale } = useTranslator(common, resources);
  const text = useTermLabels();
  const [page, setPage] = useState(1);
  const [picked, setPicked] = useState<readonly ItemId[]>([]);

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
  const pageIds = view.items.map((item) => item.id);
  const allPicked = pageIds.length > 0 && pageIds.every((id) => picked.includes(id));
  const pick = (id: ItemId, on: boolean): void => {
    setPicked((current) => (on ? [...current, id] : current.filter((entry) => entry !== id)));
  };
  return (
    <section>
      <p>
        <a href="#/items/new">{t('items.add')}</a>
      </p>
      <BulkSiteActions picked={picked} />
      <table>
        <thead>
          <tr>
            <th>
              <input
                type="checkbox"
                aria-label={t('items.selectAll')}
                checked={allPicked}
                onChange={(event) => {
                  const on = event.target.checked;
                  setPicked((current) => [
                    ...current.filter((id) => !pageIds.includes(id)),
                    ...(on ? pageIds : []),
                  ]);
                }}
              />
            </th>
            <th>{t('column.title')}</th>
            <th>{t('column.resourceClass')}</th>
            <th>{t('column.owner')}</th>
            <th>{t('column.created')}</th>
          </tr>
        </thead>
        <tbody>
          {view.items.length === 0 ? (
            <tr>
              <td colSpan={5}>{t('items.empty')}</td>
            </tr>
          ) : (
            view.items.map((item) => (
              <tr key={item.id}>
                <td>
                  <input
                    type="checkbox"
                    aria-label={t('items.selectItem', { title: item.title ?? t('common.untitled') })}
                    checked={picked.includes(item.id)}
                    onChange={(event) => {
                      pick(item.id, event.target.checked);
                    }}
                  />
                </td>
                <td>
                  <a href={`#/items/${String(item.id)}`}>{item.title ?? t('common.untitled')}</a>
                </td>
                <td>{classLabelOf(view.classLabels, item.resourceClass, text) ?? ''}</td>
                <td>{ownerNameOf(view.names, item.owner) ?? t('common.noOwner')}</td>
                <td>{formatDateTime(item.created, locale)}</td>
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
