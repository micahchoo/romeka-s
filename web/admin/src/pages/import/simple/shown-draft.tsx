/**
 * The plan as the Structure tree shows it. The tree is the proposal's, but
 * the editors read the plan by level id. A folder the stored plan does not
 * hold yet (the archive as found, or files that now make another structure)
 * is edited in the proposal's structure: its rules are read from there, and
 * its first write takes that structure, as a click does (P5 design call 1).
 * Both zooms' editors work on a folder through it.
 */
import { useMemo } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { ImportPlan, LevelId } from '../../../api/import-plan';
import { DraftContext, useDraft } from '../draft-context';
import type { Draft } from '../draft-context';
import { allSeries, withStructure } from '../series';
import { useScreen } from './structure-parts';

/** The draft the folder `level` is edited in, and whether the stored plan holds that folder already. */
export const useShownDraft = (level: LevelId): { readonly draft: Draft; readonly held: boolean } => {
  const draft = useDraft();
  const { proposal } = useScreen();
  const held = allSeries(draft.plan).some((series) => series.id === level);
  const shown = useMemo<Draft>(() => {
    const taken = (plan: ImportPlan): ImportPlan => withStructure(plan, proposal.structure);
    return held ? draft : { ...draft, plan: taken(draft.plan), change: (edit) => { draft.change((plan) => edit(taken(plan))); } };
  }, [draft, held, proposal.structure]);
  return { draft: shown, held };
};

export const InShownTree = ({ level, children }: { readonly level: LevelId; readonly children: ReactNode }): ReactElement => (
  <DraftContext.Provider value={useShownDraft(level).draft}>{children}</DraftContext.Provider>
);
