/**
 * The Media list opened from a fixity count: which files it shows, a way back to the check, and a
 * way to the whole list. The filter lives in the hash (`?fixity=differs&fixityJob=12`), so a
 * reload or a shared link opens the same list; the server applies it (`romeka_fixity`).
 */
import type { ReactElement } from 'react';
import type { FixityShown } from '../../api/fixity';
import type { Id } from '../../api/types';
import { useTranslator } from '../../i18n';
import { fixity } from '../../i18n/namespaces/fixity';

const SHOWN_TEXT = {
  checked: 'fixity.filter.checked',
  ok: 'fixity.filter.ok',
  differs: 'fixity.filter.differs',
  missing: 'fixity.filter.missing',
  unreadable: 'fixity.filter.unreadable',
} as const satisfies Record<FixityShown, string>;

export const FixityFilterNote = ({
  shown,
  job,
  onClear,
}: {
  readonly shown: FixityShown;
  readonly job: Id<'JobId'>;
  readonly onClear: () => void;
}): ReactElement => {
  const { t } = useTranslator(fixity);
  return (
    <p className="fixity-filter" role="status">
      {t(SHOWN_TEXT[shown], { job: String(job) })}{' '}
      <a href={`#/jobs/${String(job)}`}>{t('fixity.filter.back')}</a>{' '}
      <button type="button" onClick={onClear}>
        {t('fixity.filter.clear')}
      </button>
    </p>
  );
};
