/**
 * What the person knows about saving. The word beside the title follows every
 * save in place; a refusal is a notice above the screen with its way out:
 * "Try again" after a failure, "Load the saved answers" after someone else
 * saved (409 `stale`), and a copy once the import has started.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { useImportText } from './messages';
import type { PlanStore, SaveState } from './plan-store';
import { useCopy } from './use-copy';
import type { ImportId } from '../../api/import-plan';

/** The word beside the title; a started import has none, its status says it all. */
export const SaveWord = ({ save }: { readonly save: SaveState }): ReactElement | null => {
  const { t } = useImportText();
  switch (save.status) {
    case 'saved':
      return <span role="status">{t('shell.saved')}</span>;
    case 'saving':
      return <span role="status">{t('shell.saving')}</span>;
    case 'stale':
    case 'failed':
      return <span role="status">{t('shell.notSaved')}</span>;
    case 'closed':
      return null;
  }
};

const Copy = ({ id }: { readonly id: ImportId }): ReactElement => {
  const { t } = useImportText();
  const { state, copy } = useCopy();
  return (
    <>
      <button type="button" disabled={state.status === 'copying'} onClick={() => { copy(id); }}>
        {state.status === 'copying' ? t('home.past.copying') : t('shell.copy')}
      </button>
      {state.status === 'failed' ? <ErrorNotice error={state.error} /> : null}
    </>
  );
};

const StalePrompt = ({ store }: { readonly store: PlanStore }): ReactElement => {
  const { t } = useImportText();
  const [reload, setReload] = useState<{ readonly busy: boolean; readonly error: Error | null }>({ busy: false, error: null });
  const load = (): void => {
    setReload({ busy: true, error: null });
    store.reload().catch((error: unknown) => {
      setReload({ busy: false, error: toError(error) });
    });
  };
  return (
    <div className="import-notice" role="alert">
      <p>{t('shell.stale')}</p>
      <p>
        <button type="button" className="import-primary" disabled={reload.busy} onClick={load}>
          {t('shell.staleLoad')}
        </button>{' '}
        <span className="muted">{t('shell.staleNote')}</span>
      </p>
      {reload.error === null ? null : <ErrorNotice error={reload.error} />}
    </div>
  );
};

export const SaveNotice = ({ id, save, store }: { readonly id: ImportId; readonly save: SaveState; readonly store: PlanStore }): ReactElement | null => {
  const { t } = useImportText();
  switch (save.status) {
    case 'saved':
    case 'saving':
      return null;
    case 'failed':
      return (
        <div className="import-notice" role="alert">
          <p>{t('shell.failed')}</p>
          <ErrorNotice error={save.error} />
          <button type="button" onClick={() => { store.retry(); }}>
            {t('shell.retry')}
          </button>
        </div>
      );
    case 'stale':
      return <StalePrompt store={store} />;
    case 'closed':
      return (
        <div className="import-notice">
          <p>{t('shell.closed')}</p>
          <Copy id={id} />
        </div>
      );
  }
};
