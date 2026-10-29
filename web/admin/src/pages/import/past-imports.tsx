/**
 * "Past imports": the history, newest first, a page of `PER_PAGE` at a time
 * (`GET /api/romeka/imports?page=&perPage=`). Each row opens its import, and
 * offers a copy, "Try again" when its run may go again, and Discard when it
 * added nothing.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../api/import-routes';
import type { ImportRow } from '../../api/import-view';
import { ErrorNotice } from '../../app/errors';
import { formatDateTime } from '../../app/format';
import type { Column } from '../../app/list';
import { DiscardImport } from './discard';
import { useImportText } from './messages';
import { PagedList } from './paged-list';
import { importHref, placeHref } from './screens';
import { useCopy } from './use-copy';
import { canRetry, statusKey, whereWords } from './words';

const loadHistory = async (page: number, perPage: number) => importApi.history({ page, perPage });

export const PastImports = (): ReactElement => {
  const { t, locale, number } = useImportText();
  const { state, copy } = useCopy();
  // A discard reads the history again; the count names the reload for PagedList.
  const [discards, setDiscards] = useState(0);
  const columns: readonly Column<ImportRow>[] = [
    {
      header: t('home.past.colFiles'),
      cell: (row) => <a href={importHref(row.id)}>{whereWords(row.sources, t)}</a>,
    },
    { header: t('home.past.colStatus'), cell: (row) => t(statusKey(row)) },
    { header: t('home.past.colStarted'), cell: (row) => formatDateTime(row.created, locale) },
    {
      header: t('home.past.colFinished'),
      cell: (row) => (row.finished === null ? '' : formatDateTime(row.finished, locale)),
    },
    {
      header: t('home.past.colAdded'),
      numeric: true,
      cell: (row) =>
        row.summary === null
          ? ''
          : t('home.past.added', {
              done: number(row.summary.progress.done),
              total: number(row.summary.progress.total),
            }),
    },
    {
      header: t('home.past.colMore'),
      cell: (row) => {
        const copying = state.status === 'copying' && state.from === row.id;
        return (
          <div className="import-row-actions">
            <button
              type="button"
              disabled={state.status === 'copying'}
              onClick={() => {
                copy(row.id);
              }}
            >
              {copying ? t('home.past.copying') : t('home.past.copy')}
            </button>
            {canRetry(row) ? <a href={placeHref(row.id, { mode: 'simple', screen: 'add' })}>{t('home.past.retry')}</a> : null}
            {row.status === 'draft' ? (
              <DiscardImport
                id={row.id}
                onDiscarded={() => {
                  setDiscards((count) => count + 1);
                }}
              />
            ) : null}
            {state.status === 'failed' && state.from === row.id ? <ErrorNotice error={state.error} /> : null}
          </div>
        );
      },
    },
  ];
  return (
    <section className="import-past">
      <h2>{t('home.past.title')}</h2>
      <PagedList
        label={t('home.past.title')}
        resource={t('home.past.resource')}
        load={loadHistory}
        deps={[discards]}
        columns={columns}
        rowKey={(row) => String(row.id)}
        empty={<p>{t('home.past.empty')}</p>}
      />
    </section>
  );
};
