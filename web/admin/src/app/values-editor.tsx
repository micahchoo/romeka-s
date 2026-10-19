/**
 * The values of one resource, one group per property, laid out by the chosen
 * template (`value-fields.ts` owns the layout and the rules).
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import type { DataType, Property, ResourceTemplate } from '../api/types';
import { useTranslator } from '../i18n';
import type { Translator } from '../i18n';
import { patchValue, removeValue } from './draft';
import type { ResourceDraft, ValueDraft } from './draft';
import {
  addFieldValue,
  addPropertyValue,
  layoutFields,
  matchProperties,
  setValueType,
  typeChoices,
  type PropertyField,
} from './value-fields';

/** How many matches "Add another field" lists before asking for a longer query. */
const MATCH_LIMIT = 12;

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
          <label htmlFor={id('type')}>{t('field.type')}</label>
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

interface FieldGroupProps extends Change {
  readonly idPrefix: string;
  readonly field: PropertyField;
  readonly dataTypes: readonly DataType[];
}

const FieldGroup = ({ idPrefix, field, dataTypes, draft, onChange }: FieldGroupProps): ReactElement => {
  const { t } = useTranslator();
  const labelId = `${idPrefix}-field-${String(field.propertyId)}-label`;
  const rule = field.rule;
  return (
    <li className="property-field">
      <p className="property-field-head">
        <span id={labelId} className="property-field-label">
          {field.label}
        </span>
        {field.label === field.term ? null : (
          <span className="property-field-term">{field.term}</span>
        )}
        {rule?.isRequired === true ? (
          <span className="field-badge field-badge-required">{t('valueField.required')}</span>
        ) : null}
        {rule?.isPrivate === true ? (
          <span className="field-badge" title={t('valueField.privateHint')}>
            {t('valueField.private')}
          </span>
        ) : null}
      </p>
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
}

/** "Add another field": a search over property labels and terms. */
const AddField = ({ properties, draft, onChange }: AddFieldProps): ReactElement => {
  const { t } = useTranslator();
  const inputId = useId();
  const [query, setQuery] = useState('');
  const matches = matchProperties(properties, query);
  const shown = matches.slice(0, MATCH_LIMIT);
  return (
    <div className="add-field">
      <p>
        <label htmlFor={inputId}>{t('valueField.addField')}</label>
        <input
          id={inputId}
          type="search"
          placeholder={t('valueField.addFieldHint')}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
      </p>
      {query.trim() === '' ? null : matches.length === 0 ? (
        <p className="add-field-empty">{t('valueField.noMatch', { query: query.trim() })}</p>
      ) : (
        <ul className="add-field-matches" aria-label={t('valueField.matches')}>
          {shown.map((property) => (
            <li key={String(property.id)}>
              <button
                type="button"
                onClick={() => {
                  onChange(addPropertyValue(draft, property));
                  setQuery('');
                }}
              >
                <span className="property-field-label">{property.label ?? property.term}</span>{' '}
                <span className="property-field-term">{property.term}</span>
              </button>
            </li>
          ))}
          {matches.length > shown.length ? (
            <li className="add-field-more">
              {t('valueField.moreMatches', { count: matches.length - shown.length })}
            </li>
          ) : null}
        </ul>
      )}
    </div>
  );
};

export interface ValuesEditorProps extends Change {
  readonly idPrefix: string;
  readonly template: ResourceTemplate | null;
  readonly properties: readonly Property[];
  readonly dataTypes: readonly DataType[];
}

export const ValuesEditor = ({
  idPrefix,
  draft,
  onChange,
  template,
  properties,
  dataTypes,
}: ValuesEditorProps): ReactElement => {
  const { t } = useTranslator();
  const fields = layoutFields(draft.values, template, properties);
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
          />
        ))}
      </ul>
      <AddField properties={properties} draft={draft} onChange={onChange} />
    </fieldset>
  );
};
