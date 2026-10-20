/**
 * Files from this computer on a files screen of either mode. Uploads land in
 * the import's staging area, which the plan reads only as a staging source
 * (`docs/import-v2.md`, "Words"), so the first file that arrives adds one.
 * Each change of the files then looks at them again, once the plan that reads
 * them is saved.
 */
import { useEffect, useRef } from 'react';
import type { ReactElement } from 'react';
import type { ImportPlan } from '../../../api/import-plan';
import { useDraft } from '../draft-context';
import { UploadFiles } from './upload-files';

const hasStaging = (plan: ImportPlan): boolean => plan.sources.some((source) => source.kind === 'staging');

/** The plan with a staging source, under the first `s<n>` id no source has. */
export const withStaging = (plan: ImportPlan): ImportPlan => {
  if (hasStaging(plan)) {
    return plan;
  }
  const taken = new Set(plan.sources.map((source) => source.id));
  let n = 1;
  while (taken.has(`s${String(n)}`)) {
    n += 1;
  }
  return { ...plan, sources: [...plan.sources, { id: `s${String(n)}`, kind: 'staging' }] };
};

export const ComputerFiles = ({ lookAgain }: { readonly lookAgain: () => void }): ReactElement | null => {
  const draft = useDraft();
  const reads = hasStaging(draft.detail.plan);
  const due = useRef(false);
  useEffect(() => {
    if (reads && due.current) {
      due.current = false;
      lookAgain();
    }
  }, [reads, lookAgain]);
  if (draft.detail.status !== 'draft') {
    return null;
  }
  const changed = (): void => {
    if (reads) {
      lookAgain();
      return;
    }
    due.current = true;
    draft.change(withStaging);
  };
  return <UploadFiles importId={draft.id} onFilesChanged={changed} />;
};
