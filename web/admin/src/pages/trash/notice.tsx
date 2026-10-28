/**
 * A list screen's notice after a page moved one record to the trash: "Deleted <name>."
 * and its Undo, which is the undo of that one-record run. What the undo did comes from the run:
 * the record is back (with any links it left out, in the server's words), or it stays in the trash
 * and the run says why.
 *
 * ```text
 * trashed -> undoing -> ended
 * undoing -> refused -(undo)-> undoing
 * ```
 *
 * Its own chunk, loaded only when the slot holds a record of the screen's kind.
 */
import type { ReactElement } from 'react';
import { useEffect, useRef, useState } from 'react';
import type { BulkRun } from '../../api/bulk';
import { bulkApi, undoable } from '../../api/bulk';
import { bulkDeleteApi } from '../../api/bulk-delete';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { bulkDelete } from '../../i18n/namespaces/bulk-delete';
import { ServerWords } from '../items/server-words';
import { recordHref } from './kinds';
import type { Trashed } from './slot';
import './notice.css';

type State =
  | { readonly step: 'trashed' }
  | { readonly step: 'undoing' }
  | { readonly step: 'ended'; readonly run: BulkRun }
  | { readonly step: 'refused'; readonly error: Error };

const restored = (run: BulkRun): boolean => run.status === 'undone' && (run.counts?.restored ?? 0) > 0;

const reasons = (run: BulkRun): readonly string[] =>
  [run.error, ...(run.failures ?? []).map((failure) => failure.message)].filter(
    (message): message is string => message !== null,
  );

export interface TrashedNoticeProps {
  readonly trashed: Trashed;
  /** The record is back: the list reads its page again. */
  readonly onRestored: () => void;
}

export const TrashedNotice = ({ trashed, onRestored }: TrashedNoticeProps): ReactElement => {
  const { t } = useTranslator(bulkDelete);
  const [state, setState] = useState<State>({ step: 'trashed' });
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const undo = (): void => {
    setState({ step: 'undoing' });
    bulkApi
      .undo(trashed.run, false)
      .then(() => bulkDeleteApi.settled(trashed.run))
      .then(
        (run) => {
          if (!mounted.current) {
            return;
          }
          setState({ step: 'ended', run });
          if (restored(run)) {
            onRestored();
          }
        },
        (error: unknown) => {
          if (mounted.current) {
            setState({ step: 'refused', error: toError(error) });
          }
        },
      );
  };

  const undoButton = (
    <button type="button" disabled={state.step === 'undoing'} onClick={undo}>
      {t('bulkDelete.item.undo')}
    </button>
  );

  return (
    <div className="trashed-notice" role="status">
      {state.step === 'trashed' ? (
        <p>
          {t('bulkDelete.item.trashed', { name: trashed.name })} {undoButton}
        </p>
      ) : null}
      {state.step === 'undoing' ? <p>{t('bulkDelete.item.undoing', { name: trashed.name })}</p> : null}
      {state.step === 'refused' ? (
        <>
          <p>
            {t('bulkDelete.item.trashed', { name: trashed.name })} {undoButton}
          </p>
          <ErrorNotice error={state.error} />
        </>
      ) : null}
      {state.step === 'ended' ? (
        <>
          {restored(state.run) ? (
            <p>
              <a href={recordHref(trashed.target)}>{t('bulkDelete.item.back', { name: trashed.name })}</a>
            </p>
          ) : (
            <p>
              {t('bulkDelete.item.notBack', { name: trashed.name })} {undoable(state.run) ? undoButton : null}
            </p>
          )}
          <ServerWords messages={reasons(state.run)} />
        </>
      ) : null}
    </div>
  );
};

export default TrashedNotice;
