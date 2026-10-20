/**
 * A run that ended: how many are in Omeka, why it stopped, each record that
 * failed with the server's reason, the file of what was left out, a retry
 * when the run may go again, and undo.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { RunFailure } from '../../../api/import-view';
import { ErrorNotice } from '../../../app/errors';
import { formatDateTime } from '../../../app/format';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { PagedList } from '../paged-list';
import { JobLogLink } from './job-log-link';
import { useRunText } from './messages';
import { pageOf } from './page-of';
import type { Stage } from './stage';
import { StartControl, useStart } from './start';
import { UndoPanel } from './undo-panel';
import type { LiveImport } from './use-live-import';
import { useServerWords } from './use-server-words';

const Failures = ({ failures }: { readonly failures: readonly RunFailure[] }): ReactElement => {
  const { t, number } = useRunText();
  const server = useServerWords();
  return (
    <>
      <p>{failures.length === 1 ? t('run.end.failedOne') : t('run.end.failed', { count: number(failures.length) })}</p>
      <PagedList<RunFailure>
        label={t('run.end.failures')}
        resource={t('run.end.failures')}
        load={pageOf(failures)}
        deps={[failures]}
        columns={[
          { header: t('run.end.what'), cell: (failure) => <code>{failure.record}</code> },
          { header: t('run.end.why'), cell: (failure) => server(failure.detail) },
        ]}
        rowKey={(failure) => failure.record}
        empty={null}
      />
    </>
  );
};

type Download = { readonly status: 'idle' | 'loading' } | { readonly status: 'failed'; readonly error: Error };

/** The stored left.csv as a file the browser saves: its bytes as the server wrote them. */
const LeftOut = (): ReactElement => {
  const draft = useDraft();
  const { t } = useRunText();
  const [download, setDownload] = useState<Download>({ status: 'idle' });
  const save = (): void => {
    setDownload({ status: 'loading' });
    importApi.leftCsv(draft.id).then(
      (text) => {
        const url = URL.createObjectURL(new Blob([text], { type: 'text/csv;charset=utf-8' }));
        const link = document.createElement('a');
        link.href = url;
        link.download = `import-${String(draft.id)}-left.csv`;
        link.click();
        URL.revokeObjectURL(url);
        setDownload({ status: 'idle' });
      },
      (error: unknown) => {
        setDownload({ status: 'failed', error: toError(error) });
      },
    );
  };
  return (
    <>
      <p>
        <button type="button" onClick={save} disabled={download.status === 'loading'} aria-busy={download.status === 'loading'}>
          {download.status === 'loading' ? t('run.end.leftLoading') : t('run.end.left')}
        </button>
      </p>
      {download.status === 'failed' ? <ErrorNotice error={download.error} /> : null}
    </>
  );
};

const Retry = ({ live, left }: { readonly live: LiveImport; readonly left: number }): ReactElement => {
  const { t, number } = useRunText();
  const start = useStart(live);
  return (
    <StartControl
      start={start}
      label={t('run.end.retry', { count: number(left) })}
      note={t('run.end.retryNote')}
    />
  );
};

export const Ended = ({
  stage,
  live,
}: {
  readonly stage: Extract<Stage, { kind: 'ended' }>;
  readonly live: LiveImport;
}): ReactElement => {
  const { t, number, locale } = useRunText();
  const { summary } = stage;
  const { done, total } = summary.progress;
  return (
    <section className="import-run">
      <p className="import-run-lede">
        <strong>{stage.stopped ? t('run.end.stopped') : t('run.end.done')}</strong>{' '}
        {t('run.work.done', { done: number(done), total: number(total) })}
      </p>
      <p className="muted">{t('run.work.unit')}</p>
      {summary.stopped ? <p>{t('run.end.asked')}</p> : null}
      {stage.died === null ? null : <p>{t('run.end.died', { time: formatDateTime(stage.died, locale) })}</p>}
      {summary.failures.length > 0 ? <Failures failures={summary.failures} /> : null}
      <LeftOut />
      {stage.retry ? <Retry live={live} left={Math.max(total - done, summary.failures.length)} /> : null}
      <UndoPanel live={live} opener={t('run.undo.open')} />
      <JobLogLink jobId={live.detail.jobId} />
    </section>
  );
};
