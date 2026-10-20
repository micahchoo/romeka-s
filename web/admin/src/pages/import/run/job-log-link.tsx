/** The job that writes (or wrote) the import, on the Jobs page, where its log is. */
import type { ReactElement } from 'react';
import { useRunText } from './messages';

export const JobLogLink = ({ jobId }: { readonly jobId: number | null }): ReactElement | null => {
  const { t } = useRunText();
  return jobId === null ? null : (
    <p>
      <a href={`#/jobs/${String(jobId)}`}>{t('run.work.log')}</a>
    </p>
  );
};
