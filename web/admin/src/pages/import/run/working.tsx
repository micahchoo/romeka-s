/**
 * While a run or an undo works: how far it got, the time left at the rate so
 * far, that the work goes on without this page, and Stop. Stop asks the job
 * to end after the one it holds (`POST …/stop`); the job, not this page, ends
 * it, so the page says "Stopping" until a read shows it ended.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { conflictOf } from '../../../api/import-conflict';
import { importApi } from '../../../api/import-routes';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { JobLogLink } from './job-log-link';
import { useRunText } from './messages';
import { RefusalNotice, refusalOf } from './refusal';
import type { Refusal } from './refusal';
import type { Stage } from './stage';
import type { LiveImport } from './use-live-import';
import { timeLeftWords } from './words';

type Stopping =
  | { readonly status: 'idle' }
  /** Sent, or sent and taken: the page says "Stopping" until a read shows the stop. */
  | { readonly status: 'asked' }
  | { readonly status: 'refused'; readonly refusal: Refusal }
  | { readonly status: 'failed'; readonly error: Error };

export const Working = ({
  stage,
  live,
}: {
  readonly stage: Extract<Stage, { kind: 'working' }>;
  readonly live: LiveImport;
}): ReactElement => {
  const draft = useDraft();
  const translator = useRunText();
  const { t, number } = translator;
  const [stopping, setStopping] = useState<Stopping>({ status: 'idle' });
  const run = stage.work === 'run';
  const stop = (): void => {
    setStopping({ status: 'asked' });
    importApi.stop(draft.id).then(
      () => {
        live.refresh();
      },
      (error: unknown) => {
        const conflict = conflictOf(error);
        const refusal = conflict === null ? null : refusalOf(conflict, draft.id, translator);
        setStopping(refusal === null ? { status: 'failed', error: toError(error) } : { status: 'refused', refusal });
        live.refresh();
      },
    );
  };
  const asked = stage.stopping || stopping.status === 'asked';
  return (
    <section className="import-run" aria-live="polite">
      <p className="import-run-lede">
        <strong>{run ? t('run.work.adding') : t('run.work.undoing')}</strong>
      </p>
      {stage.done === null || stage.total === null ? (
        <progress className="import-run-progress" />
      ) : (
        <>
          <progress className="import-run-progress" value={stage.done} max={Math.max(stage.total, 1)} />
          <p>
            {t('run.work.done', { done: number(stage.done), total: number(stage.total) })}{' '}
            <span className="muted">{t('run.work.unit')}</span>
          </p>
        </>
      )}
      {run ? <p>{timeLeftWords(translator, stage.secondsLeft)}</p> : null}
      {stage.resumed ? <p>{t('run.restarted')}</p> : null}
      <p className="muted">{run ? t('run.work.leave') : t('run.work.leaveUndo')}</p>
      {asked ? (
        <p role="status">{t('run.work.stopping')}</p>
      ) : (
        <div className="import-run-stop-control">
          <button type="button" onClick={stop}>
            {run ? t('run.work.stop') : t('run.work.stopUndo')}
          </button>
          <p className="muted">{run ? t('run.work.stopNote') : t('run.work.stopUndoNote')}</p>
        </div>
      )}
      {stopping.status === 'refused' ? (
        <RefusalNotice refusal={stopping.refusal} id={draft.id} mode={draft.place.mode} onAgain={stop} />
      ) : null}
      {stopping.status === 'failed' ? <ErrorNotice error={stopping.error} /> : null}
      {live.error === null ? null : <ErrorNotice error={live.error} />}
      <JobLogLink jobId={live.detail.jobId} />
    </section>
  );
};
