/**
 * The layout-data form: the styling a block or a page keeps in `o:layout_data`,
 * rendered from the schema the registry carries. Each field's `group` puts it
 * in a section (Spacing, Size, Background, Alignment, Grid position) and
 * `advanced` puts it under a closed "Advanced" disclosure, so the form reads
 * as a few named sections rather than one flat list.
 *
 * The renderer reads a grid position or span only on a grid page
 * (`layout.rs::block_classes`), so on any other page the Grid section is
 * disabled and says why, and its stored values are kept.
 */
import type { ReactElement } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import { FIELD_GROUPS } from '../../blockSchema';
import type { BlockField, FieldGroup, LayoutSchema } from '../../blockSchema';
import { useEditorText } from '../messages';
import type { EditorMessageKey } from '../messages';
import type { EditorOptions } from '../options';
import { FieldControl } from './FieldControl';
import './controls.css';

const GROUP_LABELS: Readonly<Record<FieldGroup, EditorMessageKey>> = {
  spacing: 'layoutGroup.spacing',
  size: 'layoutGroup.size',
  background: 'layoutGroup.background',
  alignment: 'layoutGroup.alignment',
  grid: 'layoutGroup.grid',
};

export interface LayoutDataFormProps {
  readonly idPrefix: string;
  /** The group label; the catalogue's block-styling label by default. */
  readonly legend?: string;
  readonly schema: LayoutSchema;
  readonly values: Readonly<Record<string, JsonValue>>;
  /** Writes one or more keys as one edit. */
  readonly onChange: (values: Readonly<Record<string, JsonValue>>) => void;
  /** Whether the page is a grid, which is when the Grid section applies. */
  readonly grid: boolean;
  readonly siteId: SiteId;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
}

export const LayoutDataForm = ({
  idPrefix,
  legend,
  schema,
  values,
  onChange,
  grid,
  siteId,
  options,
}: LayoutDataFormProps): ReactElement => {
  const t = useEditorText();
  const control = (field: BlockField): ReactElement => (
    <FieldControl
      key={field.key}
      field={field}
      value={values[field.key]}
      onChange={(value) => {
        onChange({ [field.key]: value });
      }}
      record={values}
      siblings={schema.fields}
      onPatch={onChange}
      path={field.key}
      issues={[]}
      blockKey={null}
      siteId={siteId}
      idPrefix={idPrefix}
      {...(options === undefined ? {} : { options })}
    />
  );
  const loose = schema.fields.filter((field) => field.group === null && !field.advanced);
  const advanced = schema.fields.filter((field) => field.advanced);
  return (
    <fieldset className="block-field layout-data-form">
      <legend>{legend ?? t('layoutData.legend')}</legend>
      {loose.map(control)}
      {FIELD_GROUPS.map((group) => {
        const fields = schema.fields.filter((field) => field.group === group && !field.advanced);
        if (fields.length === 0) {
          return null;
        }
        const disabled = group === 'grid' && !grid;
        return (
          <fieldset key={group} className="layout-group" disabled={disabled}>
            <legend>{t(GROUP_LABELS[group])}</legend>
            {disabled ? <p className="field-note">{t('layoutGroup.gridOnly')}</p> : null}
            {fields.map(control)}
          </fieldset>
        );
      })}
      {advanced.length === 0 ? null : (
        <details className="layout-group layout-advanced">
          <summary>{t('layoutGroup.advanced')}</summary>
          {advanced.map(control)}
        </details>
      )}
    </fieldset>
  );
};
