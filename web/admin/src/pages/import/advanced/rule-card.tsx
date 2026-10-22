/**
 * One field's rule: its sources in order (the first with a value wins under
 * "first", all add up under "all"), each with up, down and remove; a source
 * to add; and, for a field that names things, the value map with the count
 * of items that hold each value.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import { isMultiValued } from '../../../api/import-plan';
import type { SlotName, SlotRule } from '../../../api/import-plan';
import type { Property } from '../../../api/types';
import { ErrorNotice } from '../../../app/errors';
import { Pager } from '../../../app/list';
import { TableScroll } from '../../../app/table-scroll';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { slotWords } from './field-names';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey } from './messages';
import { addSource, mappedText, moveSource, removeRule, removeSource, setCombine, setMapped } from './plan-edits';
import type { RuleAt } from './plan-edits';
import { choiceFor, sourceWords } from './source-words';
import type { SourceChoice } from './source-words';

export const VALUES_PER_PAGE = 50;

/** The fields whose values name things, which the server counts (`values`). */
const NAMING: readonly SlotName[] = ['sets', 'template', 'class'];

const RULE_PLAIN: Partial<Record<SlotName, AdvancedMessageKey>> = {
  sets: 'adv.fields.rule.plainSets',
  template: 'adv.fields.rule.plainTemplate',
  class: 'adv.fields.rule.plainClass',
  visibility: 'adv.fields.rule.plainVisibility',
};

