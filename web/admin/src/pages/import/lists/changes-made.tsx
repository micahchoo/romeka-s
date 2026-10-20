/**
 * Under each list: how many changes the person made, and the way back from
 * the last change to many, which is the one a wrong filter makes costly.
 */
import type { ReactElement } from 'react';
import { useDraft } from '../draft-context';
import { useImportText } from '../messages';

export const ChangesMade = (): ReactElement | null => {
  const { t, number } = useImportText();
  const draft = useDraft();
  const decisions = draft.plan.decisions;
  if (decisions.length === 0) {
    return null;
  }
  const many = decisions.filter((decision) => decision.kind === 'many').length;
  const last = decisions.map((decision) => decision.kind).lastIndexOf('many');
  return (
    <div className="import-note">
      <p>
        {decisions.length === 1 ? t('lists.changes.one') : t('lists.changes.count', { count: number(decisions.length) })}{' '}
        {many === 0 ? null : t('lists.changes.many', { count: number(many) })}
      </p>
      {last < 0 ? null : (
        <button
          type="button"
          disabled={!draft.editable}
          onClick={() => {
            draft.change((plan) => ({ ...plan, decisions: plan.decisions.filter((_, index) => index !== last) }));
          }}
        >
          {t('lists.changes.undoMany')}
        </button>
      )}
    </div>
  );
};
