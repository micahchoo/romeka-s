/**
 * The Items screen's saved list line. On a list the user saved it reads "Saved list: <name>"
 * with Rename and Delete. Opened from a saved list and then changed, it says "Changed since
 * saved" and offers to update that list or save a new one. On any other narrowed or ordered list
 * it offers "Save this list", with a name made from the filters. The nav shows each saved name
 * under Items. Its own chunk, loaded with the Items screen.
 *
 * ```text
 * idle -(Save this list | Save as a new list | Rename)-> naming -(Save)-> writing -> idle
 * writing -> naming (with the problem or the error)
 * idle -(Update)-> updating -> updated | updating (with the problem or the error)
 * idle -(Delete)-> deleting -> deleted
 * ```
 *
 * The screen keys it by the list's query, so a filter change starts it again at idle.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useId, useState } from 'react';
import { ErrorNotice } from '../../app/errors';
import { useSession } from '../../app/auth';
import { openSavedList, savedListHref } from '../../app/open-saved-list';
import type { OpenList } from '../../app/open-saved-list';
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
  | {
      readonly step: 'updating';
      readonly busy: boolean;
      readonly problem: ChangeProblem | null;
      readonly error: Error | null;
    }
  | { readonly step: 'updated'; readonly name: string }
  | { readonly step: 'deleting'; readonly name: string }
  | { readonly step: 'deleted'; readonly name: string };

/** Why a change did not apply, in words, or the error the server answered. */
const Problem = ({
  problem,
  error,
  name,
}: {
  readonly problem: ChangeProblem | null;
  readonly error: Error | null;
  readonly name: string;
}): ReactElement | null => {
  const { t, number } = useTranslator(finding);
  if (error !== null) {
    return <ErrorNotice error={error} />;
  }
  if (problem === null) {
    return null;
  }
  const words: Readonly<Record<ChangeProblem, string>> = {
    empty: t('savedLists.nameEmpty'),
    taken: t('savedLists.nameTaken', { name }),
    tooMany: t('savedLists.tooMany', { count: number(MOST_SAVED_LISTS) }),
    gone: t('savedLists.gone'),
  };
  return (
    <p className="saved-list-problem" role="alert">
      {words[problem]}
    </p>
  );
};

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
  const { t } = useTranslator(finding);
  const [draft, setDraft] = useState(initial);
  const field = useId();
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
        onFocus={(event) => {
          event.target.select();
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
      <Problem problem={problem} error={error} name={draft.trim()} />
    </form>
  );
};


/** The open saved list: its name, whether it changed since saved, and what the reader can do. */
const OpenListLine = ({
  open,
  step,
  savable,
  onUpdate,
  onSaveNew,
  onRename,
  onDelete,
}: {
  readonly open: OpenList;
  readonly step: Step;
  readonly savable: boolean;
  readonly onUpdate: () => void;
  readonly onSaveNew: () => void;
  readonly onRename: () => void;
  readonly onDelete: () => void;
}): ReactElement => {
  const { t } = useTranslator(finding);
  const { name } = open.list;
  const updating = step.step === 'updating' && step.busy;
  const busy = updating || step.step === 'deleting';
  return (
    <div className="saved-list">
      <p className="saved-list-name">{t('savedLists.current', { name })}</p>
      {open.changed ? (
        <>
          <p className="saved-list-changed">{t('savedLists.changed')}</p>
          {savable ? (
            <>
              <button type="button" disabled={busy} onClick={onUpdate}>
                {updating ? t('savedLists.saving') : t('savedLists.update', { name })}
              </button>
              <button type="button" disabled={busy} onClick={onSaveNew}>
                {t('savedLists.saveNew')}
              </button>
            </>
          ) : null}
          <a href={savedListHref(open.list)}>{t('savedLists.revert')}</a>
        </>
      ) : (
        <>
          <button type="button" disabled={busy} onClick={onRename}>
            {t('savedLists.rename')}
          </button>
          <button type="button" disabled={busy} onClick={onDelete}>
            {t('savedLists.delete')}
          </button>
        </>
      )}
      {step.step === 'updated' && !open.changed ? (
        <p className="saved-list-done" role="status">
          {t('savedLists.updated', { name: step.name })}
        </p>
      ) : null}
      {step.step === 'updating' ? <Problem problem={step.problem} error={step.error} name={name} /> : null}
    </div>
  );
};

const SavedListBar = ({
  state,
  suggestedName,
  onOpen,
}: {
  readonly state: ItemBrowse;
  /** The name the Save form starts with: the active filters in the chips' words. */
  readonly suggestedName: string;
  /** Mark the screen as opened from the saved list `name`, or from none. */
  readonly onOpen: (name: string | null) => void;
}): ReactElement | null => {
  const { t } = useTranslator(finding);
  const user = useSession();
  const [step, setStep] = useState<Step>({ step: 'idle' });
  if (user === null) {
    return null;
  }
  const query = savedQuery(state);
  const open = openSavedList(user.savedLists, state.list, query);
  const naming = (renaming: string | null): void => {
    setStep({ step: 'naming', renaming, busy: false, problem: null, error: null });
  };

  /** Save or rename, then mark the screen as opened from the list now called `opened`. */
  const run = (change: ListChange, renaming: string | null, opened: string): void => {
    const known = problemOf(user.savedLists, change);
    if (known !== null) {
      setStep({ step: 'naming', renaming, busy: false, problem: known, error: null });
      return;
    }
    setStep({ step: 'naming', renaming, busy: true, problem: null, error: null });
    changeSavedLists(change).then(
      (result) => {
        if (!result.ok) {
          setStep({ step: 'naming', renaming, busy: false, problem: result.problem, error: null });
          return;
        }
        setStep({ step: 'idle' });
        onOpen(opened);
      },
      (error: unknown) => {
        setStep({ step: 'naming', renaming, busy: false, problem: null, error: toError(error) });
      },
    );
  };

  const update = (name: string): void => {
    setStep({ step: 'updating', busy: true, problem: null, error: null });
    changeSavedLists({ kind: 'update', name, query }).then(
      (result) => {
        setStep(result.ok ? { step: 'updated', name } : { step: 'updating', busy: false, problem: result.problem, error: null });
      },
      (error: unknown) => {
        setStep({ step: 'updating', busy: false, problem: null, error: toError(error) });
      },
    );
  };

  const remove = (name: string): void => {
    setStep({ step: 'deleting', name });
    changeSavedLists({ kind: 'delete', name }).then(
      () => {
        setStep({ step: 'deleted', name });
        onOpen(null);
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
          initial={renaming ?? suggestedName.slice(0, LONGEST_NAME)}
          busy={step.busy}
          problem={step.problem}
          error={step.error}
          onSave={(name) => {
            const change: ListChange =
              renaming === null ? { kind: 'save', list: { name, query } } : { kind: 'rename', from: renaming, to: name };
            run(change, renaming, name);
          }}
          onCancel={() => {
            setStep({ step: 'idle' });
          }}
        />
      </div>
    );
  }
  if (open !== null) {
    return (
      <OpenListLine
        open={open}
        step={step}
        savable={isSavable(state)}
        onUpdate={() => {
          update(open.list.name);
        }}
        onSaveNew={() => {
          naming(null);
        }}
        onRename={() => {
          naming(open.list.name);
        }}
        onDelete={() => {
          remove(open.list.name);
        }}
      />
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
            naming(null);
          }}
        >
          {t('savedLists.save')}
        </button>
      ) : null}
    </div>
  );
};

export default SavedListBar;
