/**
 * One field's rule: its sources in order (the first with a value wins under
 * "first", all add up under "all"), each with up, down and remove; a source
 * to add; and the value map, with the count of records that hold each
 * value (`../value-map.tsx`). A column of a list whose rows describe
 * files shows where it goes, with the proposer's pick, as the Structure
 * node asks it.
 */
import { Suspense, lazy, useContext, useId, useState } from 'react';
import type { ReactElement } from 'react';
import { isMultiValued } from '../../../api/import-plan';
import type { LevelId, SlotName, SlotRule, SourceRef } from '../../../api/import-plan';
import type { NodeEvidence } from '../../../api/import-proposal';
import type { Property } from '../../../api/types';
import { useDraft } from '../draft-context';
import { fileRowsOf } from '../simple/structure-lines';
import type { PlacedColumn } from '../simple/structure-lines';
import { ScreenContext } from '../simple/structure-parts';
import { RollUpQuestion } from '../simple/structure-questions';
import { slotWords } from './field-names';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey } from './messages';
import { addSource, moveSource, removeRule, removeSource, setCombine } from './plan-edits';
import type { RuleAt } from './plan-edits';
import { choiceFor, sourceWords } from './source-words';
import type { SourceChoice } from './source-words';

const ValueMap = lazy(() => import('../value-map'));

const RULE_PLAIN: Partial<Record<SlotName, AdvancedMessageKey>> = {
  template: 'adv.fields.rule.plainTemplate',
  class: 'adv.fields.rule.plainClass',
  visibility: 'adv.fields.rule.plainVisibility',
};

/** The rule's value map, read only once opened: each open card asks the server for its values. */
const Values = ({ at, name }: { readonly at: RuleAt; readonly name: string }): ReactElement => {
  const { t } = useAdvancedText();
  const [open, setOpen] = useState(false);
  const label = t('adv.fields.map.label', { field: name });
  return (
    <details className="adv-map" onToggle={(event) => { setOpen(event.currentTarget.open); }}>
      <summary>{label}</summary>
      {open ? (
        <>
          <p className="adv-plain">{t('adv.fields.map.plain')}</p>
          <Suspense fallback={null}>
            <ValueMap home={at} label={label} />
          </Suspense>
        </>
      ) : null}
    </details>
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

/**
 * The placed columns of the suggested folder `level`, as the Structure tree
 * counts them, so a column source shows the proposer's roll-up pick; none
 * for a folder written by hand or outside the tree.
 */
const usePlacedColumns = (level: LevelId): readonly PlacedColumn[] => {
  const screen = useContext(ScreenContext);
  const { plan } = useDraft();
  const node = screen?.proposal.nodes.find((one): one is Extract<NodeEvidence, { kind: 'series' }> => one.kind === 'series' && one.level === level);
  return screen === null || node === undefined ? [] : (fileRowsOf(plan, screen.proposal, node)?.placed ?? []);
};

export const RuleCard = ({ at, rule, properties, choices }: RuleCardProps): ReactElement => {
  const { slot } = at;
  const { t } = useAdvancedText();
  const draft = useDraft();
  const placed = usePlacedColumns(at.level);
  const rollUpOf = (source: SourceRef): PlacedColumn | undefined =>
    source.from === 'column' ? placed.find((one) => one.fileAllowed && one.sheet === source.sheet && one.rollUp.index === source.column.index) : undefined;
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
          const column = rollUpOf(source);
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
              {column === undefined ? null : <RollUpQuestion column={column} />}
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
      <Values at={at} name={name} />
      <p>
        <button type="button" disabled={!draft.editable} onClick={() => { edit((plan) => removeRule(plan, at)); }}>
          {t('adv.fields.rule.removeRule', { field: name })}
        </button>
      </p>
    </article>
  );
};