const ValueMap = ({ at, rule }: { readonly at: RuleAt; readonly rule: SlotRule }): ReactElement | null => {
  const { kind, slot } = at;
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const [page, setPage] = useState(1);
  const read = usePreview(['values'], { values: { kind, page, perPage: VALUES_PER_PAGE } });
  if (read.status === 'loading') {
    return <p className="import-slow">{t('adv.fields.map.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const values = read.data.values.find((one) => one.slot === slot)?.values;
  if (values === undefined || values.total === 0) {
    return <p className="adv-plain">{t('adv.fields.map.none')}</p>;
  }
  const lastPage = Math.max(1, Math.ceil(values.total / values.perPage));
  const name = slotWords(slot, [], t);
  return (
    <div className="adv-map" aria-busy={read.busy}>
      <p className="adv-plain">{t('adv.fields.map.plain')}</p>
      <TableScroll label={t('adv.fields.map.label', { field: name })}>
        <table>
          <thead>
            <tr>
              <th scope="col">{t('adv.fields.map.value')}</th>
              <th scope="col" className="num">
                {t('adv.fields.map.items')}
              </th>
              <th scope="col">{t('adv.fields.map.becomes')}</th>
            </tr>
          </thead>
          <tbody>
            {values.items.map((value) => (
              <tr key={value.text}>
                <th scope="row">
                  {value.text}
                  {value.spellings.length > 1 ? <p className="adv-plain">{t('adv.fields.map.spellings', { spellings: value.spellings.join(', ') })}</p> : null}
                </th>
                <td className="num">{number(value.count)}</td>
                <td>
                  <input
                    type="text"
                    aria-label={t('adv.fields.map.becomesOf', { value: value.text })}
                    value={mappedText(rule, value.text)}
                    disabled={!draft.editable}
                    onChange={(event) => {
                      const to = event.target.value;
                      draft.change((plan) => setMapped(plan, at, value.text, to));
                    }}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </TableScroll>
      {lastPage > 1 ? <Pager page={page} lastPage={lastPage} total={values.total} label={t('adv.fields.map.resource')} onPage={setPage} /> : null}
    </div>
  );
};

const AddConstant = ({ at }: { readonly at: RuleAt }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const [value, setValue] = useState('');
  const id = useId();
  return (
    <form
      className="adv-inline"
      onSubmit={(event) => {
        event.preventDefault();
        if (value.trim() !== '') {
          draft.change((plan) => addSource(plan, at, { from: 'constant', value: value.trim() }));
          setValue('');
        }
      }}
    >
      <label htmlFor={id}>{t('adv.fields.rule.constant')}</label>{' '}
      <input id={id} type="text" value={value} disabled={!draft.editable} onChange={(event) => { setValue(event.target.value); }} />{' '}
      <button type="submit" disabled={!draft.editable || value.trim() === ''}>
        {t('adv.fields.rule.addConstant')}
      </button>
    </form>
  );
};

export interface RuleCardProps {
  readonly at: RuleAt;
  readonly rule: SlotRule;
  readonly properties: readonly Property[];
  readonly choices: readonly SourceChoice[];
}

export const RuleCard = ({ at, rule, properties, choices }: RuleCardProps): ReactElement => {
  const { slot } = at;
  const { t } = useAdvancedText();
  const draft = useDraft();
  const name = slotWords(slot, properties, t);
  const one = !isMultiValued(slot);
  const edit = (change: Parameters<typeof draft.change>[0]): void => {
    draft.change(change);
  };
  return (
    <article className="import-card adv-rule" aria-label={name}>
      <h5>{name}</h5>
      <p className="adv-plain">{t(RULE_PLAIN[slot] ?? 'adv.fields.rule.plain')}</p>
      <ol className="adv-sources">
        {rule.sources.length === 0 ? <li className="adv-plain">{t('adv.fields.rule.noSource')}</li> : null}
        {rule.sources.map((source, place) => {
          const words = sourceWords(source, draft.plan, t);
          return (
            <li key={`${String(place)}:${JSON.stringify(source)}`}>
              <span>{words}</span>{' '}
              <button type="button" aria-label={t('adv.fields.rule.upOf', { source: words })} disabled={!draft.editable || place === 0} onClick={() => { edit((plan) => moveSource(plan, at, place, -1)); }}>
                {t('adv.fields.rule.up')}
              </button>{' '}
              <button type="button" aria-label={t('adv.fields.rule.downOf', { source: words })} disabled={!draft.editable || place === rule.sources.length - 1} onClick={() => { edit((plan) => moveSource(plan, at, place, 1)); }}>
                {t('adv.fields.rule.down')}
              </button>{' '}
              <button type="button" aria-label={t('adv.fields.rule.removeOf', { source: words })} disabled={!draft.editable} onClick={() => { edit((plan) => removeSource(plan, at, place)); }}>
                {t('adv.fields.rule.remove')}
              </button>
            </li>
          );
        })}
      </ol>
      <p className="adv-inline">
        <select
          aria-label={t('adv.fields.rule.addOf', { field: name })}
          value=""
          disabled={!draft.editable}
          onChange={(event) => {
            const source = choiceFor(choices, event.target.value);
            if (source !== null) {
              edit((plan) => addSource(plan, at, source));
            }
          }}
        >
          <option value="">{t('adv.fields.rule.add')}</option>
          {choices.map((choice) => (
            <option key={choice.value} value={choice.value}>
              {choice.label}
            </option>
          ))}
        </select>{' '}
        <select
          aria-label={t('adv.fields.rule.combineOf', { field: name })}
          value={rule.combine}
          disabled={!draft.editable}
          onChange={(event) => {
            const combine = event.target.value === 'all' ? 'all' : 'first';
            edit((plan) => setCombine(plan, at, combine));
          }}
        >
          <option value="first">{t('adv.fields.rule.first')}</option>
          <option value="all" disabled={one}>
            {t('adv.fields.rule.all')}
          </option>
        </select>
      </p>
      {one ? <p className="adv-plain">{t('adv.fields.rule.oneValue')}</p> : null}
      <AddConstant at={at} />
      {NAMING.includes(slot) ? <ValueMap at={at} rule={rule} /> : null}
      <p>
        <button type="button" disabled={!draft.editable} onClick={() => { edit((plan) => removeRule(plan, at)); }}>
          {t('adv.fields.rule.removeRule', { field: name })}
        </button>
      </p>
    </article>
  );
};
