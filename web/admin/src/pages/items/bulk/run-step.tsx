/**
 * A change after Apply: its progress while it runs, what it did when it ends, and the undo's
 * question. The words follow the run's status; the counts are read live from its items.
 */
import type { ReactElement } from 'react';
import { useId } from 'react';
import type { BulkCounts, BulkRun, BulkUndoPreview } from '../../../api/bulk';
import { isLive, mayRun, mayUndo } from '../../../api/bulk';
import type { ItemId } from '../../../api/types';
import { useAsync } from '../../../app/useAsync';
import { useLocale, useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { serverTranslator } from '../../../i18n/serverText';
import type { ServerTranslate } from '../../../i18n/serverText';

const NO_COUNTS: BulkCounts = { pending: 0, done: 0, unchanged: 0, failed: 0, restored: 0, kept: 0 };

const unchangedText: ServerTranslate = (message) => message;

const Progress = ({ label, value, max }: { readonly label: string; readonly value: number; readonly max: number }): ReactElement => {
  const id = useId();
  return (
    <p className="bulk-progress">
      <label htmlFor={id}>{label}</label>
      <progress id={id} value={value} max={Math.max(max, 1)} />
    </p>
  );
};

const ItemLinks = ({ items }: { readonly items: readonly ItemId[] }): ReactElement => {
  const { t } = useTranslator(bulk);
  return (
    <ul className="bulk-items">
      {items.map((item) => (
        <li key={String(item)}>
          <a href={`#/items/${String(item)}`}>{t('bulk.word.untitled', { id: String(item) })}</a>
        </li>
      ))}
    </ul>
  );
};

const Failures = ({ run }: { readonly run: BulkRun }): ReactElement | null => {
  const { t, number } = useTranslator(bulk);
  const locale = useLocale();
  const translated = useAsync(() => serverTranslator(locale), [locale]);
  const translate = translated.status === 'ready' ? translated.data : unchangedText;
  const failures = run.failures ?? [];
  if (run.failed === 0) {
    return null;
  }
  return (
    <>
      <p>{t('bulk.done.failed', { count: number(run.failed) })}</p>
      <ul className="bulk-items">
        {failures.map((failure) => (
          <li key={String(failure.item)}>
            <a href={`#/items/${String(failure.item)}`}>{t('bulk.word.untitled', { id: String(failure.item) })}</a>
            {failure.message === null ? null : <>: {translate(failure.message)}</>}
          </li>
        ))}
      </ul>
    </>
  );
};

/** What the run did, in words, once no job works on it. */
const Ended = ({ run }: { readonly run: BulkRun }): ReactElement => {
  const { t, number } = useTranslator(bulk);
  const counts = run.counts ?? NO_COUNTS;
  switch (run.status) {
    case 'undone':
      return (
        <p className="bulk-headline">
          {counts.restored === 1 ? t('bulk.done.undoneOne') : t('bulk.done.undone', { count: number(counts.restored) })}
        </p>
      );
    case 'partly_undone':
      return (
        <p className="bulk-headline">
          {t('bulk.done.partlyUndone', { count: number(counts.restored), kept: number(counts.kept) })}
        </p>
      );
    case 'stopped':
      return (
        <p className="bulk-headline">
          {t('bulk.done.stopped', { done: number(counts.done), total: number(run.total) })}
        </p>
      );
    default:
      return (
        <>
          {run.status === 'failed' && run.error !== null ? (
            <p role="alert">{t('bulk.done.error', { error: run.error })}</p>
          ) : null}
          <p className="bulk-headline">
            {counts.done === 1 ? t('bulk.done.changedOne') : t('bulk.done.changedMany', { count: number(counts.done) })}
          </p>
          {counts.unchanged === 1 ? <p>{t('bulk.done.unchangedOne')}</p> : null}
          {counts.unchanged > 1 ? <p>{t('bulk.done.unchanged', { count: number(counts.unchanged) })}</p> : null}
          <Failures run={run} />
        </>
      );
  }
};

export const WatchStep = ({
  run,
  busy,
  onStop,
  onGoOn,
  onUndo,
}: {
  readonly run: BulkRun;
  readonly busy: boolean;
  readonly onStop: () => void;
  readonly onGoOn: () => void;
  readonly onUndo: () => void;
}): ReactElement => {
  const { t, number } = useTranslator(bulk);
  const counts = run.counts ?? NO_COUNTS;
  if (isLive(run.status)) {
    const undoing = run.status === 'undoing' || run.status === 'undo_stopping';
    const [value, max] = undoing
      ? [counts.restored + counts.kept, counts.restored + counts.kept + counts.done]
      : [run.total - counts.pending, run.total];
    const stopping = run.status === 'stopping' || run.status === 'undo_stopping';
    return (
      <div className="bulk-watch">
        <Progress
          label={t(undoing ? 'bulk.run.undoProgress' : 'bulk.run.progress', { done: number(value), total: number(max) })}
          value={value}
          max={max}
        />
        <p className="bulk-note">{t('bulk.run.leave')}</p>
        <p className="bulk-actions">
          {stopping ? (
            <span role="status">{t('bulk.run.stopping')}</span>
          ) : (
            <button type="button" disabled={busy} onClick={onStop}>
              {t('bulk.run.stop')}
            </button>
          )}
        </p>
      </div>
    );
  }
  return (
    <div className="bulk-watch">
      <div role="status">
        <Ended run={run} />
      </div>
      <p className="bulk-actions">
        {mayRun(run.status) && run.status !== 'previewed' ? (
          <button type="button" disabled={busy} onClick={onGoOn}>
            {t('bulk.done.goOn')}
          </button>
        ) : null}
        {mayUndo(run.status) ? (
          <button type="button" disabled={busy} onClick={onUndo}>
            {run.status === 'partly_undone' ? t('bulk.done.undoAgain') : t('bulk.done.undo')}
          </button>
        ) : null}
      </p>
    </div>
  );
};

export const UndoStep = ({
  check,
  force,
  busy,
  onForce,
  onUndo,
  onBack,
}: {
  readonly check: BulkUndoPreview;
  readonly force: boolean;
  readonly busy: boolean;
  readonly onForce: (force: boolean) => void;
  readonly onUndo: () => void;
  readonly onBack: () => void;
}): ReactElement => {
  const { t, number } = useTranslator(bulk);
  return (
    <div className="bulk-undo">
      <p className="bulk-headline">
        {check.restore === 1 ? t('bulk.undo.restoreOne') : t('bulk.undo.restore', { count: number(check.restore) })}
      </p>
      {check.edited > 0 ? (
        <>
          <p>{check.edited === 1 ? t('bulk.undo.editedOne') : t('bulk.undo.edited', { count: number(check.edited) })}</p>
          <ItemLinks items={check.editedItems} />
          <p>
            <label>
              <input
                type="checkbox"
                checked={force}
                onChange={(event) => {
                  onForce(event.target.checked);
                }}
              />{' '}
              {t('bulk.undo.force')}
            </label>
          </p>
        </>
      ) : null}
      {check.gone === 1 ? <p>{t('bulk.undo.goneOne')}</p> : null}
      {check.gone > 1 ? <p>{t('bulk.undo.gone', { count: number(check.gone) })}</p> : null}
      <p className="bulk-actions">
        <button type="button" disabled={busy} aria-busy={busy} onClick={onUndo}>
          {t('bulk.undo.confirm')}
        </button>
        <button type="button" className="link-button" disabled={busy} onClick={onBack}>
          {t('bulk.undo.back')}
        </button>
      </p>
    </div>
  );
};
