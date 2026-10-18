import type { ReactElement } from 'react';
import { jobIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { JobDetail } from './detail';
import { JobList } from './list';

/** `#/jobs` lists; `#/jobs/<id>` shows one job. There is no create route. */
export const JobsPage = ({
  param = null,
}: {
  readonly param?: string | null;
} = {}): ReactElement => {
  const { t } = useTranslator();
  if (param === null) {
    return <JobList />;
  }
  const jobId = jobIdFromParam(param);
  if (jobId === null) {
    return <p role="alert">{t('route.invalidJobId', { param })}</p>;
  }
  return <JobDetail key={String(jobId)} jobId={jobId} />;
};
