import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { Id } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { formatDateTime } from '../../app/format';
import { useTranslator } from '../../i18n';
import { JobUpdateForm } from './form';
import { importOfJob, useJobClassLabel, useJobStatusLabel } from './messages';
import './jobs.css';
import { useJobPoll } from './use-job-poll';
import type { JobLog } from './use-job-poll';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/**
 * The job's log as text (`Admin/JobController::logAction`), or why it could not be read.
 */
const LogView = ({ log }: { readonly log: JobLog | null }): ReactElement | null => {
  const { t } = useTranslator(admin);
  if (log === null) {
    return null;
  }
  if ('error' in log) {
    return <p role="alert">{t('jobs.logFailed', { message: log.error.message })}</p>;
  }
  return log.text === '' ? <p>{t('jobs.logEmpty')}</p> : <pre className="job-log">{log.text}</pre>;
};

/**
 * `Admin/JobController::showAction` with `::deleteAction`: one job, its columns,
 * its log, and the update and delete controls. While the job runs the page
 * reads it again every 2 seconds. The update writes no member, and the API
 * serves no stop endpoint, so the stop confirm upstream shows has no form here.
 */
export const JobDetail = ({ jobId }: { readonly jobId: Id<'JobId'> }): ReactElement => {
  const { t, locale } = useTranslator(common, admin);
  const statusLabel = useJobStatusLabel();
  const classLabel = useJobClassLabel();
  const poll = useJobPoll(jobId);
  const state = poll.job;

  if (state.status === 'loading') {
    return <p role="status">{t('loading.job')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const job = state.data;
  const importId = importOfJob(job.jobClass, job.args);
  return (
    <section>
      <h2>{t('jobs.rowName', { id: job.id })}</h2>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{job.id}</dd>
        <dt>{t('column.class')}</dt>
        <dd>
          {classLabel(job.jobClass)}
          {classLabel(job.jobClass) === job.jobClass ? null : (
            <>
              {' '}
              <code>{job.jobClass}</code>
            </>
          )}
        </dd>
        <dt>{t('column.status')}</dt>
        <dd>{statusLabel(job.status) ?? t('common.unknown')}</dd>
        <dt>{t('column.owner')}</dt>
        <dd>
          {job.owner === null ? (
            t('common.noOwner')
          ) : (
            <a href={`#/users/${String(job.owner.id)}`}>{`#${String(job.owner.id)}`}</a>
          )}
        </dd>
        <dt>{t('column.started')}</dt>
        <dd>{job.started === null ? t('common.never') : formatDateTime(job.started, locale)}</dd>
        <dt>{t('column.ended')}</dt>
        <dd>{job.ended === null ? t('common.never') : formatDateTime(job.ended, locale)}</dd>
      </dl>

      {importId === null ? null : (
        <p>
          <a href={`#/import/${String(importId)}`}>{t('jobs.openImport')}</a>
        </p>
      )}

      <h3>{t('jobs.args')}</h3>
      <pre>{JSON.stringify(job.args, null, 2)}</pre>

      <h3>{t('jobs.log')}</h3>
      {poll.live ? <p className="query-count" role="status">{t('jobs.refreshing')}</p> : null}
      <LogView log={poll.log} />

      <EditPanel label={t('jobs.editLabel')}>
        <JobUpdateForm job={job} onSaved={poll.onJob} />
      </EditPanel>

      <DeleteControl
        label={t('jobs.deleteLabel')}
        rowName={t('jobs.rowName', { id: job.id })}
        onConfirm={async () => {
          await api.deleteJob(job.id);
          window.location.hash = '#/jobs';
        }}
      />
    </section>
  );
};
