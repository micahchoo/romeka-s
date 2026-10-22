/**
 * Where an import stands, as the run screen draws it: one stage per status
 * (`docs/import-v2.md`, "Lifecycle and the ledger"), each with only the facts
 * that stage shows. The switch is over every status, so a new one is a `tsc`
 * error here and nowhere else.
 */
import type { ImportDetail, RunSummary } from '../../../api/import-view';
import type { UndoReport } from '../../../api/import-undo';
import { canRetry } from '../words';

export type Stage =
  /** Not started: the check, then Add. */
  | { readonly kind: 'ready' }
  /** A run or an undo works now. */
  | {
      readonly kind: 'working';
      readonly work: 'run' | 'undo';
      readonly done: number | null;
      readonly total: number | null;
      readonly secondsLeft: number | null;
      readonly stopping: boolean;
      /** A restart stopped the run, and the server went on with it by itself. */
      readonly resumed: boolean;
    }
  /** A run ended: every record written, or it stopped. Undo may follow; a retry when the run may go again. */
  | {
      readonly kind: 'ended';
      readonly stopped: boolean;
      readonly summary: RunSummary;
      /** When a reader found the job dead. */
      readonly died: string | null;
      readonly retry: boolean;
    }
  /** An undo ended. Only `undone` is final; the others may undo again. */
  | { readonly kind: 'undone'; readonly outcome: 'undone' | 'partly' | 'failed'; readonly report: UndoReport | null };

const EMPTY_SUMMARY: RunSummary = { progress: { done: 0, total: 0 }, failures: [], stopped: false };

export const stageOf = (detail: ImportDetail): Stage => {
  switch (detail.status) {
    case 'draft':
      return { kind: 'ready' };
    case 'running':
      return {
        kind: 'working',
        work: 'run',
        done: detail.progress?.done ?? detail.summary?.progress.done ?? null,
        total: detail.progress?.total ?? detail.summary?.progress.total ?? null,
        secondsLeft: detail.progress?.secondsLeft ?? null,
        stopping: detail.stopRequested,
        resumed: (detail.resumed ?? 0) > 0,
      };
    case 'undoing':
      return {
        kind: 'working',
        work: 'undo',
        done: detail.undoReport?.progress.done ?? null,
        total: detail.undoReport?.progress.total ?? null,
        secondsLeft: null,
        stopping: detail.stopRequested,
        resumed: false,
      };
    case 'done':
    case 'run_failed': {
      const summary = detail.summary ?? EMPTY_SUMMARY;
      return {
        kind: 'ended',
        stopped: detail.status === 'run_failed',
        summary,
        died: detail.died,
        retry: canRetry(detail),
      };
    }
    case 'undone':
      return { kind: 'undone', outcome: 'undone', report: detail.undoReport };
    case 'partly_undone':
      return { kind: 'undone', outcome: 'partly', report: detail.undoReport };
    case 'undo_failed':
      return { kind: 'undone', outcome: 'failed', report: detail.undoReport };
  }
};

/** Whether the screen reads the import again on its own: the work goes on without this page. */
export const isWorking = (detail: ImportDetail): boolean => detail.status === 'running' || detail.status === 'undoing';
