/**
 * Advanced, Existing items: what the import does to records already in
 * Omeka (one action for the whole import, each with one plain line), how a
 * record of this import finds the one already there (a key field) and where
 * it looks (a scope), per record kind, and what happens to one with no match.
 *
 * Delete cannot be undone, so it is the one action the person cannot choose
 * in one click: the server first counts what it would delete, and the action
 * is set only after that count is typed.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import { ACTIONS, RECORD_KINDS, isTerm } from '../../../api/import-plan';
import type { Action, MatchRule, RecordKind, Scope, Term } from '../../../api/import-plan';
import { importApi } from '../../../api/import-routes';
import { ErrorNotice } from '../../../app/errors';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadCatalog } from '../lists/catalog';
import { slotWords, useProperties } from './field-names';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey } from './messages';
import { ruleCount, setAction, setMatch, setUnmatched } from './plan-edits';
import { Setting } from './setting';
import './advanced.css';

const ACTION_WORDS: Readonly<Record<Action, readonly [AdvancedMessageKey, AdvancedMessageKey]>> = {
  create: ['adv.existing.create', 'adv.existing.createPlain'],
  append: ['adv.existing.append', 'adv.existing.appendPlain'],
  revise: ['adv.existing.revise', 'adv.existing.revisePlain'],
  update: ['adv.existing.update', 'adv.existing.updatePlain'],
  replace: ['adv.existing.replace', 'adv.existing.replacePlain'],
  delete: ['adv.existing.delete', 'adv.existing.deletePlain'],
};

const KIND_WORDS: Readonly<Record<RecordKind, AdvancedMessageKey>> = {
  item: 'adv.existing.match.item',
  media: 'adv.existing.match.media',
  itemSet: 'adv.existing.match.itemSet',
};

const DeleteLock = ({ onCancel }: { readonly onCancel: () => void }): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const [typed, setTyped] = useState('');
  const id = useId();
  const deleting = setAction(draft.plan, 'delete');
  const counted = useAsync(() => importApi.preview(draft.id, ['check'], { plan: deleting }).then((preview) => preview.check.deletes), [draft.id, JSON.stringify(deleting)]);
  const rules = ruleCount(draft.plan);
  if (counted.status === 'loading') {
    return <p role="status" className="import-slow">{t('adv.existing.deleteCounting')}</p>;
  }
  if (counted.status === 'error') {
    return <ErrorNotice error={counted.error} />;
  }
  const count = counted.data;
  const open = count > 0 && typed.trim() === String(count);
  return (
    <div className="import-notice adv-danger" role="alert">
      <p>
        <b>{count === 1 ? t('adv.existing.deleteCountOne') : t('adv.existing.deleteCount', { count: number(count) })}</b>
      </p>
      {rules === 0 ? null : <p>{t('adv.existing.deleteRules', { count: number(rules) })}</p>}
      {count === 0 ? (
        <p>{t('adv.existing.deleteNothing')}</p>
      ) : (
        <form
          className="adv-inline"
          onSubmit={(event) => {
            event.preventDefault();
            if (open) {
              draft.change((plan) => setAction(plan, 'delete'));
            }
          }}
        >
          <label htmlFor={id}>{t('adv.existing.deleteType', { count: String(count) })}</label>{' '}
          <input id={id} type="text" inputMode="numeric" autoComplete="off" className="adv-narrow" value={typed} onChange={(event) => { setTyped(event.target.value); }} />{' '}
          <button type="submit" className="adv-danger-button" disabled={!open || !draft.editable}>
            {t('adv.existing.deleteChoose', { count: number(count) })}
          </button>
        </form>
      )}
      <p>
        <button type="button" onClick={onCancel}>
          {t('adv.existing.deleteCancel')}
        </button>
      </p>
    </div>
  );
};

const scopeValue = (scope: Scope): string => (typeof scope === 'object' ? 'sets' : scope);

const MatchOf = ({ kind }: { readonly kind: RecordKind }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const properties = useProperties();
  const catalog = useAsync(loadCatalog, []);
  const rule: MatchRule = draft.plan.action.match[kind];
  const known = properties.status === 'ready' ? properties.data : [];
  const terms = known.flatMap((property) => (isTerm(property.term) ? [property.term] : []));
  const keys = [...new Set<Term>(['dcterms:identifier', 'dcterms:title', ...terms, ...(rule.key === 'internalId' ? [] : [rule.key])])];
  const name = t(KIND_WORDS[kind]);
  const sets = catalog.status === 'ready' ? catalog.data.itemSets : [];
  const chosen = typeof rule.scope === 'object' ? rule.scope.sets : [];
  const edit = (match: Partial<MatchRule>): void => {
    draft.change((plan) => setMatch(plan, kind, match));
  };
  return (
    <Setting label={name} plain={t('adv.existing.match.plain')}>
      {() => (
        <>
          <p className="adv-inline">
            <select
              aria-label={t('adv.existing.match.keyOf', { kind: name })}
              value={rule.key}
              disabled={!draft.editable}
              onChange={(event) => {
                const key = event.target.value;
                edit({ key: key === 'internalId' ? 'internalId' : isTerm(key) ? key : rule.key });
              }}
            >
              <option value="internalId">{t('adv.existing.match.internalId')}</option>
              {keys.map((key) => (
                <option key={key} value={key}>
                  {slotWords(key, known, t)}
                </option>
              ))}
            </select>{' '}
            <select
              aria-label={t('adv.existing.match.scopeOf', { kind: name })}
              value={scopeValue(rule.scope)}
              disabled={!draft.editable}
              onChange={(event) => {
                const value = event.target.value;
                edit({ scope: value === 'all' ? 'all' : value === 'sets' ? { sets: chosen } : 'importSets' });
              }}
            >
              <option value="importSets">{t(kind === 'media' ? 'adv.existing.scope.importMedia' : 'adv.existing.scope.importSets')}</option>
              <option value="all">{t('adv.existing.scope.all')}</option>
              <option value="sets">{t('adv.existing.scope.sets')}</option>
            </select>
          </p>
          {typeof rule.scope === 'object' ? (
            <ul className="adv-list adv-scroll" aria-label={t('adv.existing.scope.setsOf', { kind: name })}>
              {catalog.status === 'error' ? <li><ErrorNotice error={catalog.error} /></li> : null}
              {sets.map((set) => (
                <li key={set.id}>
                  <label className="adv-check">
                    <input
                      type="checkbox"
                      checked={chosen.includes(set.id)}
                      disabled={!draft.editable}
                      onChange={(event) => {
                        const on = event.target.checked;
                        edit({ scope: { sets: on ? [...chosen, set.id] : chosen.filter((one) => one !== set.id) } });
                      }}
                    />{' '}
                    {set.title ?? `#${String(set.id)}`}
                  </label>
                </li>
              ))}
            </ul>
          ) : null}
        </>
      )}
    </Setting>
  );
};

export const AdvancedExisting = (): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const [asking, setAsking] = useState(false);
  const current = draft.plan.action.action;
  const name = useId();
  return (
    <div className="adv-section">
      <Setting label={t('adv.existing.action.label')} plain={t('adv.existing.action.plain')}>
        {(described) => (
          <>
            <div className="import-cards" role="radiogroup" aria-describedby={described} aria-label={t('adv.existing.action.label')}>
              {ACTIONS.map((action) => {
                const [label, plain] = ACTION_WORDS[action];
                const on = asking ? action === 'delete' : action === current;
                return (
                  <label key={action} className={`import-opt${on ? ' on' : ''}`}>
                    <input
                      type="radio"
                      name={name}
                      value={action}
                      checked={on}
                      disabled={!draft.editable}
                      onChange={() => {
                        if (action === 'delete') {
                          setAsking(current !== 'delete');
                          return;
                        }
                        setAsking(false);
                        draft.change((plan) => setAction(plan, action));
                      }}
                    />{' '}
                    <span>
                      <b>{t(label)}</b>. {t(plain)}
                    </span>
                  </label>
                );
              })}
            </div>
            {asking ? <DeleteLock onCancel={() => { setAsking(false); }} /> : null}
            {current === 'delete' && !asking ? <p className="adv-plain">{t('adv.existing.deleteChosen')}</p> : null}
          </>
        )}
      </Setting>
      <h4>{t('adv.existing.match.title')}</h4>
      {RECORD_KINDS.map((kind) => (
        <MatchOf key={kind} kind={kind} />
      ))}
      {current === 'create' ? null : (
        <Setting label={t('adv.existing.unmatched.label')} plain={t('adv.existing.unmatched.plain')}>
          {(described) => (
            <select
              aria-label={t('adv.existing.unmatched.label')}
              aria-describedby={described}
              value={draft.plan.flags.unmatched === undefined ? draft.plan.action.unmatched : draft.plan.flags.unmatched === 'leave' ? 'leave' : 'create'}
              disabled={!draft.editable}
              onChange={(event) => {
                const unmatched = event.target.value === 'create' ? 'create' : 'leave';
                draft.change((plan) => setUnmatched(plan, unmatched));
              }}
            >
              <option value="create">{t('adv.existing.unmatched.create')}</option>
              <option value="leave">{t('adv.existing.unmatched.leave')}</option>
            </select>
          )}
        </Setting>
      )}
    </div>
  );
};

export default AdvancedExisting;
