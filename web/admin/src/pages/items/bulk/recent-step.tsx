/**
 * "Recent bulk changes": the person's last 25 changes (an administrator's list has everyone's),
 * newest first, each with what it did and its Undo while it can be undone.
 */
import type { ReactElement } from 'react';
import type { BulkRun } from '../../../api/bulk';
import { isLive, mayUndo } from '../../../api/bulk';
import { formatDateTime } from '../../../app/format';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { useTermLabels } from '../../../i18n/termText';
import type { BrowseOptions } from '../browse-options';
import { editWords } from './words';

export const RecentStep = ({
  runs,
  options,
  busy,
  onShow,
  onUndo,
}: {
  readonly runs: readonly BulkRun[] | null;
  readonly options: BrowseOptions;
  readonly busy: boolean;
  readonly onShow: (run: BulkRun) => void;
  readonly onUndo: (run: BulkRun) => void;
}): ReactElement => {
  const translator = useTranslator(bulk);
  const { t, number, locale } = translator;
  const text = useTermLabels();
  if (runs === null) {
    return <p role="status">{t('bulk.recent.loading')}</p>;
  }
  if (runs.length === 0) {
    return <p>{t('bulk.recent.none')}</p>;
  }
  return (
    <table className="bulk-recent">
      <caption>{t('bulk.recent.heading')}</caption>
      <thead>
        <tr>
          <th scope="col">{t('bulk.recent.when')}</th>
          <th scope="col">{t('bulk.recent.change')}</th>
          <th scope="col">{t('bulk.recent.status')}</th>
          <th scope="col">{t('bulk.recent.items')}</th>
          <td />
        </tr>
      </thead>
      <tbody>
        {runs.map((run) => (
          <tr key={String(run.id)}>
            <td>{formatDateTime(run.created, locale)}</td>
            <td>{editWords({ t, options, text }, run.change)}</td>
            <td>{t(`bulk.status.${run.status}`)}</td>
            <td>{t('bulk.recent.count', { done: number(run.done), total: number(run.total) })}</td>
            <td>
              {isLive(run.status) ? (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    onShow(run);
                  }}
                >
                  {t('bulk.recent.show')}
                </button>
              ) : mayUndo(run.status) ? (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    onUndo(run);
                  }}
                >
                  {t('bulk.done.undo')}
                </button>
              ) : null}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
};
