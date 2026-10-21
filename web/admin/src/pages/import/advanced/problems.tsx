/**
 * Advanced, Problems: one policy per flag kind for the whole import, with
 * how many flags of the kind this import has; the kinds that stop a run,
 * which take no policy; and one answer per anomaly with its count.
 */
import type { ReactElement } from 'react';
import { BLOCKING_FLAG_KINDS, FLAG_KINDS } from '../../../api/import-plan';
import type { FlagKind, FlagPolicy, ImportPlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { TableScroll } from '../../../app/table-scroll';
import { useDraft } from '../draft-context';
import { listHref } from '../lists/address';
import { usePreview } from '../lists/use-preview';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey } from './messages';
import { anomaliesOf, setAnomalies, setFlagPolicy } from './plan-edits';
import './advanced.css';

/** The contract's defaults (`docs/import-v2.md`, "Flags"); `unmatched` falls back to `action.unmatched`. */
const DEFAULT_POLICY: Readonly<Partial<Record<FlagKind, FlagPolicy>>> = {
  'empty-required': 'keep',
  'date-form': 'keep',
  'space-at-ends': 'repair',
  'encoding-suspect': 'keep',
  'name-encoding': 'keep',
  'cells-count': 'leave',
  'no-file': 'keep',
  'not-media': 'leave',
  'listed-not-media': 'leave',
  'format-mismatch': 'keep',
  exists: 'leave',
  'many-matches': 'leave',
  'many-values': 'keep',
  'link-missing': 'keep',
  'unknown-type': 'keep',
  'unknown-set': 'keep',
  'unknown-kind': 'leave',
  'duplicate-row': 'leave',
};

/**
 * The policies a kind offers when they are not keep and leave: two kinds have a repair (trim, and read again with the
 * list's encoding); a file the install refuses can only be left out, because the run's ingest refuses it again.
 */
const POLICIES: Readonly<Partial<Record<FlagKind, readonly FlagPolicy[]>>> = {
  'space-at-ends': ['keep', 'repair', 'leave'],
  'encoding-suspect': ['keep', 'repair', 'leave'],
  'listed-not-media': ['leave'],
};

const POLICY_WORDS: Readonly<Record<FlagPolicy, AdvancedMessageKey>> = {
  keep: 'adv.problems.policy.keep',
  repair: 'adv.problems.policy.repair',
  leave: 'adv.problems.policy.leave',
};

export const policyOf = (plan: ImportPlan, kind: FlagKind): FlagPolicy => {
  if (kind === 'unmatched') {
    return plan.flags.unmatched ?? (plan.action.unmatched === 'create' ? 'keep' : 'leave');
  }
  return plan.flags[kind] ?? DEFAULT_POLICY[kind] ?? 'keep';
};

const ANSWERABLE = FLAG_KINDS.filter((kind) => !BLOCKING_FLAG_KINDS.includes(kind));

