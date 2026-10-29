/**
 * The dialog a refused save opens: someone saved the page after this editor
 * loaded it. "Show their version" loads theirs into the draft, with this
 * draft one Undo away; "Keep mine" saves this draft over theirs. Its own lazy
 * chunk, fetched only when a save is refused.
 */
import type { ReactElement } from 'react';
import { useEffect, useRef } from 'react';
import type { PageAuthor } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import { versionTime } from './versionTime';
import { pageHistory } from '../../../../i18n/namespaces/page-history';

export interface ConflictDialogProps {
  /** When the stored page was saved, `null` when the server did not say. */
  readonly modified: string | null;
  readonly author: PageAuthor | null;
  readonly busy: boolean;
  readonly onShowTheirs: () => void;
  readonly onKeepMine: () => void;
  /** Close without choosing: the draft stays as it is, unsaved. */
  readonly onClose: () => void;
}

export const ConflictDialog = ({
  modified,
  author,
  busy,
  onShowTheirs,
  onKeepMine,
  onClose,
}: ConflictDialogProps): ReactElement => {
  const translator = useTranslator(pageHistory);
  const { t } = translator;
  const first = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);
  // "since you opened this page" already says when, so a save today shows its time alone.
  const time = modified === null ? '' : versionTime(modified, translator.locale, (shown) => shown);
  const name = author?.name ?? t('history.someoneElse');

  return (
    <div className="template-dialog-backdrop">
      <div
        className="template-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="conflict-title"
        aria-describedby="conflict-text"
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.stopPropagation();
            onClose();
          }
        }}
      >
        <h2 id="conflict-title">{t('history.conflictTitle')}</h2>
        <p id="conflict-text">
          {modified === null
            ? t('history.conflictUntimed', { name })
            : t('history.conflict', { name, time })}
        </p>
        <p className="field-note">{t('history.conflictChoices')}</p>
        <div className="template-dialog-actions">
          <button type="button" ref={first} disabled={busy} onClick={onShowTheirs}>
            {t('history.showTheirs')}
          </button>
          <button type="button" disabled={busy} onClick={onKeepMine}>
            {t('history.keepMine')}
          </button>
        </div>
      </div>
    </div>
  );
};
