/**
 * The Items screen's saved list line. On a list the user saved it reads "Saved list: <name>"
 * with Rename and Delete; on any other narrowed or ordered list it offers "Save this list". The
 * nav shows each saved name under Items. Its own chunk, loaded with the Items screen.
 *
 * ```text
 * idle -(Save this list | Rename)-> naming -(Save)-> writing -> idle
 * writing -> naming (with the problem or the error)
 * idle -(Delete)-> deleting -> deleted
 * ```
 *
 * The screen keys it by the list's query, so a filter change starts it again at idle.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useId, useState } from 'react';
import { ErrorNotice } from '../../app/errors';
import { useSession } from '../../app/auth';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { finding } from '../../i18n/namespaces/finding';
import { isSavable, savedQuery } from './browse';
import type { ItemBrowse } from './browse';
import { changeSavedLists, LONGEST_NAME, MOST_SAVED_LISTS, problemOf } from './saved-lists-model';
import type { ChangeProblem, ListChange } from './saved-lists-model';

type Step =
  | { readonly step: 'idle' }
  | {
      readonly step: 'naming';
      /** The list being renamed, or `null` for a new one. */
      readonly renaming: string | null;
      readonly busy: boolean;
      readonly problem: ChangeProblem | null;
      readonly error: Error | null;
    }
  | { readonly step: 'deleting'; readonly name: string }
  | { readonly step: 'deleted'; readonly name: string };

const NamingForm = ({
  initial,
  busy,
  problem,
  error,
  onSave,
  onCancel,
}: {
  readonly initial: string;
  readonly busy: boolean;
  readonly problem: ChangeProblem | null;
  readonly error: Error | null;
  readonly onSave: (name: string) => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const { t, number } = useTranslator(finding);
  const [draft, setDraft] = useState(initial);
  const field = useId();
  const words: Readonly<Record<ChangeProblem, string>> = {
    empty: t('savedLists.nameEmpty'),
    taken: t('savedLists.nameTaken', { name: draft.trim() }),
    tooMany: t('savedLists.tooMany', { count: number(MOST_SAVED_LISTS) }),
    gone: t('savedLists.gone'),
  };
  const submit = (event: SyntheticEvent): void => {
    event.preventDefault();
    onSave(draft.trim());
  };
  return (
    <form className="saved-list-form" onSubmit={submit}>
      <label htmlFor={field}>{t('savedLists.name')}</label>
      <input
        id={field}
        value={draft}
        maxLength={LONGEST_NAME}
        aria-describedby={`${field}-where`}
        aria-invalid={problem === null ? undefined : true}
        onChange={(event) => {
          setDraft(event.target.value);
        }}
        autoFocus
      />
      <button type="submit" disabled={busy}>
        {busy ? t('savedLists.saving') : t('savedLists.submit')}
      </button>
      <button type="button" onClick={onCancel} disabled={busy}>
        {t('savedLists.cancel')}
      </button>
      <p id={`${field}-where`} className="saved-list-where">
        {t('savedLists.where')}
      </p>
      {problem === null ? null : (
        <p className="saved-list-problem" role="alert">
          {words[problem]}
        </p>
      )}
      {error === null ? null : <ErrorNotice error={error} />}
    </form>
  );
};

const SavedListBar = ({ state }: { readonly state: ItemBrowse }): ReactElement | null => {
  const { t } = useTranslator(finding);
  const user = useSession();
  const [step, setStep] = useState<Step>({ step: 'idle' });
  if (user === null) {
    return null;
  }
  const query = savedQuery(state);
  const current = user.savedLists.find((list) => list.query === query) ?? null;

  const run = (change: ListChange, renaming: string | null): void => {
    const known = problemOf(user.savedLists, change);
    if (known !== null) {
      setStep({ step: 'naming', renaming, busy: false, problem: known, error: null });
      return;
    }
    setStep({ step: 'naming', renaming, busy: true, problem: null, error: null });
    changeSavedLists(change).then(
      (result) => {
        setStep(result.ok ? { step: 'idle' } : { step: 'naming', renaming, busy: false, problem: result.problem, error: null });
      },
      (error: unknown) => {
        setStep({ step: 'naming', renaming, busy: false, problem: null, error: toError(error) });
      },
    );
  };

  const remove = (name: string): void => {
    setStep({ step: 'deleting', name });
    changeSavedLists({ kind: 'delete', name }).then(
      () => {
        setStep({ step: 'deleted', name });
      },
      (error: unknown) => {
        setStep({ step: 'naming', renaming: name, busy: false, problem: null, error: toError(error) });
      },
    );
  };

  if (step.step === 'naming') {
    const { renaming } = step;
    return (
      <div className="saved-list">
        <NamingForm
          initial={renaming ?? ''}
          busy={step.busy}
          problem={step.problem}
          error={step.error}
          onSave={(name) => {
            run(renaming === null ? { kind: 'save', list: { name, query } } : { kind: 'rename', from: renaming, to: name }, renaming);
          }}
          onCancel={() => {
            setStep({ step: 'idle' });
          }}
        />
      </div>
    );
  }
  if (current !== null) {
    return (
      <div className="saved-list">
        <p className="saved-list-name">{t('savedLists.current', { name: current.name })}</p>
        <button
          type="button"
          disabled={step.step === 'deleting'}
          onClick={() => {
            setStep({ step: 'naming', renaming: current.name, busy: false, problem: null, error: null });
          }}
        >
          {t('savedLists.rename')}
        </button>
        <button
          type="button"
          disabled={step.step === 'deleting'}
          onClick={() => {
            remove(current.name);
          }}
        >
          {t('savedLists.delete')}
        </button>
      </div>
    );
  }
  if (step.step !== 'deleted' && !isSavable(state)) {
    return null;
  }
  return (
    <div className="saved-list">
      {step.step === 'deleted' ? (
        <p className="saved-list-name" role="status">
          {t('savedLists.deleted', { name: step.name })}
        </p>
      ) : null}
      {isSavable(state) ? (
        <button
          type="button"
          onClick={() => {
            setStep({ step: 'naming', renaming: null, busy: false, problem: null, error: null });
          }}
        >
          {t('savedLists.save')}
        </button>
      ) : null}
    </div>
  );
};

export default SavedListBar;
