/**
 * The `propertyList` control: properties in the curator's order, stored as a
 * list of terms. A select adds one, each row can be moved or removed, and an
 * empty list is a real state the block reads as "every property".
 */
import type { ReactElement } from 'react';
import {
  ReorderButtons,
  ReorderHandle,
  ReorderStatus,
  useReorderable,
} from '../../../../app/reorder/useReorderable';
import { useTranslator } from '../../../../i18n';
import { editor } from '../../../../i18n/namespaces/editor';
import { Help, helpId } from '../Labelled';
import { usePropertyName, useProperties } from './useProperties';

export interface PropertyListFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly value: readonly string[];
  readonly onChange: (terms: readonly string[]) => void;
}

export const PropertyListField = ({
  id,
  label,
  help,
  value,
  onChange,
}: PropertyListFieldProps): ReactElement => {
  const { t } = useTranslator(editor);
  const state = useProperties();
  const propertyName = usePropertyName();
  const properties = state.status === 'ready' ? state.data : [];
  const nameOf = (term: string): string => {
    const found = properties.find((property) => property.term === term);
    return found === undefined ? term : propertyName(found);
  };
  const reorder = useReorderable({
    items: value,
    onReorder: onChange,
    labelOf: (term) => nameOf(term),
  });
  const addable = properties.filter((property) => !value.includes(property.term));
  return (
    <fieldset className="block-field property-list-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      {value.length === 0 ? (
        <p className="field-note">{t('propertyList.empty')}</p>
      ) : (
        <ol className="property-list-rows">
          {value.map((term, index) => (
            <li className="property-list-row" key={term} {...reorder.row(index)}>
              <ReorderHandle reorder={reorder} index={index} />
              <span className="property-list-name">{nameOf(term)}</span>
              <ReorderButtons reorder={reorder} index={index} />
              <button
                type="button"
                aria-label={t('propertyList.remove', { label: nameOf(term) })}
                onClick={() => {
                  onChange(value.filter((entry) => entry !== term));
                }}
              >
                <span aria-hidden="true">×</span>
              </button>
            </li>
          ))}
        </ol>
      )}
      <ReorderStatus reorder={reorder} />
      <label htmlFor={`${id}-add`}>{t('propertyList.add')}</label>
      <select
        id={`${id}-add`}
        value=""
        aria-busy={state.status === 'loading'}
        onChange={(event) => {
          if (event.target.value !== '') {
            onChange([...value, event.target.value]);
          }
        }}
      >
        <option value="">{t('propertyList.choose')}</option>
        {addable.map((property) => (
          <option key={String(property.id)} value={property.term}>
            {propertyName(property)}
          </option>
        ))}
      </select>
      <Help id={id} text={help} />
    </fieldset>
  );
};
