/**
 * What every screen body reads: the import, its plan as edited, the way to
 * change it, and where the person is. `DraftShell` provides it; both modes'
 * bodies read the one store, so an answer given in one mode is there in the
 * other.
 */
import { createContext, useContext } from 'react';
import type { ImportId, ImportPlan } from '../../api/import-plan';
import type { ImportDetail } from '../../api/import-view';
import type { PlanSnapshot } from './plan-store';
import type { Place } from './screens';

export interface Draft extends PlanSnapshot {
  readonly id: ImportId;
  readonly place: Place;
  /** Edits the plan; ignored when `editable` is false. */
  readonly change: (edit: (plan: ImportPlan) => ImportPlan) => void;
  /** Takes the import another route answered (an added list); ignored while an edit waits to be saved. */
  readonly adopt: (detail: ImportDetail) => void;
  /**
   * How many times the files were looked at again since the frame opened. A
   * look changes what the previews count but not the plan's revision, so a
   * preview reads again on either.
   */
  readonly looks: number;
  /** Says the files were looked at again. */
  readonly looked: () => void;
}

export const DraftContext = createContext<Draft | null>(null);

export const useDraft = (): Draft => {
  const draft = useContext(DraftContext);
  if (draft === null) {
    throw new Error('useDraft is read outside DraftShell');
  }
  return draft;
};
