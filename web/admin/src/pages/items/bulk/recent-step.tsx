/**
 * "Recent bulk changes": the person's last 25 changes (an administrator's list has everyone's),
 * newest first, each with what it did and its Undo while it can be undone. While a delete in the
 * list still has items in the trash, an administrator can empty the trash for good here, next to
 * the Undo that emptying takes away.
 *
 * It is its own chunk, loaded when the list opens.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import type { BulkRun } from '../../../api/bulk';
import { isLive, undoable } from '../../../api/bulk';
import type { Emptied } from '../../../api/bulk-delete';
import { bulkDeleteApi } from '../../../api/bulk-delete';
import { useSession } from '../../../app/auth';
import { ErrorNotice } from '../../../app/errors';
import { formatDateTime } from '../../../app/format';
import { toError } from '../../../app/useAsync';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { bulkDelete } from '../../../i18n/namespaces/bulk-delete';
import { useTermLabels } from '../../../i18n/termText';
import type { BrowseOptions } from '../browse-options';
import { editWords } from './words';

/** The roles that may empty the trash (`empty_trash` in `bulk/trash.rs`). */
const ADMINS: ReadonlySet<string> = new Set(['global_admin', 'site_admin']);

type Emptying =
  | { readonly step: 'idle' }
  | { readonly step: 'asking' }
  | { readonly step: 'emptying' }
  | { readonly step: 'failed'; readonly error: Error };

/** "Empty the trash…" and its question. */
const EmptyTrash = ({ onEmptied }: { readonly onEmptied: (emptied: Emptied) => void }): ReactElement => {
  const { t } = useTranslator(bulkDelete);
  const [state, setState] = useState<Emptying>({ step: 'idle' });
  switch (state.step) {
    case 'idle':
      return (
        <p className="bulk-actions">
          <button
            type="button"
            className="link-button-danger"
            onClick={() => {
              setState({ step: 'asking' });
            }}
          >
            {t('bulkDelete.empty.open')}
          </button>
        </p>
      );
    case 'asking':
    case 'emptying':
      return (
        <div className="bulk-undo" role="group" aria-label={t('bulkDelete.empty.confirm')}>
          <p>{t('bulkDelete.empty.ask')}</p>
          <p className="bulk-actions">
            <button
              type="button"
              disabled={state.step === 'emptying'}
              aria-busy={state.step === 'emptying'}
              onClick={() => {
                setState({ step: 'emptying' });
                bulkDeleteApi.emptyTrash().then(
                  (emptied) => {
                    setState({ step: 'idle' });
                    onEmptied(emptied);
                  },
                  (error: unknown) => {
                    setState({ step: 'failed', error: toError(error) });
                  },
                );
              }}
            >
              {t('bulkDelete.empty.confirm')}
            </button>
            <button
              type="button"
              className="link-button"
              disabled={state.step === 'emptying'}
              onClick={() => {
                setState({ step: 'idle' });
              }}
            >
              {t('bulkDelete.empty.cancel')}
            </button>
          </p>
        </div>
      );
    case 'failed':
      return <ErrorNotice error={state.error} />;
  }
};

/** A delete whose items are still in the trash. */
const inTrash = (run: BulkRun): boolean => run.kind === 'delete' && undoable(run);

export const RecentStep = ({
  runs,
  options,
  busy,
  onShow,
  onUndo,
  onEmptied,
}: {
  readonly runs: readonly BulkRun[] | null;
  readonly options: BrowseOptions;
  readonly busy: boolean;
  readonly onShow: (run: BulkRun) => void;
  readonly onUndo: (run: BulkRun) => void;
  /** The trash was emptied: the list reads its runs again. */
  readonly onEmptied: () => void;
}): ReactElement => {
  const admin = ADMINS.has(useSession()?.role ?? '');
  const [emptied, setEmptied] = useState<Emptied | null>(null);
  const trashText = useTranslator(bulkDelete);
  const note =
    emptied === null ? null : (
      <p role="status">
        {emptied.items === 0
          ? trashText.t('bulkDelete.empty.nothing')
          : emptied.items === 1
            ? trashText.t('bulkDelete.empty.doneOne')
            : trashText.t('bulkDelete.empty.done', { count: trashText.number(emptied.items) })}
      </p>
    );
  const translator = useTranslator(bulk);
  const { t, number, locale } = translator;
  const text = useTermLabels();
  if (runs === null) {
    return <p role="status">{t('bulk.recent.loading')}</p>;
  }
  if (runs.length === 0) {
    return <p>{t('bulk.recent.none')}</p>;
  }
  const trash =
    admin && runs.some(inTrash) ? (
      <EmptyTrash
        onEmptied={(answer) => {
          setEmptied(answer);
          onEmptied();
        }}
      />
    ) : null;
  return (
    <>
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
            <td>{run.kind === 'delete' ? t('bulk.word.delete') : editWords({ t, options, text }, run.change)}</td>
            <td>{run.purged === null ? t(`bulk.status.${run.status}`) : t('bulk.status.purged')}</td>
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
              ) : undoable(run) ? (
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
    {note}
    {trash}
    </>
  );
};
