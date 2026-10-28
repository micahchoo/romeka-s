/**
 * The dashboard's fixity line: "Files last checked Oct 28: 2 problems", linking to that check's
 * result. A problem is a file that differs, is missing or cannot be read, counted for the reader
 * as the result screen counts it. Checks are jobs, so only the roles that may read jobs ask for
 * the line; the server answers anyone else 403, and they see no line.
 */
import type { ReactElement } from 'react';
import { ApiError } from '../../api/client';
import { fixityApi, problemsOf } from '../../api/fixity';
import type { FixityLatest } from '../../api/fixity';
import { useSession } from '../../app/auth';
import { formatDateTime } from '../../app/format';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { fixity } from '../../i18n/namespaces/fixity';

/** The roles the jobs grant lets read jobs (`Api::allow`, `JobAdapter`). */
const READS_JOBS: ReadonlySet<string> = new Set(['global_admin', 'site_admin', 'editor', 'reviewer']);

/** The latest check, or `null` for a reader the server does not show checks to. */
const latestOrHidden = async (): Promise<FixityLatest | null> => {
  try {
    return await fixityApi.latest();
  } catch (error) {
    if (error instanceof ApiError && error.status === 403) {
      return null;
    }
    throw error;
  }
};

export const FixityLine = (): ReactElement | null => {
  const reads = READS_JOBS.has(useSession()?.role ?? '');
  return reads ? <LatestCheck /> : null;
};

const LatestCheck = (): ReactElement | null => {
  const { t, number, locale } = useTranslator(fixity);
  const state = useAsync(latestOrHidden, []);
  if (state.status !== 'ready' || state.data === null) {
    return null;
  }
  const { check, running } = state.data;
  if (check === null) {
    return running === null ? (
      <p className="fixity-line">
        <a href="#/jobs">{t('fixity.line.never')}</a>
      </p>
    ) : (
      <p className="fixity-line">
        <a href={`#/jobs/${String(running)}`}>{t('fixity.line.running')}</a>
      </p>
    );
  }
  const date = formatDateTime(check.ended ?? check.started, locale);
  const problems = problemsOf(check);
  let text: string;
  if (problems === 0) {
    text = t('fixity.line.noProblems', { date });
  } else if (problems === 1) {
    text = t('fixity.line.problemsOne', { date });
  } else {
    text = t('fixity.line.problemsMany', { date, count: number(problems) });
  }
  return (
    <p className="fixity-line">
      <a href={`#/jobs/${String(check.job)}`}>{text}</a>
    </p>
  );
};
