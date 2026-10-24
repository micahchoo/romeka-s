/**
 * The bulk change panel as one state machine: each step holds exactly what it shows, and the
 * reducer is the only way between steps. An answer that arrives for a step the panel has left
 * (a poll after Close, a preview after Back) matches no transition and changes nothing.
 *
 * ```text
 * closed -> edit -> previewing -> preview -> starting -> watch
 *                       preview -(expired)-> preview (Preview again)
 * closed -> recent -> undoCheck -> undoAsk -> undoStarting -> watch
 *            recent -> watch                     (Show a run's progress)
 *                      undoAsk -> watch          (Back)
 *            watch -> undoCheck
 *            watch -> resuming -> watch          (a stopped or failed run goes on)
 * ```
 *
 * The effects (requests, the 2 s poll) live in `panel.tsx`; this file is pure.
 */
import type { BulkEdit, BulkPreview, BulkRefusal, BulkRun, BulkUndoPreview } from '../../../api/bulk';
import { NO_EDIT, isLive } from '../../../api/bulk';

/** Which items the change reaches: the ticked ones, or every item the list matches. */
export type Scope = 'ticked' | 'matching';

export interface Draft {
  readonly scope: Scope;
  readonly edit: BulkEdit;
}

export type Panel =
  | { readonly step: 'closed' }
  | { readonly step: 'recent'; readonly runs: readonly BulkRun[] | null; readonly failure: BulkRefusal | null }
  | { readonly step: 'edit'; readonly draft: Draft; readonly failure: BulkRefusal | null }
  | { readonly step: 'previewing'; readonly draft: Draft }
  | {
      readonly step: 'preview';
      readonly draft: Draft;
      readonly preview: BulkPreview;
      readonly failure: BulkRefusal | null;
    }
  | { readonly step: 'starting'; readonly draft: Draft; readonly preview: BulkPreview }
  | { readonly step: 'watch'; readonly run: BulkRun; readonly failure: BulkRefusal | null }
  | { readonly step: 'resuming'; readonly run: BulkRun }
  | { readonly step: 'undoCheck'; readonly run: BulkRun }
  | {
      readonly step: 'undoAsk';
      readonly run: BulkRun;
      readonly check: BulkUndoPreview;
      readonly force: boolean;
      readonly failure: BulkRefusal | null;
    }
  | { readonly step: 'undoStarting'; readonly run: BulkRun; readonly check: BulkUndoPreview; readonly force: boolean };

export type Step = Panel['step'];

export type Action =
  | { readonly type: 'openEdit'; readonly scope: Scope }
  | { readonly type: 'openRecent' }
  | { readonly type: 'close' }
  | { readonly type: 'recentLoaded'; readonly runs: readonly BulkRun[] }
  | { readonly type: 'draft'; readonly draft: Draft }
  | { readonly type: 'previewSent' }
  | { readonly type: 'previewed'; readonly preview: BulkPreview }
  | { readonly type: 'back' }
  | { readonly type: 'show'; readonly run: BulkRun }
  | { readonly type: 'runSent' }
  | { readonly type: 'resumeSent' }
  | { readonly type: 'run'; readonly run: BulkRun }
  | { readonly type: 'undoCheck'; readonly run: BulkRun }
  | { readonly type: 'undoChecked'; readonly check: BulkUndoPreview }
  | { readonly type: 'force'; readonly force: boolean }
  | { readonly type: 'undoSent' }
  | { readonly type: 'refused'; readonly refusal: BulkRefusal };

export const CLOSED: Panel = { step: 'closed' };

/** Whether the panel waits on a request it sent, so its buttons hold still. */
export const isBusy = (panel: Panel): boolean =>
  panel.step === 'previewing' ||
  panel.step === 'starting' ||
  panel.step === 'resuming' ||
  panel.step === 'undoCheck' ||
  panel.step === 'undoStarting' ||
  (panel.step === 'recent' && panel.runs === null && panel.failure === null);

