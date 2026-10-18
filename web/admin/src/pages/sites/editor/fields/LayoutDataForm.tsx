/**
 * The layout-data form: the styling and grid fields a block or a page keeps in
 * `o:layout_data`, rendered from the schema the registry carries. The `class`
 * and grid keys are text, the alignments and background positions are choices,
 * and `background_image_asset` is an asset reference.
 */
import type { ReactElement } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import type { LayoutSchema } from '../../blockSchema';
import { FieldControl } from './FieldControl';

export interface LayoutDataFormProps {
  readonly idPrefix: string;
  /** The group label; the catalogue's block-styling label by default. */
  readonly legend?: string;
  readonly schema: LayoutSchema;
  readonly values: Readonly<Record<string, JsonValue>>;
  readonly onChange: (key: string, value: JsonValue) => void;
  readonly siteId: SiteId;
}

export const LayoutDataForm = ({
  idPrefix,
  legend,
  schema,
  values,
  onChange,
  siteId,
}: LayoutDataFormProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <fieldset className="block-field layout-data-form">
      <legend>{legend ?? t('layoutData.legend')}</legend>
      {schema.fields.map((field) => (
        <FieldControl
          key={field.key}
          field={field}
          value={values[field.key]}
          onChange={(value) => {
            onChange(field.key, value);
          }}
          path={field.key}
          issues={[]}
          blockKey={null}
          siteId={siteId}
          idPrefix={idPrefix}
        />
      ))}
    </fieldset>
  );
};
