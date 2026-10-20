/**
 * Undoing the last import. The window is global: the one import that holds
 * it can be undone until any next import starts. The banner names it on the
 * Import page and the Folder step; the panel says what an undo deletes and
 * asks before it starts the undo job.
 */
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { listImports, undoImport } from '../../api/imports';
import type { ImportRecord } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { useAsync, toError } from '../../app/useAsync';
import { useImportText } from './messages';
import { isConflict } from './run-api';
import { tabHref } from './tabs';

type Phase =
  | { readonly status: 'asking' }
  | { readonly status: 'starting' }
  | { readonly status: 'closed' }
  | { readonly status: 'error'; readonly error: Error };

export const UndoPanel = ({
  record,
  onCancel,
  onStarted,
}: {
  readonly record: ImportRecord;
  readonly onCancel: () => void;
  readonly onStarted: () => void;
}): ReactElement => {
  const { t, number } = useImportText();
  const [phase, setPhase] = useState<Phase>({ status: 'asking' });
  const confirmButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    confirmButton.current?.focus();
  }, []);
  const summary = record.summary;
  const items = summary?.items ?? 0;
  const confirm = (): void => {
    setPhase({ status: 'starting' });
    undoImport(api, record.id).then(onStarted, (error: unknown) => {
      setPhase(isConflict(error) ? { status: 'closed' } : { status: 'error', error: toError(error) });
    });
  };
  const lists = [
    { key: 'undo.placesIn', names: summary?.existingSets ?? [] },
    { key: 'undo.newSets', names: summary?.newSets ?? [] },
    { key: 'undo.newTypes', names: summary?.newTypes ?? [] },
    { key: 'undo.newFields', names: summary?.newFields ?? [] },
  ] as const;
  return (
    <section className="delete-control import-undo" role="group" aria-label={t('undo.heading', { folder: record.folder })}>
      <h3>{t('undo.heading', { folder: record.folder })}</h3>
      <p>{t('undo.deletes')}</p>
      <ul>
        <li>{t('undo.items', { items: number(items), files: number(summary?.files ?? 0) })}</li>
        {lists.map((list) =>
          list.names.length === 0 ? null : <li key={list.key}>{t(list.key, { names: list.names.join(', ') })}</li>,
        )}
      </ul>
      <p>{t('undo.edits')}</p>
      {phase.status === 'closed' ? <p role="alert">{t('conflict.undo')}</p> : null}
      {phase.status === 'error' ? <ErrorNotice error={phase.error} /> : null}
      <p>
        <button
          ref={confirmButton}
          type="button"
          className="delete-confirm-button"
          disabled={phase.status === 'starting' || phase.status === 'closed'}
          onClick={confirm}
        >
          {phase.status === 'starting' ? t('undo.busy') : t('undo.confirm', { count: number(items) })}
        </button>{' '}
        <button type="button" disabled={phase.status === 'starting'} onClick={onCancel}>
          {t('undo.keep')}
        </button>
      </p>
    </section>
  );
};

/** The banner while an import in `imports` can still be undone; the Import page passes the history it read. */
export const UndoNotice = ({ imports }: { readonly imports: readonly ImportRecord[] }): ReactElement | null => {
  const { t, number } = useImportText();
  const [asking, setAsking] = useState(false);
  const open = imports.find((row) => row.undoable) ?? null;
  if (open === null) {
    return null;
  }
  if (asking) {
    return (
      <UndoPanel
        record={open}
        onCancel={() => { setAsking(false); }}
        onStarted={() => { window.location.hash = tabHref(open.id, 'run'); }}
      />
    );
  }
  return (
    <p className="import-banner">
      {t('banner.text', { folder: open.folder, items: number(open.summary?.items ?? 0) })}{' '}
      <button type="button" className="link-button" onClick={() => { setAsking(true); }}>
        {t('banner.undo')}
      </button>
    </p>
  );
};

/** {@link UndoNotice} over the history it reads itself, for the Folder step. */
export const UndoBanner = (): ReactElement | null => {
  const state = useAsync(() => listImports(api), []);
  return state.status === 'ready' ? <UndoNotice imports={state.data} /> : null;
};
