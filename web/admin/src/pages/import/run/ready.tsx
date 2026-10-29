/**
 * Before the start: what adding writes, in the check's counts, then the
 * button. Simple says it in its own sentences (`Summary`, from `simple/add`),
 * which also hold the problems to fix and the undo note; Advanced shows the
 * counts by record kind and what each does.
 *
 * While a change is saved or the check counts again, the last counts stay on
 * screen with the start held, so a control in the summary keeps its place
 * and its focus.
 */
import { useRef } from 'react';
import type { ReactElement } from 'react';
import type { Check } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { zonedFormat } from '../../../app/format';
import { useDraft } from '../draft-context';
import { placeHref } from '../screens';
import { nodeHref } from '../picked-node';
import { useRunText } from './messages';
import type { RunTranslator } from './messages';
import { StartControl, useStart } from './start';
import type { LiveImport } from './use-live-import';
import { countWords, itemsWritten } from './words';
import type { Counted } from './start';

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

/** Simple's sentences for the check, in place of the counts by record kind. */
export type Summary = (props: { readonly check: Check }) => ReactElement;

export const Ready = ({ live, summary: SimpleSummary }: { readonly live: LiveImport; readonly summary: Summary | undefined }): ReactElement => {
  const draft = useDraft();
  const translator = useRunText();
  const { t, locale } = translator;
  const start = useStart(live);
  const advanced = draft.place.mode === 'advanced';
  const last = useRef<Extract<Counted, { status: 'ready' }>['data'] | null>(null);
  if (live.detail.plan.sources.length === 0) {
    return (
      <div className="import-empty">
        <p>
          <strong>{t('run.empty.title')}</strong>
        </p>
        <p>{t('run.empty.note')}</p>
        <p>
          <a href={advanced ? nodeHref(draft.id, { mode: 'advanced', screen: 'structure' }, 'archive') : placeHref(draft.id, { mode: 'simple', screen: 'files' })}>
            {t('run.empty.link')}
          </a>
        </p>
      </div>
    );
  }
  const { counted } = start;
  if (counted?.status === 'ready') {
    last.current = counted.data;
  }
  const shown = counted?.status === 'ready' ? counted.data : last.current;
  if (counted?.status === 'error') {
    return <ErrorNotice error={counted.error} />;
  }
  if (shown === null) {
    return (
      <p role="status" className="import-slow">
        {counted === null ? t('run.ready.saving') : t('run.ready.counting')}
      </p>
    );
  }
  const { check, at } = shown;
  const waiting = counted?.status !== 'ready';
  const label =
    check.deletes > 0
      ? t('run.ready.delete')
      : advanced
        ? t('advanced.runs.go')
        : t('run.ready.add', { what: countWords(translator, 'items', itemsWritten(check.counts)) });
  const ownWords = SimpleSummary !== undefined;
  return (
    <section className="import-run" aria-busy={waiting}>
      {ownWords ? <SimpleSummary check={check} /> : <AdvancedCounts check={check} />}
      {check.blocking.length > 0 && !ownWords ? (
        <p role="alert" className="import-run-stop">
          {t('run.ready.must', { what: countWords(translator, 'problems', check.blocking.length) })}{' '}
          <a href={placeHref(draft.id, advanced ? { mode: 'advanced', screen: 'problems' } : { mode: 'simple', screen: 'check' })}>
            {t('run.ready.look')}
          </a>
        </p>
      ) : null}
      {!check.canRun && check.blocking.length === 0 ? <p className="import-run-stop">{t('run.ready.nothing')}</p> : null}
      {waiting ? (
        <p role="status" className="import-slow">
          {counted === null ? t('run.ready.saving') : t('run.ready.counting')}
        </p>
      ) : (
        <p className="muted">
          {t('run.ready.counted', { time: zonedFormat(locale, 'time').format(at) })}{' '}
          <button type="button" className="link-button" onClick={start.recount}>
            {t('run.ready.recount')}
          </button>
        </p>
      )}
      <StartControl start={start} label={label} note={check.deletes > 0 || ownWords ? null : t('run.ready.later')} />
    </section>
  );
};
