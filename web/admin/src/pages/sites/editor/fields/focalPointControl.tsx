/**
 * The hero's focal point control, in a chunk of its own: only the hero names a
 * focal point, and the control's frame is the largest field control the editor
 * carries. `mediaControls.tsx` keeps the media picker; this file keeps the
 * point on its image, so the shared `fields` chunk stays under its budget.
 */
import type { ReactElement } from 'react';
import type { BlockField } from '../../blockSchema';
import { useEditorText } from '../messages';
import type { ControlProps, FieldOf } from './controlBase';
import { FocalPointField, focalImage } from './FocalPointField';

export type FocalPointControlProps = ControlProps & {
  readonly field: FieldOf<'focalPoint'>;
  /** The fields of the same list, which name the image the point is on. */
  readonly siblings: readonly BlockField[];
};

export const FocalPointControl = ({
  field,
  id,
  record,
  siblings,
  onPatch,
}: FocalPointControlProps): ReactElement => {
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
