import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { Property } from '../api/types';
import { useTranslator } from '../i18n';
import type { MessageKey } from '../i18n';
import {
  addValue,
  patchValue,
  removeValue,
  type ResourceDraft,
  type ValueKind,
} from './draft';
import { ErrorNotice } from './errors';
import type { FormOptions, Option } from './options';
import { classOptions, propertyById, selectedOption, templateOptions, userOptions } from './options';
import type { SaveState } from './save';

export interface OptionSelectProps<IdT extends number> {
  readonly id: string;
  readonly label: string;
  readonly options: readonly Option<IdT>[];
  readonly value: IdT | null;
  readonly onChange: (value: IdT | null) => void;
  readonly emptyLabel?: string;
}

export const OptionSelect = <IdT extends number>({
  id,
  label,
  options,
  value,
  onChange,
  emptyLabel,
}: OptionSelectProps<IdT>): ReactElement => {
  const { t } = useTranslator();
  return (
    <p>
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value === null ? '' : String(value)}
        onChange={(event) => {
          onChange(selectedOption(options, event.target.value));
        }}
      >
        <option value="">{emptyLabel ?? t('common.none')}</option>
        {options.map((option) => (
          <option key={String(option.id)} value={String(option.id)}>
            {option.label}
          </option>
        ))}
      </select>
    </p>
  );
};

export interface TextFieldProps {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly type?: 'text' | 'email';
}

export const TextField = ({ id, label, value, onChange, type }: TextFieldProps): ReactElement => (
  <p>
    <label htmlFor={id}>{label}</label>
    <input
      id={id}
      type={type ?? 'text'}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
      }}
    />
  </p>
);

export interface CheckboxFieldProps {
  readonly id: string;
  readonly label: string;
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
}

export const CheckboxField = ({
  id,
  label,
  checked,
  onChange,
}: CheckboxFieldProps): ReactElement => (
  <p>
    <label htmlFor={id}>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      {label}
    </label>
  </p>
);

export interface ResourceFieldsProps {
  readonly idPrefix: string;
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
  readonly options: FormOptions;
}

/** Template, class, owner and visibility, the shared columns of a write. */
export const ResourceFields = ({
  idPrefix,
  draft,
  onChange,
  options,
}: ResourceFieldsProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <fieldset>
      <legend>{t('field.resourceLegend')}</legend>
      <OptionSelect
        id={`${idPrefix}-template`}
        label={t('field.resourceTemplate')}
        options={templateOptions(options.templates)}
        value={draft.resourceTemplate}
        onChange={(value) => {
          onChange({ ...draft, resourceTemplate: value });
        }}
      />
      <OptionSelect
        id={`${idPrefix}-class`}
        label={t('field.class')}
        options={classOptions(options.classes)}
        value={draft.resourceClass}
        onChange={(value) => {
          onChange({ ...draft, resourceClass: value });
        }}
      />
      <OptionSelect
        id={`${idPrefix}-owner`}
        label={t('field.owner')}
        options={userOptions(options.users)}
        value={draft.owner}
        onChange={(value) => {
          onChange({ ...draft, owner: value });
        }}
        emptyLabel={t('common.noOwner')}
      />
      <CheckboxField
        id={`${idPrefix}-public`}
        label={t('field.public')}
        checked={draft.isPublic}
        onChange={(checked) => {
          onChange({ ...draft, isPublic: checked });
        }}
      />
    </fieldset>
  );
};

const VALUE_KINDS: readonly { readonly value: ValueKind; readonly labelKey: MessageKey }[] = [
  { value: 'literal', labelKey: 'field.kindText' },
  { value: 'uri', labelKey: 'field.kindUri' },
  { value: 'resource', labelKey: 'field.kindResource' },
];

const isValueKind = (value: string): value is ValueKind =>
  VALUE_KINDS.some((kind) => kind.value === value);

const valueFieldLabel = (kind: ValueKind): MessageKey => {
  switch (kind) {
    case 'literal':
      return 'field.value';
    case 'uri':
      return 'field.uri';
    case 'resource':
      return 'field.resourceId';
  }
};

export interface ValueRowProps {
  readonly idPrefix: string;
  readonly draft: ResourceDraft;
  readonly value: ResourceDraft['values'][number];
  readonly properties: readonly Property[];
  readonly onChange: (draft: ResourceDraft) => void;
}