/** Whether the panel reads its run again in 2 s. */
export const isPolling = (panel: Panel): boolean => panel.step === 'watch' && isLive(panel.run.status);

const refused = (panel: Panel, refusal: BulkRefusal): Panel => {
  switch (panel.step) {
    case 'recent':
      return { ...panel, failure: refusal };
    case 'edit':
      return { ...panel, failure: refusal };
    case 'previewing':
      return { step: 'edit', draft: panel.draft, failure: refusal };
    case 'preview':
      return { ...panel, failure: refusal };
    case 'starting':
      return { step: 'preview', draft: panel.draft, preview: panel.preview, failure: refusal };
    case 'watch':
      return { ...panel, failure: refusal };
    case 'resuming':
    case 'undoCheck':
      return { step: 'watch', run: panel.run, failure: refusal };
    case 'undoAsk':
      return { ...panel, failure: refusal };
    case 'undoStarting':
      return { step: 'undoAsk', run: panel.run, check: panel.check, force: panel.force, failure: refusal };
    case 'closed':
      return panel;
  }
};

export const reduce = (panel: Panel, action: Action): Panel => {
  switch (action.type) {
    case 'openEdit':
      return { step: 'edit', draft: { scope: action.scope, edit: NO_EDIT }, failure: null };
    case 'openRecent':
      return { step: 'recent', runs: null, failure: null };
    case 'close':
      return CLOSED;
    case 'recentLoaded':
      return panel.step === 'recent' ? { step: 'recent', runs: action.runs, failure: null } : panel;
    case 'draft':
      return panel.step === 'edit' ? { step: 'edit', draft: action.draft, failure: null } : panel;
    case 'previewSent':
      return panel.step === 'edit' || panel.step === 'preview' ? { step: 'previewing', draft: panel.draft } : panel;
    case 'previewed':
      return panel.step === 'previewing'
        ? { step: 'preview', draft: panel.draft, preview: action.preview, failure: null }
        : panel;
    case 'back':
      return back(panel);
    case 'show':
      return panel.step === 'recent' ? { step: 'watch', run: action.run, failure: null } : panel;
    case 'runSent':
      return panel.step === 'preview' ? { step: 'starting', draft: panel.draft, preview: panel.preview } : panel;
    case 'resumeSent':
      return panel.step === 'watch' ? { step: 'resuming', run: panel.run } : panel;
    case 'run':
      return runArrived(panel, action.run);
    case 'undoCheck':
      return panel.step === 'watch' || panel.step === 'recent' ? { step: 'undoCheck', run: action.run } : panel;
    case 'undoChecked':
      return panel.step === 'undoCheck'
        ? { step: 'undoAsk', run: panel.run, check: action.check, force: false, failure: null }
        : panel;
    case 'force':
      return panel.step === 'undoAsk' ? { ...panel, force: action.force } : panel;
    case 'undoSent':
      return panel.step === 'undoAsk'
        ? { step: 'undoStarting', run: panel.run, check: panel.check, force: panel.force }
        : panel;
    case 'refused':
      return refused(panel, action.refusal);
  }
};

const back = (panel: Panel): Panel => {
  switch (panel.step) {
    case 'preview':
      return { step: 'edit', draft: panel.draft, failure: null };
    case 'undoAsk':
      return { step: 'watch', run: panel.run, failure: null };
    default:
      return panel;
  }
};

/** A run read after a start, a stop or a poll: it is what the panel watches from then on. */
const runArrived = (panel: Panel, run: BulkRun): Panel => {
  switch (panel.step) {
    case 'starting':
    case 'resuming':
    case 'undoStarting':
      return { step: 'watch', run, failure: null };
    case 'watch':
      return panel.run.id === run.id ? { step: 'watch', run, failure: panel.failure } : panel;
    default:
      return panel;
  }
};
