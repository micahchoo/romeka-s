/**
 * One job and its log (`GET /api/romeka/jobs/{id}/log`): read when the page
 * opens, then every 2 seconds while the job is starting or in progress. It
 * stops when the job ends or the page closes.
 */
import { useEffect, useState } from 'react';
import { api } from '../../api/client';
import { getJobLog } from '../../api/imports';
import type { Id, Job } from '../../api/types';
import { toError } from '../../app/useAsync';
import type { AsyncState } from '../../app/useAsync';

export const JOB_POLL_MS = 2000;

/** `Job::STATUS_STARTING` and `Job::STATUS_IN_PROGRESS`: a job that may still write. */
const LIVE: ReadonlySet<string> = new Set(['starting', 'in_progress']);

export type JobLog = { readonly text: string } | { readonly error: Error };

export interface JobPoll {
  readonly job: AsyncState<Job>;
  readonly log: JobLog | null;
  /** Whether the page reads the job again. */
  readonly live: boolean;
  /** Shows a job the page wrote itself, for example after an update. */
  readonly onJob: (job: Job) => void;
}

const readLog = (jobId: Id<'JobId'>): Promise<JobLog> =>
  getJobLog(api, jobId).then(
    (text) => ({ text }),
    (error: unknown) => ({ error: toError(error) }),
  );

export const useJobPoll = (jobId: Id<'JobId'>): JobPoll => {
  const [round, setRound] = useState(0);
  const [job, setJob] = useState<AsyncState<Job>>({ status: 'loading' });
  const [log, setLog] = useState<JobLog | null>(null);
  const live = job.status === 'ready' && LIVE.has(job.data.status ?? '');
  useEffect(() => {
    if (round > 0 && !live) {
      return undefined;
    }
    let cancelled = false;
    const timer = window.setTimeout(
      () => {
        Promise.all([api.getJob(jobId), readLog(jobId)]).then(
          ([next, nextLog]) => {
            if (!cancelled) {
              setJob({ status: 'ready', data: next });
              setLog(nextLog);
              setRound((current) => current + 1);
            }
          },
          (error: unknown) => {
            if (!cancelled) {
              setJob({ status: 'error', error: toError(error) });
              setRound((current) => current + 1);
            }
          },
        );
      },
      round === 0 ? 0 : JOB_POLL_MS,
    );
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [jobId, round, live]);
  return {
    job,
    log,
    live,
    onJob: (next) => {
      setJob({ status: 'ready', data: next });
    },
  };
};
