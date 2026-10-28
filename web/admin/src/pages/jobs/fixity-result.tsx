/**
 * A check's result on its job page: "4,230 files checked on Oct 28. 2 differ. 1 missing.", each
 * part a link to the Media list of those files. The server counts the media the reader may see,
 * through the same read as that list, so each number is the list's total.
 */
import type { ReactElement } from 'react';
import { ApiError } from '../../api/client';
import { fixityApi, fixityHref, problemsOf } from '../../api/fixity';
import type { FixityCheck, FixityShown } from '../../api/fixity';
import type { Id, Job } from '../../api/types';
import { formatDateTime } from '../../app/format';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { fixity } from '../../i18n/namespaces/fixity';

type CountKey = 'differs' | 'missing' | 'unreadable';

const COUNT_TEXT = {
  differs: ['fixity.result.differsOne', 'fixity.result.differsMany'],
  missing: ['fixity.result.missingOne', 'fixity.result.missingMany'],
  unreadable: ['fixity.result.unreadableOne', 'fixity.result.unreadableMany'],
} as const satisfies Record<CountKey, readonly [string, string]>;

const COUNTS: readonly CountKey[] = ['differs', 'missing', 'unreadable'];

const ResultLine = ({ check }: { readonly check: FixityCheck }): ReactElement => {
  const { t, number, locale } = useTranslator(fixity);
  const date = formatDateTime(check.ended ?? check.started, locale);
  const link = (shown: FixityShown, text: string): ReactElement => (
    <a href={fixityHref(check.job, shown)}>{text}</a>
  );
  return (
    <p className="fixity-result">
      {link(
        'checked',
        check.checked === 1
          ? t('fixity.result.checkedOne', { date })
          : t('fixity.result.checkedMany', { count: number(check.checked), date }),
      )}
      {COUNTS.filter((key) => check[key] > 0).map((key) => {
        const [one, many] = COUNT_TEXT[key];
        return (
          <span key={key}>
            {' '}
            {link(key, check[key] === 1 ? t(one) : t(many, { count: number(check[key]) }))}
          </span>
        );
      })}
      {problemsOf(check) === 0 ? <> {t('fixity.result.allMatch')}</> : null}
    </p>
  );
};

/** The check's counts, or `null` for a check that ended before it wrote a result (404). */
const resultOf = async (job: Id<'JobId'>): Promise<FixityCheck | null> => {
  try {
    return await fixityApi.check(job);
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) {
      return null;
    }
    throw error;
  }
};

/** The result of the check `job`, once it has ended. */
export const FixityResult = ({ job }: { readonly job: Job }): ReactElement => {
  const { t } = useTranslator(fixity);
  const ended = job.ended !== null;
  const state = useAsync(() => (ended ? resultOf(job.id) : Promise.resolve(null)), [job.id, ended]);
  let body: ReactElement;
  if (!ended) {
    body = <p>{t('fixity.result.waiting')}</p>;
  } else if (state.status === 'loading') {
    body = <p role="status">{t('fixity.result.loading')}</p>;
  } else if (state.status === 'error') {
    body = <p role="alert">{t('fixity.result.failed', { message: state.error.message })}</p>;
  } else if (state.data === null) {
    body = <p>{t('fixity.result.none')}</p>;
  } else {
    body = (
      <>
        <ResultLine check={state.data} />
        {state.data.status === 'stopped' ? <p>{t('fixity.result.stopped')}</p> : null}
      </>
    );
  }
  return (
    <section aria-labelledby="fixity-result-title">
      <h3 id="fixity-result-title">{t('fixity.result.title')}</h3>
      {body}
    </section>
  );
};
