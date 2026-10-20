/**
 * One draft's plan as the person edits it, saved a moment after each edit
 * (`PUT /api/romeka/imports/{id}` with the revision last read). Both modes
 * edit this one store, so switching mode never loses an answer.
 *
 * One save is out at a time: edits made meanwhile go in the next save, with
 * the revision the last answer carried. A 409 `stale` means someone saved
 * after this read, so the store stops saving and keeps the edits on screen
 * until the person reloads; it never overwrites the other save.
 */
import type { ImportStatus, ImportDetail } from '../../api/import-view';
import type { ImportPlan } from '../../api/import-plan';
import { conflictOf } from '../../api/import-conflict';
import type { ImportApi } from '../../api/import-routes';
import { toError } from '../../app/useAsync';

/** Long enough to gather a burst of edits into one save, short enough to finish before a person leaves. */
export const SAVE_DELAY_MS = 500;

export type SaveState =
  | { readonly status: 'saved' }
  /** An edit waits for the delay or for the save that is out. */
  | { readonly status: 'saving' }
  /** Someone saved after this read; `revision` is theirs. */
  | { readonly status: 'stale'; readonly revision: number }
  /** The import has started, so its plan cannot change. */
  | { readonly status: 'closed'; readonly importStatus: ImportStatus }
  | { readonly status: 'failed'; readonly error: Error };

export interface PlanSnapshot {
  readonly detail: ImportDetail;
  /** The plan as edited, which may be ahead of `detail.plan`. */
  readonly plan: ImportPlan;
  readonly save: SaveState;
  /** Whether an edit is taken now. */
  readonly editable: boolean;
}

type Edit = (plan: ImportPlan) => ImportPlan;

const opened = (detail: ImportDetail): SaveState =>
  detail.status === 'draft' ? { status: 'saved' } : { status: 'closed', importStatus: detail.status };

const takesEdits = (save: SaveState): boolean => save.status !== 'stale' && save.status !== 'closed';

export class PlanStore {
  private state: PlanSnapshot;
  private readonly listeners = new Set<() => void>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private out = false;

  constructor(
    private readonly api: Pick<ImportApi, 'save' | 'get'>,
    detail: ImportDetail,
  ) {
    const save = opened(detail);
    this.state = { detail, plan: detail.plan, save, editable: takesEdits(save) };
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  readonly snapshot = (): PlanSnapshot => this.state;

  /** Applies `edit` and saves it soon; ignored while the store takes no edits. */
  change(edit: Edit): void {
    if (!this.state.editable) {
      return;
    }
    this.set({ plan: edit(this.state.plan), save: { status: 'saving' } });
    this.schedule();
  }

  /** Sends the edit a failed save left. */
  retry(): void {
    if (this.state.save.status === 'failed') {
      this.set({ save: { status: 'saving' } });
      this.send();
    }
  }

  /** Reads the stored import again, dropping edits it does not hold: the way out of `stale`. */
  async reload(): Promise<void> {
    this.cancelTimer();
    const detail = await this.api.get(this.state.detail.id);
    this.set({ detail, plan: detail.plan, save: opened(detail) });
  }

  /** Stops the timer and sends an edit still waiting, as the page that edits it goes away. */
  close(): void {
    if (this.timer !== null) {
      this.cancelTimer();
      this.send();
    }
  }

  private schedule(): void {
    this.cancelTimer();
    this.timer = setTimeout(() => {
      this.timer = null;
      this.send();
    }, SAVE_DELAY_MS);
  }

  private cancelTimer(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private send(): void {
    if (this.out || !this.state.editable || this.state.plan === this.state.detail.plan) {
      return;
    }
    const sent = this.state.plan;
    this.out = true;
    this.api.save(this.state.detail.id, sent, this.state.detail.revision).then(
      (detail) => {
        this.out = false;
        // The answer holds the plan the server parsed; the edits since stay ahead of it.
        const plan = this.state.plan === sent ? detail.plan : this.state.plan;
        const ahead = plan !== detail.plan;
        this.set({ detail, plan, save: ahead ? { status: 'saving' } : { status: 'saved' } });
        if (ahead && this.timer === null) {
          this.send();
        }
      },
      (error: unknown) => {
        this.out = false;
        this.set({ save: this.refused(error) });
      },
    );
  }

  private refused(error: unknown): SaveState {
    const conflict = conflictOf(error);
    if (conflict?.code === 'stale') {
      return { status: 'stale', revision: conflict.revision };
    }
    if (conflict?.code === 'notDraft') {
      return { status: 'closed', importStatus: conflict.status };
    }
    return { status: 'failed', error: toError(error) };
  }

  private set(next: Partial<Omit<PlanSnapshot, 'editable'>>): void {
    const merged = { ...this.state, ...next };
    this.state = { ...merged, editable: merged.detail.status === 'draft' && takesEdits(merged.save) };
    for (const listener of this.listeners) {
      listener();
    }
  }
}
