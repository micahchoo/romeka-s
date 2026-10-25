/**
 * Which items a bulk change reaches: the ticked ones, or every item the list matches. The edit
 * step and the delete step both ask it.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import type { Scope } from './model';

/** The two selections the panel can name, with their sizes. */
export interface Reach {
  readonly ticked: number;
  readonly matching: number;
  /** The list has a filter, so "all that match" is not all items. */
  readonly filtered: boolean;
  /** The person's role may act on every matching item (`batch_update_all`, `batch_delete_all`). */
  readonly canMatchAll: boolean;
}

export const ScopeChoice = ({
  scope,
  reach,
  refused,
  onScope,
}: {
  readonly scope: Scope;
  readonly reach: Reach;
  /** Why "all that match" is closed to the person's role. */
  readonly refused: string;
  readonly onScope: (scope: Scope) => void;
}): ReactElement => {
  const { t, number } = useTranslator(bulk);
  const tickedText = reach.ticked === 1 ? t('bulk.scope.tickedOne') : t('bulk.scope.tickedMany', { count: number(reach.ticked) });
  const many = reach.filtered ? 'bulk.scope.matchingMany' : 'bulk.scope.allMany';
  const one = reach.filtered ? 'bulk.scope.matchingOne' : 'bulk.scope.allOne';
  const matchingText = reach.matching === 1 ? t(one) : t(many, { count: number(reach.matching) });
  const radio = (value: Scope, text: string, disabled: boolean): ReactElement => (
    <label>
      <input
        type="radio"
        name="bulk-scope"
        value={value}
        checked={scope === value}
        disabled={disabled}
        onChange={() => {
          onScope(value);
        }}
      />{' '}
      {text}
    </label>
  );
  return (
    <fieldset className="bulk-scope">
      <legend>{t('bulk.scope.legend')}</legend>
      {reach.ticked > 0 ? radio('ticked', tickedText, false) : null}
      {radio('matching', matchingText, !reach.canMatchAll)}
      {reach.canMatchAll ? null : <p className="bulk-note">{refused}</p>}
    </fieldset>
  );
};
