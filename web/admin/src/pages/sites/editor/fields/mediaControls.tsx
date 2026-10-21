/**
 * The controls of the field kinds that name a media of the site: one media
 * (`mediaRef`). The focal point on that media's image is its own chunk
 * (`focalPointControl.tsx`), because only the hero uses it.
 */
import type { ReactElement } from 'react';
import { useEditorText } from '../messages';
import type { ControlProps, FieldOf } from './controlBase';
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
      <MediaRefField
        id={id}
        label={e(field.label)}
        help={e(field.help)}
        accepts={field.accepts}
        value={value}
        onChange={onChange}
      />
      {error}
    </>
  );
};
