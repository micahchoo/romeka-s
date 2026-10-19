/**
 * The values of one resource, one group per property, laid out by the chosen
 * template (`value-fields.ts` owns the layout and the rules).
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { DataType, Property } from '../api/types';
import { useTranslator } from '../i18n';
import type { Translator } from '../i18n';
import { patchValue, removeValue } from './draft';
import type { ResourceDraft, ValueDraft } from './draft';
import {
  addFieldValue,
  addPropertyValue,
  layoutFields,
  setValueType,
  typeChoices,
  type PropertyField,
  type TemplateLayout,
} from './value-fields';
import { useDescribeText } from './describe-messages';
import { useLesson } from './lessons/lesson';
import { isLocalTerm } from './local-fields';
import { TermHelp, StandardTermPicker } from './standard-term-picker';

const CORE_TYPE_LABELS = {
  literal: 'field.kindText',
  uri: 'field.kindUri',
  resource: 'field.kindResource',
} as const;

const isCoreType = (type: string): type is keyof typeof CORE_TYPE_LABELS =>
  type in CORE_TYPE_LABELS;

const typeLabel = (type: string, dataTypes: readonly DataType[], t: Translator['t']): string => {
  if (isCoreType(type)) {
    return t(CORE_TYPE_LABELS[type]);
  }
  return dataTypes.find((dataType) => dataType.id === type)?.label ?? type;
};

interface Change {
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
}

interface ValueRowProps extends Change {
  readonly idPrefix: string;
  readonly field: PropertyField;
  readonly value: ValueDraft;
  readonly labelId: string;
  readonly dataTypes: readonly DataType[];
}

const ValueRow = ({
  idPrefix,
  field,
  value,
  labelId,
  dataTypes,
  draft,
  onChange,
}: ValueRowProps): ReactElement => {
  const { t } = useTranslator();
  const describe = useDescribeText();
  const id = (suffix: string): string => `${idPrefix}-${value.key}-${suffix}`;
  const patch = (next: Partial<ValueDraft>): void => {
    onChange(patchValue(draft, value.key, next));
  };
  const choices = typeChoices(field, value);
  const named = { field: field.label };
  return (
    <li className="value-row">
      {choices.length > 1 ? (
        <p>
          <label htmlFor={id('type')}>{describe('value.kind')}</label>
          <select
            id={id('type')}
            aria-label={t('valueField.dataType', named)}
            value={value.type}
            onChange={(event) => {
              onChange(setValueType(draft, value.key, event.target.value));
            }}
          >
            {choices.map((type) => (
              <option key={type} value={type}>
                {typeLabel(type, dataTypes, t)}
              </option>
            ))}
          </select>
        </p>
      ) : null}
      {value.kind === 'literal' ? (
        <p className="value-field-wide">
          <textarea
            id={id('text')}
            rows={1}
            aria-labelledby={labelId}
            value={value.text}
            onChange={(event) => {
              patch({ text: event.target.value });
            }}
          />
        </p>
      ) : (
        <p className="value-field-wide">
          <label htmlFor={id('text')}>
            {t(value.kind === 'uri' ? 'field.uri' : 'field.resourceId')}
          </label>
          <input
            id={id('text')}
            type="text"
            aria-label={t(value.kind === 'uri' ? 'valueField.uri' : 'valueField.resourceId', named)}
            inputMode={value.kind === 'resource' ? 'numeric' : undefined}
            value={value.text}
            onChange={(event) => {
              patch({ text: event.target.value });
            }}
          />
        </p>
      )}
      {value.kind === 'uri' ? (
        <p>
          <label htmlFor={id('label')}>{t('field.label')}</label>
          <input
            id={id('label')}
            type="text"
            aria-label={t('valueField.uriLabel', named)}
            value={value.label}
            onChange={(event) => {
              patch({ label: event.target.value });
            }}
          />
        </p>
      ) : null}
      {value.kind === 'resource' ? null : (
        <p className="value-lang-field">
          <label htmlFor={id('lang')}>{t('field.language')}</label>
          <input
            id={id('lang')}
            type="text"
            aria-label={t('valueField.language', named)}
            value={value.lang}
            onChange={(event) => {
              patch({ lang: event.target.value });
            }}
          />
        </p>
      )}
      <p>
        <label htmlFor={id('public')}>
          <input
            id={id('public')}
            type="checkbox"
            aria-label={t('valueField.public', named)}
            checked={value.isPublic}
            onChange={(event) => {
              patch({ isPublic: event.target.checked });
            }}
          />
          {t('field.public')}
        </label>
      </p>
      <p>
        <button
          type="button"
          aria-label={t('field.removeValue', { term: field.label })}
          onClick={() => {
            onChange(removeValue(draft, value.key));
          }}
        >
          {t('action.remove')}
        </button>
      </p>
    </li>
  );
};

/** The lessons a field group is the anchor of. */
interface FieldLessons {
  readonly required: boolean;
  readonly local: boolean;
}

