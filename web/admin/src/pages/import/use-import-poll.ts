/**
 * The import as the server has it now: read once when the Run step opens,
 * then every second while a job works on it (running or undoing). Each answer
 * goes to `onDetail`, so the whole page sees the new status and the undo report.
 */
import { useEffect, useState } from 'react';
import { api } from '../../api/client';
import { getImport } from '../../api/imports';
import type { ImportDetail } from '../../api/imports';
import { toError } from '../../app/useAsync';
import { LIVE_STATUSES } from './run-api';

export const POLL_MS = 1000;

export interface ImportPoll {
  readonly error: Error | null;
  /** Read the import again now, for example after an undo starts. */
  readonly refresh: () => void;
}

/**
 * The next read: its number, and whether it goes now or after `POLL_MS`. An
 * answer moves on only from the read it answers, so a refresh asked while a
 * read is in flight is never lost.
 */
interface Ask {
  readonly n: number;
  readonly now: boolean;
}

export const useImportPoll = (detail: ImportDetail, onDetail: (next: ImportDetail) => void): ImportPoll => {
  const [ask, setAsk] = useState<Ask>({ n: 0, now: true });
  const [error, setError] = useState<Error | null>(null);
  const live = LIVE_STATUSES.has(detail.status);
  const { id } = detail;
  useEffect(() => {
    if (!ask.now && !live) {
      return undefined;
    }
    let cancelled = false;
    const answered = (): void => {
      setAsk((current) => (current.n === ask.n ? { n: current.n + 1, now: false } : current));
    };
    const timer = window.setTimeout(
      () => {
        getImport(api, id).then(
          (next) => {
            if (!cancelled) {
              setError(null);
              onDetail(next);
              answered();
            }
          },
          (failure: unknown) => {
            if (!cancelled) {
              setError(toError(failure));
              answered();
            }
          },
        );
      },
      ask.now ? 0 : POLL_MS,
    );
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [id, ask, live, onDetail]);
  return {
    error,
    refresh: () => {
      setAsk((current) => ({ n: current.n + 1, now: true }));
    },
  };
};
