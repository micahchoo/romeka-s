import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { useTranslator } from '../i18n';
import type { ResourceDraft } from './draft';
import { ErrorNotice } from './errors';
import type { FormOptions, Option } from './options';
import { classOptions, selectedOption, templateOptions, userOptions } from './options';
import type { SaveState } from './save';
import { templateById, withTemplate } from './value-fields';

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

/**
 * Template, class, owner and visibility, the shared columns of a write. Picking
 * a template lays its rows into the values and may set the class.
 */
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
          onChange(
            withTemplate(draft, templateById(options.templates, value), options.properties),
          );
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
