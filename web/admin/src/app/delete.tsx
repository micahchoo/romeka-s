/**
 * The shared delete control for a show page or an edit form.
 *
 * A delete is the one write the API cannot undo, so the control is a small
 * state machine instead of a button. The first click only opens a confirmation
 * that names the row; the request runs on the confirm button. A refused or
 * missing row lands in the error phase and the control returns to the closed
 * state, so the message stays on screen without a request on the next click.
 */
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { useTranslator } from '../i18n';
import { ErrorNotice } from './errors';
import { toError } from './useAsync';

export interface DeleteControlProps {
  /** What the first button says, for example "Delete this item". */
  readonly label: string;
  /** The row's display name. The confirmation and its button carry it. */
  readonly rowName: string;
  /** Runs on confirm. The caller deletes the row and leaves the page. */
  readonly onConfirm: () => Promise<void>;
}

type Phase =
  | { readonly status: 'idle' }
  | { readonly status: 'confirming' }
  | { readonly status: 'deleting' }
  | { readonly status: 'error'; readonly error: Error };

export const DeleteControl = ({ label, rowName, onConfirm }: DeleteControlProps): ReactElement => {
  const { t } = useTranslator();
  const [phase, setPhase] = useState<Phase>({ status: 'idle' });
  const confirmButton = useRef<HTMLButtonElement>(null);

  // The confirmation replaces the button the user pressed, so focus follows it
  // to the control that carries the action.
  useEffect(() => {
    if (phase.status === 'confirming') {
      confirmButton.current?.focus();
    }
  }, [phase.status]);

  const confirm = (): void => {
    setPhase({ status: 'deleting' });
    onConfirm().catch((error: unknown) => {
      setPhase({ status: 'error', error: toError(error) });
    });
  };

  if (phase.status === 'confirming' || phase.status === 'deleting') {
    const busy = phase.status === 'deleting';
    return (
      <section className="delete-control delete-confirm" role="group" aria-label={label}>
        <p>{t('delete.confirm', { name: rowName })}</p>
        <p>
          <button
            ref={confirmButton}
            type="button"
            className="delete-confirm-button"
            disabled={busy}
            onClick={confirm}
          >
            {busy ? t('delete.busy') : t('delete.button', { name: rowName })}
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => {
              setPhase({ status: 'idle' });
            }}
          >
            {t('action.cancel')}
          </button>
        </p>
      </section>
    );
  }

  return (
    <section className="delete-control">
      <p>
        <button
          type="button"
          onClick={() => {
            setPhase({ status: 'confirming' });
          }}
        >
          {label}
        </button>
      </p>
      {phase.status === 'error' ? <ErrorNotice error={phase.error} /> : null}
    </section>
  );
};
