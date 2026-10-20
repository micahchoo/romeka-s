/**
 * The import as it stands now. While a run or an undo works, the screen reads
 * it again every `POLL_MS`, the Jobs page's rate, and hands each read to the
 * frame (`adopt`), so the status over the screen moves with it. The work goes
 * on without this page; leaving stops only the reads.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { importApi } from '../../../api/import-routes';
import type { ImportDetail } from '../../../api/import-view';
import { toError } from '../../../app/useAsync';
import type { Draft } from '../draft-context';
import { isWorking } from './stage';

export const POLL_MS = 2000;

export interface LiveImport {
  readonly detail: ImportDetail;
  /** The last read failed; the next one is tried at the usual rate. */
  readonly error: Error | null;
  /** Reads the import now, as after a start, a stop or a refusal that says it moved on. */
  readonly refresh: () => void;
}

export const useLiveImport = (draft: Draft): LiveImport => {
  const [read, setRead] = useState<ImportDetail | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [round, setRound] = useState(0);
  // Until this screen reads the import itself, the frame's read is the newest.
  const detail = read ?? draft.detail;
  const working = isWorking(detail);
  const { id } = draft;
  // The frame makes a new `adopt` on each of its renders; the reads need only the latest.
  const adopt = useRef(draft.adopt);
  adopt.current = draft.adopt;

  const load = useCallback(
    (isCurrent: () => boolean): void => {
      importApi.get(id).then(
        (next) => {
          if (isCurrent()) {
            setRead(next);
            setError(null);
            adopt.current(next);
            setRound((current) => current + 1);
          }
        },
        (failure: unknown) => {
          if (isCurrent()) {
            setError(toError(failure));
            setRound((current) => current + 1);
          }
        },
      );
    },
    [id],
  );

  useEffect(() => {
    if (!working) {
      return undefined;
    }
    let current = true;
    const timer = window.setTimeout(() => {
      load(() => current);
    }, POLL_MS);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [working, round, load]);

  const refresh = useCallback(() => {
    load(() => true);
  }, [load]);

  return { detail, error, refresh };
};
