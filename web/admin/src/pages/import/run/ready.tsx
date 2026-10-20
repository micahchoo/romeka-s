/**
 * Before the start: what adding writes, in the check's counts, then the
 * button. Simple says it in sentences; Advanced shows the counts by record
 * kind and what each does.
 */
import type { ReactElement } from 'react';
import type { Check } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { counterpart, placeHref } from '../screens';
import { useRunText } from './messages';
import type { RunTranslator } from './messages';
import { StartControl, useStart } from './start';
import type { LiveImport } from './use-live-import';
import { checkWords, countWords } from './words';

const doesWords = (does: Check['counts'][number]['does'], t: RunTranslator['t']): ReactElement | string =>
  does === 'leave' ? t('advanced.runs.leave') : <code>{does === 'create' ? does : does.act}</code>;

const AdvancedCounts = ({ check }: { readonly check: Check }): ReactElement => {
  const { t, number } = useRunText();
  return (
    <table className="import-run-counts">
      <caption>{t('advanced.runs.counts')}</caption>
      <thead>
        <tr>
          <th scope="col">{t('advanced.runs.kind')}</th>
          <th scope="col">{t('advanced.runs.does')}</th>
          <th scope="col" className="cell-num">
            {t('advanced.runs.count')}
          </th>
        </tr>
      </thead>
      <tbody>
        {check.counts.map((count) => (
          <tr key={`${count.kind}:${typeof count.does === 'string' ? count.does : count.does.act}`}>
            <td>
              <code>{count.kind}</code>
            </td>
            <td>{doesWords(count.does, t)}</td>
            <td className="cell-num">{number(count.count)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
};

const SimpleCounts = ({ check }: { readonly check: Check }): ReactElement => {
  const draft = useDraft();
  const translator = useRunText();
  const { t } = translator;
  const words = checkWords(translator, check.counts);
  return (
    <>
      {words.adds === null ? null : <p className="import-run-lede">{t('run.ready.goes', { what: words.adds })}</p>}
      {words.changes === null ? null : <p>{t('run.ready.changes', { what: words.changes })}</p>}
      {words.leaves === null ? null : <p>{t('run.ready.left', { what: words.leaves })}</p>}
      {check.action === 'create' ? null : (
        <p className="import-note">
          {t('run.ready.chosen')} <a href={placeHref(draft.id, counterpart(draft.place))}>{t('run.ready.chosenLink')}</a>
        </p>
      )}
    </>
  );
};

export const Ready = ({ live }: { readonly live: LiveImport }): ReactElement => {
  const draft = useDraft();
  const translator = useRunText();
  const { t, locale } = translator;
  const start = useStart(live);
  const advanced = draft.place.mode === 'advanced';
  if (live.detail.plan.sources.length === 0) {
    return (
      <div className="import-empty">
        <p>
          <strong>{t('run.empty.title')}</strong>
        </p>
        <p>{t('run.empty.note')}</p>
        <p>
          <a href={placeHref(draft.id, advanced ? { mode: 'advanced', screen: 'files' } : { mode: 'simple', screen: 'files' })}>
            {t('run.empty.link')}
          </a>
        </p>
      </div>
    );
  }
  const { counted } = start;
  if (counted === null) {
    return <p role="status">{t('run.ready.saving')}</p>;
  }
  if (counted.status === 'loading') {
    return (
      <p role="status" className="import-slow">
        {t('run.ready.counting')}
      </p>
    );
  }
  if (counted.status === 'error') {
    return <ErrorNotice error={counted.error} />;
  }
  const { check, at } = counted.data;
  const words = checkWords(translator, check.counts);
  const label =
    check.deletes > 0
      ? t('run.ready.delete')
      : advanced
        ? t('advanced.runs.go')
        : t('run.ready.add', { what: countWords(translator, 'items', words.items) });
  return (
    <section className="import-run">
      {advanced ? <AdvancedCounts check={check} /> : <SimpleCounts check={check} />}
      {check.blocking.length > 0 ? (
        <p role="alert" className="import-run-stop">
          {t('run.ready.must', { what: countWords(translator, 'problems', check.blocking.length) })}{' '}
          <a href={placeHref(draft.id, advanced ? { mode: 'advanced', screen: 'problems' } : { mode: 'simple', screen: 'check' })}>
            {t('run.ready.look')}
          </a>
        </p>
      ) : null}
      {!check.canRun && check.blocking.length === 0 ? <p className="import-run-stop">{t('run.ready.nothing')}</p> : null}
      <p className="muted">
        {t('run.ready.counted', { time: new Intl.DateTimeFormat(locale, { timeStyle: 'short' }).format(at) })}{' '}
        <button type="button" className="link-button" onClick={start.recount}>
          {t('run.ready.recount')}
        </button>
      </p>
      <StartControl start={start} label={label} note={check.deletes > 0 ? null : t('run.ready.later')} />
    </section>
  );
};
