/**
 * Advanced, Changes: every decision of the plan in the order it applies (a
 * later one on the same thing wins), 50 to a page, each with Remove. A
 * decision that names something no longer there says so. Removing all of
 * them asks once more, because nothing brings them back.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { OrphanReason } from '../../../api/import-model';
import { Pager } from '../../../app/list';
import { TableScroll } from '../../../app/table-scroll';
import { useDraft } from '../draft-context';
import { PER_PAGE } from '../paged-list';
import { usePreview } from '../lists/use-preview';
import { decisionWords } from './decision-words';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey } from './messages';
import { removeDecision, removeDecisions } from './plan-edits';
import './advanced.css';

const ORPHAN_WORDS: Readonly<Record<OrphanReason, AdvancedMessageKey>> = {
  missingEntry: 'adv.changes.orphan.missingEntry',
  wrongTarget: 'adv.changes.orphan.wrongTarget',
  lineKeyChanged: 'adv.changes.orphan.lineKeyChanged',
  noGroup: 'adv.changes.orphan.noGroup',
  outside: 'adv.changes.orphan.outside',
};

export const AdvancedChanges = (): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const [page, setPage] = useState(1);
  const [clearing, setClearing] = useState(false);
  const read = usePreview(['orphans'], {});
  const { decisions } = draft.plan;
  if (decisions.length === 0) {
    return <p className="import-note">{t('adv.changes.none')}</p>;
  }
  const lastPage = Math.max(1, Math.ceil(decisions.length / PER_PAGE));
  const shown = Math.min(page, lastPage);
  const first = (shown - 1) * PER_PAGE;
  const orphans = read.status === 'ready' ? read.data.orphans : [];
  return (
    <div className="adv-section">
      <p className="adv-plain">{t('adv.changes.plain')}</p>
      <p>{decisions.length === 1 ? t('adv.changes.countOne') : t('adv.changes.count', { count: number(decisions.length) })}</p>
      <TableScroll label={t('adv.changes.label')}>
        <table className="adv-table">
          <thead>
            <tr>
              <th scope="col" className="num">
                #
              </th>
              <th scope="col">{t('adv.changes.kind')}</th>
              <th scope="col">{t('adv.changes.change')}</th>
              <th scope="col">
                <span className="adv-hidden">{t('adv.changes.remove')}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {decisions.slice(first, first + PER_PAGE).map((decision, at) => {
              const index = first + at;
              const [kind, words] = decisionWords(decision, t);
              const orphan = orphans.find((one) => one.decision === index);
              return (
                <tr key={`${String(index)}:${JSON.stringify(decision)}`}>
                  <td className="num">{number(index + 1)}</td>
                  <td>
                    <code>{kind}</code>
                  </td>
                  <td>
                    {words}
                    {orphan === undefined ? null : <p className="adv-problem">{t(ORPHAN_WORDS[orphan.reason])}</p>}
                  </td>
                  <td>
                    <button
                      type="button"
                      aria-label={t('adv.changes.removeOne', { number: index + 1 })}
                      disabled={!draft.editable}
                      onClick={() => {
                        draft.change((plan) => removeDecision(plan, index));
                      }}
                    >
                      {t('adv.changes.remove')}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableScroll>
      {lastPage > 1 ? <Pager page={shown} lastPage={lastPage} total={decisions.length} label={t('adv.changes.resource')} onPage={setPage} /> : null}
      <p className="import-row-actions">
        {clearing ? (
          <>
            <span>{t('adv.changes.clearSure', { count: number(decisions.length) })}</span>{' '}
            <button
              type="button"
              className="adv-danger-button"
              disabled={!draft.editable}
              onClick={() => {
                draft.change(removeDecisions);
                setClearing(false);
              }}
            >
              {t('adv.changes.clearYes', { count: number(decisions.length) })}
            </button>{' '}
            <button type="button" onClick={() => { setClearing(false); }}>
              {t('adv.changes.clearNo')}
            </button>
          </>
        ) : (
          <button type="button" disabled={!draft.editable} onClick={() => { setClearing(true); }}>
            {t('adv.changes.clear', { count: number(decisions.length) })}
          </button>
        )}
      </p>
    </div>
  );
};

export default AdvancedChanges;