const NO_LESSONS: FieldLessons = { required: false, local: false };

/** The first required field and the first local one anchor their lessons. */
const fieldLessons = (fields: readonly PropertyField[]): ((field: PropertyField) => FieldLessons) => {
  const required = fields.find((field) => field.rule?.isRequired === true);
  const local = fields.find((field) => isLocalTerm(field.term));
  return (field) => ({ required: field === required, local: field === local });
};

interface FieldGroupProps extends Change {
  readonly idPrefix: string;
  readonly field: PropertyField;
  readonly dataTypes: readonly DataType[];
  readonly lessons: FieldLessons;
}

const FieldGroup = ({
  idPrefix,
  field,
  dataTypes,
  draft,
  onChange,
  lessons,
}: FieldGroupProps): ReactElement => {
  const { t } = useTranslator();
  const labelId = `${idPrefix}-field-${String(field.propertyId)}-label`;
  const rule = field.rule;
  const required = useLesson('required', lessons.required);
  const local = useLesson('local-field', lessons.local);
  return (
    <li className="property-field">
      <p className="property-field-head">
        <span id={labelId} className="property-field-label">
          {field.label}
        </span>
        {field.label === field.term ? null : (
          <span className="property-field-term">{field.term}</span>
        )}
        <TermHelp entry={field.property} />
        {local.toggle}
        {rule?.isRequired === true ? (
          <span className="field-badge field-badge-required">{t('valueField.required')}</span>
        ) : null}
        {required.toggle}
        {rule?.isPrivate === true ? (
          <span className="field-badge" title={t('valueField.privateHint')}>
            {t('valueField.private')}
          </span>
        ) : null}
      </p>
      {local.panel}
      {required.panel}
      {field.comment === null ? null : <p className="property-field-comment">{field.comment}</p>}
      {field.values.length === 0 ? null : (
        <ul className="value-rows">
          {field.values.map((value) => (
            <ValueRow
              key={value.key}
              idPrefix={idPrefix}
              field={field}
              value={value}
              labelId={labelId}
              dataTypes={dataTypes}
              draft={draft}
              onChange={onChange}
            />
          ))}
        </ul>
      )}
      <p>
        <button
          type="button"
          aria-label={t('valueField.addValue', { field: field.label })}
          onClick={() => {
            onChange(addFieldValue(draft, field));
          }}
        >
          {t('field.addValue')}
        </button>
      </p>
    </li>
  );
};

interface AddFieldProps extends Change {
  readonly properties: readonly Property[];
  readonly createField: ((name: string) => Promise<Property>) | undefined;
}

/** "Add another field": a plain-words search over every property. */
const AddField = ({ properties, draft, onChange, createField }: AddFieldProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <div className="add-field">
      <StandardTermPicker
        label={t('valueField.addField')}
        placeholder={t('valueField.addFieldHint')}
        entries={properties}
        onPick={(property) => {
          onChange(addPropertyValue(draft, property));
        }}
        onCreate={
          createField === undefined
            ? undefined
            : async (name) => {
                onChange(addPropertyValue(draft, await createField(name)));
              }
        }
      />
    </div>
  );
};

export interface ValuesEditorProps extends Change {
  readonly idPrefix: string;
  readonly template: TemplateLayout | null;
  readonly properties: readonly Property[];
  readonly dataTypes: readonly DataType[];
  /** Makes a new property for "Add another field"; without it none is offered. */
  readonly createField?: (name: string, properties: readonly Property[]) => Promise<Property>;
  /** Anchor the item form's lessons here; a preview of the form shows none. */
  readonly showLessons?: boolean;
}

export const ValuesEditor = ({
  idPrefix,
  draft,
  onChange,
  template,
  properties: loaded,
  dataTypes,
  createField,
  showLessons = false,
}: ValuesEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [created, setCreated] = useState<readonly Property[]>([]);
  const properties = [...loaded, ...created.filter((entry) => !loaded.some((p) => p.id === entry.id))];
  const fields = layoutFields(draft.values, template, properties);
  const lessonsOf = showLessons ? fieldLessons(fields) : () => NO_LESSONS;
  return (
    <fieldset className="values-editor">
      <legend>{t('field.valuesLegend')}</legend>
      <ul className="property-fields">
        {fields.map((field) => (
          <FieldGroup
            key={String(field.propertyId)}
            idPrefix={idPrefix}
            field={field}
            dataTypes={dataTypes}
            draft={draft}
            onChange={onChange}
            lessons={lessonsOf(field)}
          />
        ))}
      </ul>
      <AddField
        properties={properties}
        draft={draft}
        onChange={onChange}
        createField={
          createField === undefined
            ? undefined
            : async (name) => {
                const property = await createField(name, properties);
                setCreated((current) => [...current, property]);
                return property;
              }
        }
      />
    </fieldset>
  );
};
