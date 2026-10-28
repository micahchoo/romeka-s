/**
 * "Check stored files" on the Jobs screen. Only the two administrator roles may write a job, so
 * only they see it; the server refuses everyone else (`omeka_api::fixity::view::start`). A click
 * while a check runs opens that check instead of starting a second one.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { fixityApi } from '../../api/fixity';
import { useSession } from '../../app/auth';
import { toError } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { fixity } from '../../i18n/namespaces/fixity';

const ADMINS: ReadonlySet<string> = new Set(['global_admin', 'site_admin']);

export const FixityStart = (): ReactElement | null => {
  const { t } = useTranslator(fixity);
  const admin = ADMINS.has(useSession()?.role ?? '');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  if (!admin) {
    return null;
  }
  const start = (): void => {
    setBusy(true);
    setFailure(null);
    fixityApi.start().then(
      (job) => {
        window.location.hash = `#/jobs/${String(job)}`;
      },
      (error: unknown) => {
        setBusy(false);
        setFailure(toError(error).message);
      },
    );
  };
  return (
    <div className="fixity-start">
      <button type="button" onClick={start} disabled={busy}>
        {busy ? t('fixity.start.busy') : t('fixity.start')}
      </button>{' '}
      <span>{t('fixity.start.note')}</span>
      {failure === null ? null : <p role="alert">{t('fixity.start.failed', { message: failure })}</p>}
    </div>
  );
};
