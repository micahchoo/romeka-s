/**
 * The Section panel: what the inspector shows for a block group. The layout is
 * a row of small diagrams, one per section layout; under it sit the alignment,
 * gap and overlay, the section's background, and its slots, which list the
 * group's blocks in the columns they take.
 *
 * Every value lives in the group's `o:layout_data` (the section under
 * `romeka.section`, the background in upstream's own members), so each edit
 * leaves as one `setLayoutDataValues` action.
 */
import type { ReactElement } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import type { BlockField, BlockFieldOption, BlockRegistry } from '../../blockSchema';
import { layoutLabel } from '../canvas/summary';
import type { BlockDraft, BlockKey, DraftAction, ValidationIssue } from '../draft';
import { Help, helpId } from '../Labelled';
import { formValues } from '../layoutPaths';
import { counted, useEditorText } from '../messages';
import type { EditorOptions } from '../options';
import {
  SECTION_LAYOUT_PATH,
  SECTION_SHAPES,
  emptySlots,
  gridColumns,
  sectionOf,
} from '../sections';
import type { SectionShape } from '../sections';
import { layoutDataObject } from '../values';
import { useTranslator } from '../../../../i18n';
import { EDITOR_NAMESPACES } from '../messages';
import { sites } from '../../../../i18n/namespaces/sites';
import { FieldControl } from './FieldControl';
import './section.css';

/** A group with no section layout follows the page, which lists its blocks one under another. */
const STACK: SectionShape = { columns: [1], cards: false, bleed: false };

export interface SectionFormProps {
  readonly block: BlockDraft;
  /** The group's blocks, in slot order. */
  readonly members: readonly BlockDraft[];
  readonly registry: BlockRegistry;
  readonly issues: readonly ValidationIssue[];
  readonly siteId: SiteId;
  readonly dispatch: (action: DraftAction) => void;
  readonly onSelect: (key: BlockKey) => void;
  readonly options?: EditorOptions;
}

/** One layout's diagram: its columns as bars, cards drawn as boxes, a band edge to edge. */
const Diagram = ({ shape }: { readonly shape: SectionShape }): ReactElement => (
  <span
    className={[
      'section-diagram',
      shape.cards ? 'section-diagram-cards' : '',
      shape.bleed ? 'section-diagram-band' : '',
    ].join(' ')}
    style={{ gridTemplateColumns: gridColumns(shape) }}
    aria-hidden="true"
  >
    {shape.columns.map((_, index) => (
      <span key={index} className="section-diagram-slot" />
    ))}
  </span>
);

const LayoutPicker = ({
  id,
  field,
  options,
  value,
  onChange,
}: {
  readonly id: string;
  readonly field: BlockField;
  readonly options: readonly BlockFieldOption[];
  readonly value: string;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const e = useEditorText();
  return (
    <div className="block-field">
      <span id={`${id}-label`} className="section-picker-label">
        {e(field.label)}
      </span>
      <div
        className="section-picker"
        role="radiogroup"
        aria-labelledby={`${id}-label`}
        aria-describedby={helpId(id)}
      >
        {options.map((option) => {
          const shape = SECTION_SHAPES.get(option.value);
          return (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={option.value === value}
              className="section-choice"
              onClick={() => {
                onChange(option.value);
              }}
            >
              {shape === undefined ? null : <Diagram shape={shape} />}
              <span className="section-choice-name">{e(option.label)}</span>
            </button>
          );
        })}
      </div>
      <Help id={id} text={e(field.help)} />
    </div>
  );
};

/** The section's slots: each block in the column it takes, and the empty slots after them. */
const Slots = ({
  shape,
  members,
  registry,
  onSelect,
}: {
  readonly shape: SectionShape;
  readonly members: readonly BlockDraft[];
  readonly registry: BlockRegistry;
  readonly onSelect: (key: BlockKey) => void;
}): ReactElement => {
  const { t } = useTranslator(sites, ...EDITOR_NAMESPACES);
  const empty = emptySlots(shape, members.length);
  return (
    <div className="block-field">
      <span className="section-picker-label">{t('section.slots')}</span>
      <ol className="section-slots" style={{ gridTemplateColumns: gridColumns(shape) }}>
        {members.map((member, index) => (
          <li key={member.key}>
            <button
              type="button"
              className="section-slot"
              onClick={() => {
                onSelect(member.key);
              }}
            >
              <span className="section-slot-number">{t('section.slot', { slot: index + 1 })}</span>
              {layoutLabel(registry, member, t)}
            </button>
          </li>
        ))}
        {Array.from({ length: empty }, (_, index) => (
          <li key={`empty-${String(index)}`} className="section-slot section-slot-empty">
            {t('canvas.slotEmpty', { slot: members.length + index + 1 })}
          </li>
        ))}
      </ol>
      <p className="field-help">
        {counted(t, 'blocks', members.length)}. {t('section.slotsHelp')}
      </p>
    </div>
  );
};

export const SectionForm = ({
  block,
  members,
  registry,
  issues,
  siteId,
  dispatch,
  onSelect,
  options,
}: SectionFormProps): ReactElement => {
  const e = useEditorText();
  const idPrefix = `block-${block.key}-section`;
  const background = registry.blockLayoutData.fields.filter((field) => field.group === 'background');
  const sectionFields = registry.sectionLayoutData.fields;
  const values = formValues(layoutDataObject(block), [...sectionFields, ...background]);
  const write = (patch: Readonly<Record<string, JsonValue>>): void => {
    dispatch({ type: 'setLayoutDataValues', blockKey: block.key, values: patch });
  };
  const section = sectionOf(block);
  const control = (field: BlockField, siblings: readonly BlockField[]): ReactElement => (
    <FieldControl
      key={field.key}
      field={field}
      value={values[field.key]}
      onChange={(value) => {
        write({ [field.key]: value });
      }}
      record={values}
      siblings={siblings}
      onPatch={write}
      path={field.key}
      issues={issues}
      blockKey={block.key}
      siteId={siteId}
      idPrefix={idPrefix}
      {...(options === undefined ? {} : { options })}
    />
  );
  return (
    <fieldset className="block-field layout-data-form section-form">
      <legend>{e('section.legend')}</legend>
      {sectionFields.map((field) =>
        field.key === SECTION_LAYOUT_PATH && field.type === 'choice' ? (
          <LayoutPicker
            key={field.key}
            id={`${idPrefix}-layout`}
            field={field}
            options={field.options}
            value={section?.layout ?? ''}
            onChange={(value) => {
              write({ [field.key]: value });
            }}
          />
        ) : section === null ? null : (
          control(field, sectionFields)
        ),
      )}
      {section === null ? <p className="field-note">{e('section.none')}</p> : null}
      <Slots
        shape={section?.shape ?? STACK}
        members={members}
        registry={registry}
        onSelect={onSelect}
      />
      <fieldset className="layout-group">
        <legend>{e('layoutGroup.background')}</legend>
        {background.map((field) => control(field, background))}
      </fieldset>
    </fieldset>
  );
};
