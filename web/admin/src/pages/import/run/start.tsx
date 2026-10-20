/**
 * Starting a run, first or again: the check counts what the stored answers
 * write (`preview` with `check`, which reads the frozen answers of a run that
 * started), and the start sends back the digest those counts came with. So a
 * person adds exactly what they read; a change since answers 409 `changed`,
 * and the screen counts again (`docs/import-v2.md`, "Digest").
 *
 * The first start and a retry are one code path: only the button's words
 * differ.
 */
import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { conflictOf } from '../../../api/import-conflict';
import { importApi } from '../../../api/import-routes';
import type { Check } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import type { AsyncState } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { useRunText } from './messages';
import { RefusalNotice, refusalOf } from './refusal';
import type { Refusal } from './refusal';
import type { LiveImport } from './use-live-import';

export type Counted = AsyncState<{ readonly check: Check; readonly at: Date }>;

type Starting =
  | { readonly status: 'idle' }
  | { readonly status: 'starting' }
  | { readonly status: 'refused'; readonly refusal: Refusal }
  | { readonly status: 'failed'; readonly error: Error };

export interface Start {
  /** `null` while an answer waits to be saved: counts made now would not be the ones the start sends. */
  readonly counted: Counted | null;
  readonly recount: () => void;
  readonly starting: Starting;
  readonly press: (confirm: number | undefined) => void;
}

export const useStart = (live: LiveImport): Start => {
  const draft = useDraft();
  const translator = useRunText();
  const saving = draft.save.status === 'saving';
  const [counted, setCounted] = useState<Counted>({ status: 'loading' });
  const [round, setRound] = useState(0);
  const [starting, setStarting] = useState<Starting>({ status: 'idle' });
  const { revision } = live.detail;

  useEffect(() => {
    if (saving) {
      return undefined;
    }
    let current = true;
    setCounted({ status: 'loading' });
    importApi.preview(draft.id, ['check']).then(
      (preview) => {
        if (current) {
          setCounted({ status: 'ready', data: { check: preview.check, at: new Date() } });
        }
      },
      (error: unknown) => {
        if (current) {
          setCounted({ status: 'error', error: toError(error) });
        }
      },
    );
    return () => {
      current = false;
    };
  }, [draft.id, revision, saving, round]);

  const recount = (): void => {
    setRound((current) => current + 1);
  };

  const press = (confirm: number | undefined): void => {
    if (counted.status !== 'ready' || saving) {
      return;
    }
    setStarting({ status: 'starting' });
    const { digest } = counted.data.check;
    importApi.run(draft.id, confirm === undefined ? { digest } : { digest, confirm }).then(
      () => {
        setStarting({ status: 'idle' });
        live.refresh();
      },
      (error: unknown) => {
        const conflict = conflictOf(error);
        const refusal = conflict === null ? null : refusalOf(conflict, draft.id, translator);
        if (refusal === null) {
          setStarting({ status: 'failed', error: toError(error) });
          return;
        }
        setStarting({ status: 'refused', refusal });
        if (refusal.next.kind === 'recount') {
          recount();
        }
        if (refusal.next.kind === 'refresh') {
          live.refresh();
        }
      },
    );
  };

  return { counted: saving ? null : counted, recount, starting, press };
};

/**
 * The button that starts the run, with what it needs: the count typed for a
 * delete, the state of the press, and a refusal with its next step.
 */
export const StartControl = ({
  start,
  label,
  note,
}: {
  readonly start: Start;
  readonly label: string;
  /** What follows the start, under the button. */
  readonly note: string | null;
}): ReactElement | null => {
  const draft = useDraft();
  const { t, number } = useRunText();
  const [typed, setTyped] = useState('');
  const { counted, starting } = start;
  const check = counted?.status === 'ready' ? counted.data.check : null;
  const refused = starting.status === 'refused' ? starting.refusal : null;
  const deletes = refused?.next.kind === 'confirm' ? refused.next.deletes : (check?.deletes ?? 0);
  const confirmed = deletes === 0 || typed.trim() === String(deletes);
  const ready = check !== null && check.canRun && confirmed && starting.status !== 'starting';
  return (
    <div className="import-run-start">
      {deletes > 0 ? (
        <div className="import-run-confirm">
          <p>{t('run.confirm.note', { count: number(deletes) })}</p>
          <label>
            {t('run.confirm.label', { count: number(deletes) })}{' '}
            <input
              type="text"
              inputMode="numeric"
              autoComplete="off"
              value={typed}
              onChange={(event) => {
                setTyped(event.target.value);
              }}
            />
          </label>
        </div>
      ) : null}
      <p>
        <button
          type="button"
          className={deletes > 0 ? 'import-run-danger' : 'import-primary'}
          disabled={!ready}
          aria-busy={starting.status === 'starting'}
          onClick={() => {
            start.press(deletes > 0 ? deletes : undefined);
          }}
        >
          {starting.status === 'starting' ? t('run.ready.starting') : label}
        </button>
      </p>
      {note === null ? null : <p className="muted">{note}</p>}
      {refused === null ? null : (
        <RefusalNotice
          refusal={refused}
          id={draft.id}
          mode={draft.place.mode}
          onAgain={() => {
            start.press(deletes > 0 ? deletes : undefined);
          }}
        />
      )}
      {starting.status === 'failed' ? <ErrorNotice error={starting.error} /> : null}
    </div>
  );
};
