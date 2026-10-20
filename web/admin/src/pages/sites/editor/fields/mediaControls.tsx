/**
 * The controls of the field kinds that name a media of the site: one media
 * (`mediaRef`) and the point of its image a crop keeps in view (`focalPoint`).
 */
import type { ReactElement } from 'react';
import type { BlockField } from '../../blockSchema';
import { useEditorText } from '../messages';
import type { ControlProps, FieldOf } from './controlBase';
import { FocalPointField, focalImage } from './FocalPointField';
import { MediaRefField } from './MediaRefField';

export const MediaRefControl = ({
  field,
  id,
  value,
  onChange,
  error,
}: ControlProps & { readonly field: FieldOf<'mediaRef'> }): ReactElement => {
  const e = useEditorText();
  return (
    <>
      <MediaRefField id={id} label={e(field.label)} help={e(field.help)} value={value} onChange={onChange} />
      {error}
    </>
  );
};

export const FocalPointControl = ({
  field,
  id,
  record,
  siblings,
  onPatch,
}: ControlProps & {
  readonly field: FieldOf<'focalPoint'>;
  /** The fields of the same list, which name the image the point is on. */
  readonly siblings: readonly BlockField[];
}): ReactElement => {
  const e = useEditorText();
  return (
    <FocalPointField
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      x={record[field.key]}
      y={record[field.yKey]}
      image={focalImage(field.images, record, siblings)}
      onChange={(x, y) => {
        onPatch({ [field.key]: x, [field.yKey]: y });
      }}
    />
  );
};
