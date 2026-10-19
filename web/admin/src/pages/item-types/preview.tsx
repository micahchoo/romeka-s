/**
 * The live preview: the real values editor the item form renders, laid out by
 * the rows being edited. What a user types here stays here; it is never sent.
 */
import { useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import type { DataType, Property } from '../../api/types';
import { emptyResourceDraft, isBlank, type ResourceDraft } from '../../app/draft';
import { withTemplateRows, type TemplateLayout } from '../../app/value-fields';
import { ValuesEditor } from '../../app/values-editor';
import { useItemTypesText } from './messages';
import { asLayout, type TypeDraft } from './model';

/** The template's rows laid in again, keeping whatever was typed. */
const relaid = (
  preview: ResourceDraft,
  layout: TemplateLayout,
  properties: readonly Property[],
): ResourceDraft =>
  withTemplateRows(
    { ...preview, values: preview.values.filter((value) => !isBlank(value)) },
    layout,
    properties,
  );

export interface TypePreviewProps {
  readonly id: string;
  readonly draft: TypeDraft;
  readonly properties: readonly Property[];
  readonly dataTypes: readonly DataType[];
}

export const TypePreview = ({
  id,
  draft,
  properties,
  dataTypes,
}: TypePreviewProps): ReactElement => {
  const t = useItemTypesText();
  const layout = useMemo(() => asLayout(draft), [draft]);
  const [preview, setPreview] = useState(() => relaid(emptyResourceDraft(), layout, properties));
  const [shownFor, setShownFor] = useState(layout);
  if (shownFor !== layout) {
    setShownFor(layout);
    setPreview(relaid(preview, layout, properties));
  }
  const name = draft.label.trim() === '' ? t('preview.unnamed') : draft.label.trim();
  return (
    <form
      id={id}
      className="type-preview"
      aria-label={t('preview.label', { name })}
      onSubmit={(event) => {
        event.preventDefault();
      }}
    >
      <h2 className="type-eyebrow">{t('preview.heading', { name })}</h2>
      <div className="type-preview-card">
        <h3>{t('preview.title', { name })}</h3>
        <p className="field-note">{t('preview.note')}</p>
        <ValuesEditor
          idPrefix={`${id}-values`}
          draft={preview}
          onChange={setPreview}
          template={layout}
          properties={properties}
          dataTypes={dataTypes}
        />
      </div>
    </form>
  );
};
