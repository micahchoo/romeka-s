import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Id, Job } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import { formatDateTime } from '../../app/format';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { JobUpdateForm } from './form';
import { useJobStatusLabel } from './messages';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

/**
 * `Admin/JobController::showAction` with `::deleteAction`: one job, its columns
 * and the update and delete controls. The update writes no member, and the API
 * serves no stop endpoint, so the stop confirm upstream shows has no form here.
 */
export const JobDetail = ({ jobId }: { readonly jobId: Id<'JobId'> }): ReactElement => {
  const { t, locale } = useTranslator(common, admin);
  const statusLabel = useJobStatusLabel();
  const state = useAsync(() => api.getJob(jobId), [jobId]);
  const [edited, setEdited] = useState<Job | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.job')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const job = edited ?? state.data;
  return (
    <section>
      <h2>{t('jobs.rowName', { id: job.id })}</h2>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{job.id}</dd>
        <dt>{t('column.class')}</dt>
        <dd>{job.jobClass}</dd>
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

      <h3>{t('jobs.args')}</h3>
      <pre>{JSON.stringify(job.args, null, 2)}</pre>

      <EditPanel label={t('jobs.editLabel')}>
        <JobUpdateForm job={job} onSaved={setEdited} />
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