export const AdvancedProblems = (): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const read = usePreview(['flags', 'identical', 'unpaired', 'entries'], {
    identical: { perPage: 1 },
    unpaired: { rows: { perPage: 1 }, files: { perPage: 1 } },
    entries: { reason: 'unmatched', perPage: 1 },
  });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('adv.problems.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { flags, identical, unpaired, entries } = read.data;
  const countOf = (kind: FlagKind): number => flags.filter((flag) => flag.kind === kind).reduce((sum, flag) => sum + flag.count, 0);
  const items = { mode: 'advanced', screen: 'items' } as const;
  const answers = anomaliesOf(draft.plan);
  const blocking = BLOCKING_FLAG_KINDS.filter((kind) => countOf(kind) > 0);
  return (
    <div className="adv-section" aria-busy={read.busy}>
      <p className="adv-plain">{t('adv.problems.plain')}</p>
      <TableScroll label={t('adv.problems.kinds.label')}>
        <table className="adv-table">
          <thead>
            <tr>
              <th scope="col">{t('adv.problems.kinds.kind')}</th>
              <th scope="col" className="num">
                {t('adv.problems.kinds.count')}
              </th>
              <th scope="col">{t('adv.problems.kinds.policy')}</th>
            </tr>
          </thead>
          <tbody>
            {ANSWERABLE.map((kind) => {
              const count = countOf(kind);
              const said = t(`problem.${kind}`);
              return (
                <tr key={kind}>
                  <th scope="row">
                    <code>{kind}</code>
                    <p className="adv-plain">{said}</p>
                  </th>
                  <td className="num">{number(count)}</td>
                  <td>
                    <select
                      aria-label={t('adv.problems.policyOf', { kind })}
                      value={policyOf(draft.plan, kind)}
                      disabled={!draft.editable}
                      onChange={(event) => {
                        const policy = event.target.value === 'repair' ? 'repair' : event.target.value === 'leave' ? 'leave' : 'keep';
                        draft.change((plan) => setFlagPolicy(plan, kind, policy));
                      }}
                    >
                      {(POLICIES[kind] ?? (['keep', 'leave'] as const)).map((policy) => (
                        <option key={policy} value={policy}>
                          {t(POLICY_WORDS[policy])}
                        </option>
                      ))}
                    </select>
                    {kind === 'unmatched' ? <p className="adv-plain">{t('adv.problems.unmatched')}</p> : null}
                    {count > 0 ? (
                      <>
                        {' '}
                        <a href={listHref(draft.id, items, { problem: kind })}>{t('adv.problems.show', { count: number(count) })}</a>
                      </>
                    ) : null}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableScroll>
      <h4>{t('adv.problems.blocking.title')}</h4>
      {blocking.length === 0 ? (
        <p className="adv-plain">{t('adv.problems.blocking.none')}</p>
      ) : (
        <ul className="adv-list" role="alert">
          {blocking.map((kind) => (
            <li key={kind}>
              <code>{kind}</code> {t(`problem.${kind}`)}: {number(countOf(kind))}. {t('adv.problems.blocking.fix')}
            </li>
          ))}
        </ul>
      )}
      <h4>{t('adv.problems.anomalies.title')}</h4>
      <TableScroll label={t('adv.problems.anomalies.label')}>
        <table className="adv-table">
          <thead>
            <tr>
              <th scope="col">{t('adv.problems.anomalies.anomaly')}</th>
              <th scope="col" className="num">
                {t('adv.problems.kinds.count')}
              </th>
              <th scope="col">{t('adv.problems.anomalies.answer')}</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <th scope="row">
                {t('adv.problems.identical.name')}
                <p className="adv-plain">{t('adv.problems.identical.plain')}</p>
              </th>
              <td className="num">{number(identical.files)}</td>
              <td>
                <select
                  aria-label={t('adv.problems.identical.name')}
                  value={answers.identical}
                  disabled={!draft.editable}
                  onChange={(event) => {
                    const identicalAnswer = event.target.value === 'keepAll' ? 'keepAll' : 'keepOne';
                    draft.change((plan) => setAnomalies(plan, { identical: identicalAnswer }));
                  }}
                >
                  <option value="keepOne">{t('adv.problems.identical.keepOne')}</option>
                  <option value="keepAll">{t('adv.problems.identical.keepAll')}</option>
                </select>
              </td>
            </tr>
            <tr>
              <th scope="row">
                {t('adv.problems.unfit.name')}
                <p className="adv-plain">{t('adv.problems.unfit.plain')}</p>
              </th>
              <td className="num">{answers.unmatchedFiles === 'leave' ? number(entries.total) : t('adv.problems.unfit.own')}</td>
              <td>
                <select
                  aria-label={t('adv.problems.unfit.name')}
                  value={answers.unmatchedFiles}
                  disabled={!draft.editable}
                  onChange={(event) => {
                    const unmatchedFiles = event.target.value === 'own' ? 'own' : 'leave';
                    draft.change((plan) => setAnomalies(plan, { unmatchedFiles }));
                  }}
                >
                  <option value="leave">{t('adv.problems.unfit.leave')}</option>
                  <option value="own">{t('adv.problems.unfit.ownAnswer')}</option>
                </select>
              </td>
            </tr>
            <tr>
              <th scope="row">
                {t('adv.problems.noRow.name')}
                <p className="adv-plain">{t('adv.problems.noRow.plain')}</p>
              </th>
              <td className="num">{number(unpaired.items)}</td>
              <td>
                <select
                  aria-label={t('adv.problems.noRow.name')}
                  value={answers.itemsWithoutRow}
                  disabled={!draft.editable}
                  onChange={(event) => {
                    const itemsWithoutRow = event.target.value === 'leave' ? 'leave' : 'keep';
                    draft.change((plan) => setAnomalies(plan, { itemsWithoutRow }));
                  }}
                >
                  <option value="keep">{t('adv.problems.noRow.keep')}</option>
                  <option value="leave">{t('adv.problems.noRow.leave')}</option>
                </select>
              </td>
            </tr>
          </tbody>
        </table>
      </TableScroll>
    </div>
  );
};

export default AdvancedProblems;
