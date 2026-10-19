import type { ReactElement, ReactNode } from 'react';
import { useState } from 'react';
import type { ApiPage } from '../api/types';
import { useTranslator } from '../i18n';
import { ErrorNotice } from './errors';
import { TableScroll } from './table-scroll';
import { useAsync } from './useAsync';
import { common } from '../i18n/namespaces/common';

export interface Column<T> {
  readonly header: string;
  readonly cell: (row: T) => ReactNode;
  /** Right-aligns the column and uses tabular numerals (ids, counts). */
  readonly numeric?: boolean;
}

export interface ListPageProps<T> {
  /** Plural resource name for the loading and pagination messages (`media`). */
  readonly resourceLabel: string;
  /** Text shown in the table body when the page has no rows. */
  readonly emptyMessage: string;
  readonly load: (page: number) => Promise<ApiPage<T>>;
  readonly columns: readonly Column<T>[];
  readonly rowKey: (row: T) => string;
}

/** Replaces the rows of a page while keeping its pagination. */
export const mapPage = <T, U>(page: ApiPage<T>, map: (row: T) => U): ApiPage<U> => ({
  ...page,
  items: page.items.map(map),
});

export interface PagerProps {
  readonly page: number;
  readonly lastPage: number;
  readonly total: number;
  readonly label: string;
  readonly onPage: (page: number) => void;
}

export const Pager = ({ page, lastPage, total, label, onPage }: PagerProps): ReactElement => {
  const { t } = useTranslator(common);
  return (
    <p className="pager">
      {t('list.pager', { page, lastPage, total, resource: label })}
      <button
        type="button"
        disabled={page <= 1}
        onClick={() => {
          onPage(page - 1);
        }}
      >
        {t('action.previous')}
      </button>{' '}
      <button
        type="button"
        disabled={page >= lastPage}
        onClick={() => {
          onPage(page + 1);
        }}
      >
        {t('action.next')}
      </button>
    </p>
  );
};

/**
 * A paged browse table for one API resource. Every listing page uses this so
 * the loading, error, empty and pagination states stay identical.
 */
export const ListPage = <T,>({
  resourceLabel,
  emptyMessage,
  load,
  columns,
  rowKey,
}: ListPageProps<T>): ReactElement => {
  const { t } = useTranslator(common);
  const [page, setPage] = useState(1);
  const state = useAsync(() => load(page), [page]);

  if (state.status === 'loading') {
    return <p role="status">{t('list.loading', { resource: resourceLabel })}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const view = state.data;
  return (
    <section>
      <TableScroll label={t('list.tableLabel', { resource: resourceLabel })}>
        <table>
          <thead>
            <tr>
              {columns.map((column) => (
                <th key={column.header} className={column.numeric === true ? 'cell-num' : undefined}>
                  {column.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {view.items.length === 0 ? (
              <tr>
                <td colSpan={columns.length}>{emptyMessage}</td>
              </tr>
            ) : (
              view.items.map((row) => (
                <tr key={rowKey(row)}>
                  {columns.map((column) => (
                    <td
                      key={column.header}
                      className={column.numeric === true ? 'cell-num' : undefined}
                    >
                      {column.cell(row)}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </TableScroll>
      <Pager
        page={view.page}
        lastPage={view.lastPage}
        total={view.total}
        label={resourceLabel}
        onPage={setPage}
      />
    </section>
  );
};
