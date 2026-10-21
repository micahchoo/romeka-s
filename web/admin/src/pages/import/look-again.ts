/**
 * "Look at the files again": one rescan of the import's sources, from the
 * button or from files that arrived. A look asked while one is out runs once
 * more when it ends, so the last arrival is always counted. A look that ends
 * tells the frame (`looked`), and the previews count again.
 *
 * A large index is rescanned by a job (the route answers its id). The look
 * then reads the job at the run screen's rate until it ends, reads the import
 * and the counts again, and only then is done. Leaving the screen stops the
 * reads, not the job.
 */
import { useEffect, useRef, useState } from 'react';
import { importApi } from '../../api/import-routes';
import type { SourceState } from '../../api/import-sections';
import { importJobs } from '../../api/import-uploads';
import { toError } from '../../app/useAsync';
import { useDraft } from './draft-context';
import { POLL_MS } from './run/use-live-import';

export type Look =
  | { readonly at: 'idle' }
  | { readonly at: 'looking' }
  /** The rescan runs as a job, which takes a while. */
  | { readonly at: 'working'; readonly jobId: number }
  | { readonly at: 'done'; readonly sources: readonly SourceState[] }
  | { readonly at: 'failed'; readonly error: Error };

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });

export const useLookAgain = (): { readonly look: Look; readonly lookAgain: () => void } => {
  const { id, looked, adopt } = useDraft();
  const [look, setLook] = useState<Look>({ at: 'idle' });
  const out = useRef(false);
  const asked = useRef(false);
  const mounted = useRef(true);
  // The frame makes a new `adopt` on each of its renders; a job's end needs only the latest.
  const adoptNow = useRef(adopt);
  adoptNow.current = adopt;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  /** Reads the job until it ends: the sources it left, or why it failed. */
  const finished = async (jobId: number): Promise<readonly SourceState[]> => {
    for (;;) {
      await wait(POLL_MS);
      if (!mounted.current) {
        return [];
      }
      const status = await importJobs.jobStatus(jobId);
      if (status === 'completed') {
        adoptNow.current(await importApi.get(id));
        return (await importApi.preview(id, [])).sources;
      }
      if (status === 'error' || status === 'stopped') {
        throw new Error(await importJobs.jobReason(jobId));
      }
    }
  };

  const lookAgain = (): void => {
    if (out.current) {
      asked.current = true;
      return;
    }
    out.current = true;
    asked.current = false;
    setLook({ at: 'looking' });
    const settle = (next: Look): void => {
      out.current = false;
      if (!mounted.current) {
        return;
      }
      setLook(next);
      looked();
      if (asked.current) {
        lookAgain();
      }
    };
    importApi
      .scan(id)
      .then((result) => {
        if (result.kind === 'scanned') {
          return result.sources;
        }
        setLook({ at: 'working', jobId: result.jobId });
        return finished(result.jobId);
      })
      .then(
        (sources) => {
          settle({ at: 'done', sources });
        },
        (error: unknown) => {
          settle({ at: 'failed', error: toError(error) });
        },
      );
  };
  return { look, lookAgain };
};
