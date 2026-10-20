/**
 * The `property` field control: one property, stored as its term. The list
 * loads when the control first shows, so a page with no such field never
 * asks for it.
 */
import type { ReactElement } from 'react';
import { useEditorText } from '../messages';
import { Labelled, helpId } from '../Labelled';
import { propertyName, useProperties } from './useProperties';

export interface PropertySelectProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly value: string;
  readonly onChange: (term: string) => void;
}

export const PropertySelect = ({
  id,
  label,
  help,
  value,
  onChange,
}: PropertySelectProps): ReactElement => {
  const t = useEditorText();
  const state = useProperties();
  const properties = state.status === 'ready' ? state.data : [];
  const listed = value === '' || properties.some((property) => property.term === value);
  return (
    <Labelled id={id} label={label} help={help}>
      <select
        id={id}
        value={value}
        aria-describedby={helpId(id)}
        aria-busy={state.status === 'loading'}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        <option value="">{t('property.none')}</option>
        {listed ? null : <option value={value}>{value}</option>}
        {properties.map((property) => (
          <option key={String(property.id)} value={property.term}>
            {propertyName(property)}
          </option>
        ))}
      </select>
    </Labelled>
  );
};
