import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, Job } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';
import { useJobStatusLabel } from './messages';
import { common } from '../../i18n/namespaces/common';
import { admin } from '../../i18n/namespaces/admin';

const PER_PAGE = 25;

interface JobRow {
  readonly job: Job;
  readonly ownerName: string | null;
}

const load = async (page: number): Promise<ApiPage<JobRow>> => {
  const [jobs, users] = await Promise.all([
    api.listJobs({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'desc' }),
    api.listUsers({ perPage: 100 }),
  ]);
  const names = userNames(users.items);
  return mapPage(jobs, (job) => ({ job, ownerName: ownerNameOf(names, job.owner) }));
};

/** `Admin/JobController::browseAction`: ID, class, status, owner. */
export const JobList = (): ReactElement => {
  const { t, locale } = useTranslator(common, admin);
  const statusLabel = useJobStatusLabel();
  const resource = t('resource.jobs');
  return (
    <ListPage<JobRow>
      resourceLabel={resource}
      emptyMessage={t('list.empty', { resource })}
      load={load}
      rowKey={(row) => String(row.job.id)}
      columns={[
        {
          header: t('column.id'),
          cell: (row) => <a href={`#/jobs/${String(row.job.id)}`}>{row.job.id}</a>,
          numeric: true,
        },
        { header: t('column.class'), cell: (row) => row.job.jobClass },
        {
          header: t('column.status'),
          cell: (row) => statusLabel(row.job.status) ?? t('common.unknown'),
        },
        { header: t('column.owner'), cell: (row) => row.ownerName ?? t('common.noOwner') },
        {
          header: t('column.started'),
          cell: (row) => (row.job.started === null ? '' : formatDateTime(row.job.started, locale)),
        },
        {
          header: t('column.ended'),
          cell: (row) => (row.job.ended === null ? '' : formatDateTime(row.job.ended, locale)),
        },
      ]}
    />
  );
};
