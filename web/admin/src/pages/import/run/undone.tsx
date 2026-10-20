/**
 * An undo that ended, from the undo job's report: what it removed and put
 * back, what it undid anyway, what was gone already, why it stopped, and what
 * it kept. A partly undone or stopped undo may go again on what is left; to
 * add the files again after a full undo, the frame offers a copy.
 */
import type { ReactElement } from 'react';
import { JobLogLink } from './job-log-link';
import { useRunText } from './messages';
import type { Stage } from './stage';
import { UndoPanel } from './undo-panel';
import type { LiveImport } from './use-live-import';
import { useServerWords } from './use-server-words';
import { kindCountsWords } from './words';

export const Undone = ({
  stage,
  live,
}: {
  readonly stage: Extract<Stage, { kind: 'undone' }>;
  readonly live: LiveImport;
}): ReactElement => {
  const translator = useRunText();
  const { t, number } = translator;
  const server = useServerWords();
  const { report, outcome } = stage;
  const headline = { undone: t('run.undone.done'), partly: t('run.undone.partly'), failed: t('run.undone.failed') }[outcome];
  const removed = report === null ? null : kindCountsWords(translator, report.removed);
  const restored = report === null ? null : kindCountsWords(translator, report.restored);
  return (
    <section className="import-run">
      <p className="import-run-lede">
        <strong>{headline}</strong> {removed === null ? null : t('run.undone.removed', { what: removed })}{' '}
        {restored === null ? null : t('run.undone.restored', { what: restored })}
      </p>
      {report === null ? null : (
        <>
          {report.forced.length === 0 ? null : (
            <p>{report.forced.length === 1 ? t('run.undone.anywayOne') : t('run.undone.anyway', { count: number(report.forced.length) })}</p>
          )}
          {report.skipped.length === 0 ? null : (
            <p>{report.skipped.length === 1 ? t('run.undone.goneOne') : t('run.undone.gone', { count: number(report.skipped.length) })}</p>
          )}
          {report.stopped ? (
            <p>{t('run.undone.asked', { done: number(report.progress.done), total: number(report.progress.total) })}</p>
          ) : null}
          {report.error === null ? null : (
            <p>
              {t('run.undone.error', {
                done: number(report.progress.done),
                total: number(report.progress.total),
                reason: server(report.error.detail),
              })}
            </p>
          )}
          {report.kept.length === 0 ? null : (
            <p>{report.kept.length === 1 ? t('run.undone.keptOne') : t('run.undone.kept', { count: number(report.kept.length) })}</p>
          )}
        </>
      )}
      {outcome === 'undone' ? null : <UndoPanel live={live} opener={t('run.undone.rest')} open={outcome === 'partly'} />}
      <JobLogLink jobId={live.detail.jobId} />
    </section>
  );
};
