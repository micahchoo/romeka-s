/**
 * The layout-data form: the styling a block or a page keeps in `o:layout_data`,
 * rendered from the schema the registry carries. Each field's `group` puts it
 * in a section (Spacing, Size, Background, Alignment, Grid) and `advanced`
 * puts it under a closed "Advanced" disclosure, so the form reads as a few
 * named sections rather than one flat list.
 *
 * The Grid section applies only on a grid page, and what it means depends on
 * whose layout data this is. A block's grid fields place it in the page grid,
 * which the renderer reads only on a grid page (`layout.rs::block_classes`),
 * so off the grid the section is disabled, says why, and keeps its values. A
 * page's grid fields define the grid itself, so off the grid they are hidden:
 * the page layout control right above them brings them back.
 */
import type { ReactElement, ReactNode } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import { FIELD_GROUPS } from '../../blockSchema';
import type { BlockField, FieldGroup, LayoutSchema } from '../../blockSchema';
import { useEditorText } from '../messages';
import type { EditorMessageKey } from '../messages';
import type { EditorOptions } from '../options';
import { FieldControl } from './FieldControl';
import './controls.css';

const GROUP_LABELS: Readonly<Record<Exclude<FieldGroup, 'grid'>, EditorMessageKey>> = {
  spacing: 'layoutGroup.spacing',
  size: 'layoutGroup.size',
  background: 'layoutGroup.background',
  alignment: 'layoutGroup.alignment',
};

/** Whose `o:layout_data` the form edits. */
export type LayoutDataOwner = 'block' | 'page';

const OWNERS: Readonly<
  Record<
    LayoutDataOwner,
    {
      readonly legend: EditorMessageKey;
      readonly grid: EditorMessageKey;
      readonly offGrid: 'disable' | 'hide';
    }
  >
> = {
  block: { legend: 'layoutData.legend', grid: 'layoutGroup.grid', offGrid: 'disable' },
  page: { legend: 'layoutData.pageLegend', grid: 'layoutGroup.pageGrid', offGrid: 'hide' },
};

export interface LayoutDataFormProps {
  readonly idPrefix: string;
  readonly owner: LayoutDataOwner;
  readonly schema: LayoutSchema;
  readonly values: Readonly<Record<string, JsonValue>>;
  /** Writes one or more keys as one edit. */
  readonly onChange: (values: Readonly<Record<string, JsonValue>>) => void;
  /** Whether the page is a grid, which is when the Grid section applies. */
  readonly grid: boolean;
  readonly siteId: SiteId;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
  /** Controls the form shows before the schema's fields, such as the page layout. */
  readonly children?: ReactNode;
}

export const LayoutDataForm = ({
  idPrefix,
  owner,
  schema,
  values,
  onChange,
  grid,
  siteId,
  options,
  children,
}: LayoutDataFormProps): ReactElement => {
  const t = useEditorText();
  const labels = OWNERS[owner];
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
      <legend>{t(labels.legend)}</legend>
      {children}
      {loose.map(control)}
      {FIELD_GROUPS.map((group) => {
        const fields = schema.fields.filter((field) => field.group === group && !field.advanced);
        if (fields.length === 0) {
          return null;
        }
        const offGrid = group === 'grid' && !grid;
        if (offGrid && labels.offGrid === 'hide') {
          return null;
        }
        return (
          <fieldset key={group} className="layout-group" disabled={offGrid}>
            <legend>{t(group === 'grid' ? labels.grid : GROUP_LABELS[group])}</legend>
            {offGrid ? <p className="field-note">{t('layoutGroup.gridOnly')}</p> : null}
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
