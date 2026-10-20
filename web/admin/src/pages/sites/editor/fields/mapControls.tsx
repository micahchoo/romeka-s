/**
 * The controls of the field kinds the Map block brings: a list of picked
 * items, a starting view (a centre and a zoom under two keys) and a property.
 */
import type { ReactElement } from 'react';
import { brandId } from '../../../../api/parse';
import type { ItemId, JsonValue } from '../../../../api/types';
import { useEditorText } from '../messages';
import { WithHelp, asText } from './controlBase';
import type { ControlProps, FieldOf } from './controlBase';
import { LazyItemPicker } from './LazyItemPicker';
import { MapViewField } from './MapViewField';
import { PropertySelect } from './PropertySelect';

/** The stored ids of an item list, in order, skipping anything that is not one. */
const asItemIds = (value: JsonValue | undefined): readonly ItemId[] =>
  Array.isArray(value)
    ? value.flatMap((entry) =>
        typeof entry === 'number' && Number.isInteger(entry) && entry > 0
          ? [brandId<'ItemId'>(entry)]
          : [],
      )
    : [];

export const ItemListControl = ({
  field,
  id,
  value,
  onChange,
  error,
}: ControlProps & { readonly field: FieldOf<'itemList'> }): ReactElement => {
  const e = useEditorText();
  return (
    <WithHelp id={id} help={e(field.help)}>
      <LazyItemPicker
        id={id}
        label={e(field.label)}
        mode="many"
        picked={asItemIds(value)}
        onChange={(picked) => {
          onChange([...picked]);
        }}
      />
      {error}
    </WithHelp>
  );
};

export const MapViewControl = ({
  field,
  id,
  record,
  onPatch,
  siteId,
}: ControlProps & { readonly field: FieldOf<'mapView'> }): ReactElement => {
  const e = useEditorText();
  return (
    <MapViewField
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      centreKey={field.key}
      zoomKey={field.zoomKey}
      record={record}
      onPatch={onPatch}
      siteId={siteId}
    />
  );
};

export const PropertyControl = ({
  field,
  id,
  value,
  onChange,
}: ControlProps & { readonly field: FieldOf<'property'> }): ReactElement => {
  const e = useEditorText();
  return (
    <PropertySelect
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      value={asText(value)}
      onChange={onChange}
    />
  );
};
