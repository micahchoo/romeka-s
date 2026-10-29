/**
 * "Discard": remove an import that added nothing, its plan and its uploaded
 * files (`DELETE /api/romeka/imports/{id}`). The server takes only a draft, in
 * the one statement that deletes it, so an import that has added anything is
 * refused there too; Undo is that import's way back. The past imports and the
 * frame of a draft both offer it.
 */
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import type { ImportId } from '../../api/import-plan';
import { importApi } from '../../api/import-routes';
import { ErrorNotice } from '../../app/errors';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { useImportText } from './messages';

type Phase =
  | { readonly status: 'idle' }
  | { readonly status: 'confirming' }
  | { readonly status: 'discarding' }
  | { readonly status: 'refused'; readonly error: Error };

export const DiscardImport = ({
  id,
  onDiscarded,
}: {
  readonly id: ImportId;
  readonly onDiscarded: () => void;
}): ReactElement => {
  const { t } = useImportText();
  const shared = useTranslator(common).t;
  const [phase, setPhase] = useState<Phase>({ status: 'idle' });
  const go = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (phase.status === 'confirming') {
      go.current?.focus();
    }
  }, [phase.status]);

  if (phase.status === 'confirming' || phase.status === 'discarding') {
    const busy = phase.status === 'discarding';
    return (
      <div className="import-discard" role="group" aria-label={t('discard.open')}>
        <p>{t('discard.note')}</p>
        <p className="import-row-actions">
          <button
            ref={go}
            type="button"
            disabled={busy}
            onClick={() => {
              setPhase({ status: 'discarding' });
              importApi.remove(id).then(onDiscarded, (error: unknown) => {
                setPhase({ status: 'refused', error: toError(error) });
              });
            }}
          >
            {busy ? t('discard.busy') : t('discard.go')}
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => {
              setPhase({ status: 'idle' });
            }}
          >
            {shared('action.cancel')}
          </button>
        </p>
      </div>
    );
  }
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setPhase({ status: 'confirming' });
        }}
      >
        {t('discard.open')}
      </button>
      {phase.status === 'refused' ? <ErrorNotice error={phase.error} /> : null}
    </>
  );
};