const ValueRow = ({ idPrefix, draft, value, properties, onChange }: ValueRowProps): ReactElement => {
  const { t } = useTranslator();
  const row = (suffix: string): string => `${idPrefix}-${value.key}-${suffix}`;
  // A stored value can name a property outside the fetched page, so the row
  // keeps its own option rather than silently switching properties.
  const options = propertyById(properties, value.propertyId) === null
    ? [{ id: value.propertyId, label: `${value.term}${t('field.currentSuffix')}` }, ...properties.map((property) => ({ id: property.id, label: property.term }))]
    : properties.map((property) => ({ id: property.id, label: property.term }));
  return (
    <li className="value-row">
      <p>
        <label htmlFor={row('property')}>{t('field.property')}</label>
        <select
          id={row('property')}
          value={String(value.propertyId)}
          onChange={(event) => {
            const property = propertyById(properties, Number(event.target.value));
            if (property !== null) {
              onChange(patchValue(draft, value.key, { propertyId: property.id, term: property.term }));
            }
          }}
        >
          {options.map((option) => (
            <option key={String(option.id)} value={String(option.id)}>
              {option.label}
            </option>
          ))}
        </select>
      </p>
      <p>
        <label htmlFor={row('type')}>{t('field.type')}</label>
        <select
          id={row('type')}
          value={value.kind}
          onChange={(event) => {
            if (isValueKind(event.target.value)) {
              onChange(patchValue(draft, value.key, { kind: event.target.value }));
            }
          }}
        >
          {VALUE_KINDS.map((kind) => (
            <option key={kind.value} value={kind.value}>
              {t(kind.labelKey)}
            </option>
          ))}
        </select>
      </p>
      <p className="value-field-wide">
        <label htmlFor={row('text')}>{t(valueFieldLabel(value.kind))}</label>
        <input
          id={row('text')}
          type="text"
          inputMode={value.kind === 'resource' ? 'numeric' : undefined}
          value={value.text}
          onChange={(event) => {
            onChange(patchValue(draft, value.key, { text: event.target.value }));
          }}
        />
      </p>
      {value.kind === 'uri' ? (
        <p>
          <label htmlFor={row('label')}>{t('field.label')}</label>
          <input
            id={row('label')}
            type="text"
            value={value.label}
            onChange={(event) => {
              onChange(patchValue(draft, value.key, { label: event.target.value }));
            }}
          />
        </p>
      ) : null}
      {value.kind === 'resource' ? null : (
        <p>
          <label htmlFor={row('lang')}>{t('field.language')}</label>
          <input
            id={row('lang')}
            type="text"
            value={value.lang}
            onChange={(event) => {
              onChange(patchValue(draft, value.key, { lang: event.target.value }));
            }}
          />
        </p>
      )}
      <p>
        <label htmlFor={row('public')}>
          <input
            id={row('public')}
            type="checkbox"
            checked={value.isPublic}
            onChange={(event) => {
              onChange(patchValue(draft, value.key, { isPublic: event.target.checked }));
            }}
          />
          {t('field.public')}
        </label>
      </p>
      <p>
        <button
          type="button"
          aria-label={t('field.removeValue', { term: value.term })}
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

export interface ValuesEditorProps {
  readonly idPrefix: string;
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
  readonly properties: readonly Property[];
  /** Terms a dedicated field owns, so the editor does not duplicate them. */
  readonly excludeTerms: readonly string[];
}

/** The property/value rows of one resource, with add and remove. */
export const ValuesEditor = ({
  idPrefix,
  draft,
  onChange,
  properties,
  excludeTerms,
}: ValuesEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [pending, setPending] = useState('');
  const rows = draft.values.filter((value) => !excludeTerms.includes(value.term));
  return (
    <fieldset>
      <legend>{t('field.valuesLegend')}</legend>
      <ul className="value-rows">
        {rows.map((value) => (
          <ValueRow
            key={value.key}
            idPrefix={idPrefix}
            draft={draft}
            value={value}
            properties={properties}
            onChange={onChange}
          />
        ))}
      </ul>
      <p>
        <label htmlFor={`${idPrefix}-add-property`}>{t('field.addProperty')}</label>
        <select
          id={`${idPrefix}-add-property`}
          value={pending}
          onChange={(event) => {
            setPending(event.target.value);
          }}
        >
          <option value="">{t('field.selectProperty')}</option>
          {properties.map((property) => (
            <option key={String(property.id)} value={String(property.id)}>
              {property.term}
            </option>
          ))}
        </select>
        <button
          type="button"
          disabled={pending === ''}
          onClick={() => {
            const property = propertyById(properties, Number(pending));
            if (property !== null) {
              onChange(addValue(draft, property));
              setPending('');
            }
          }}
        >
          {t('field.addValue')}
        </button>
      </p>
    </fieldset>
  );
};

export const SaveStatus = ({ state }: { readonly state: SaveState }): ReactElement | null => {
  const { t } = useTranslator();
  switch (state.status) {
    case 'idle':
      return null;
    case 'saving':
      return <p role="status">{t('field.saving')}</p>;
    case 'saved':
      return <p role="status">{t('field.saved')}</p>;
    case 'error':
      return <ErrorNotice error={state.error} />;
  }
};

export interface SubmitRowProps {
  readonly label: string;
  readonly busy: boolean;
}

export const SubmitRow = ({ label, busy }: SubmitRowProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <p className="form-actions">
      <button type="submit" disabled={busy}>
        {busy ? t('field.saving') : label}
      </button>
    </p>
  );
};

export interface EditPanelProps {
  readonly label: string;
  readonly children: ReactNode;
}

/**
 * The edit form on a show page. Closed by default, and the closed state renders
 * no form, so the record above it stays the only copy on the page.
 */
export const EditPanel = ({ label, children }: EditPanelProps): ReactElement => {
  const [open, setOpen] = useState(false);
  return (
    <section className="edit-panel">
      <button
        type="button"
        className="edit-toggle"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        {label}
      </button>
      {open ? children : null}
    </section>
  );
};
