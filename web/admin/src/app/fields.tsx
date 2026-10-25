import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { User } from '../api/types';
import { useTranslator } from '../i18n';
import { useSession } from './auth';
import { useTermLabels } from '../i18n/termText';
import { useDescribeText } from './describe-messages';
import type { ResourceDraft } from './draft';
import { ErrorNotice } from './errors';
import { useLesson } from './lessons/lesson';
import type { FormOptions, Option } from './options';
import { classOptions, selectedOption, templateOptions, userOptions } from './options';
import { plainName } from './plain-terms';
import type { SaveState } from './save';
import { typesFor, type ResourceKind } from './type-use';
import { templateById, withTemplate } from './value-fields';
import { common } from '../i18n/namespaces/common';

export interface OptionSelectProps<IdT extends number> {
  readonly id: string;
  readonly label: string;
  readonly options: readonly Option<IdT>[];
  readonly value: IdT | null;
  readonly onChange: (value: IdT | null) => void;
  readonly emptyLabel?: string;
  /** Shown after the select on its line, such as a lesson's "?". */
  readonly after?: ReactNode;
}

export const OptionSelect = <IdT extends number>({
  id,
  label,
  options,
  value,
  onChange,
  emptyLabel,
  after,
}: OptionSelectProps<IdT>): ReactElement => {
  const { t } = useTranslator(common);
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
      {after}
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
  /** Which form this is, so the Type picker offers the types used for it. */
  readonly kind: ResourceKind;
  readonly draft: ResourceDraft;
  readonly onChange: (draft: ResourceDraft) => void;
  readonly options: FormOptions;
  /** Whether the form edits a stored resource rather than making one. */
  readonly editing: boolean;
}

const OWNER_CHANGING_ROLES: readonly string[] = ['global_admin', 'site_admin'];

/**
 * Whether `user` may `change-owner`, which `AclFactory` grants the two admin
 * roles alone. `hydrateOwner` refuses the change to anyone else, so a form
 * offers an owner select only when this holds.
 */
export const mayChangeOwner = (user: User | null): boolean =>
  user !== null && OWNER_CHANGING_ROLES.includes(user.role);

/**
 * `common/resource-fields.phtml` shows the owner select only for a stored
 * resource the user may `change-owner`.
 */
const offersOwner = (editing: boolean, user: User | null): boolean =>
  editing && mayChangeOwner(user);

/**
 * Type, owner and visibility, then the class under "Advanced". Picking a type
 * lays its rows into the values and sets its class unless the user chose
 * another; the fold opens by itself whenever the class differs from the type's.
 */
export const ResourceFields = ({
  idPrefix,
  kind,
  draft,
  onChange,
  options,
  editing,
}: ResourceFieldsProps): ReactElement => {
  const { t } = useTranslator(common);
  const user = useSession();
  const describe = useDescribeText();
  const text = useTermLabels();
  const [open, setOpen] = useState(false);
  const selected = templateById(options.templates, draft.resourceTemplate);
  const typeClass = selected?.resourceClass?.id ?? null;
  const differs = selected === null ? draft.resourceClass !== null : draft.resourceClass !== typeClass;
  const shownClass = options.classes.find((entry) => entry.id === typeClass) ?? null;
  const lesson = useLesson('type-pick', kind === 'items');
  return (
    <fieldset>
      <legend>{t('field.resourceLegend')}</legend>
      <OptionSelect
        id={`${idPrefix}-template`}
        label={describe('type.label')}
        options={templateOptions(typesFor(options.templates, kind, draft.resourceTemplate))}
        value={draft.resourceTemplate}
        emptyLabel={describe('type.none')}
        onChange={(value) => {
          onChange(
            withTemplate(draft, templateById(options.templates, value), options.properties, selected),
          );
        }}
        after={lesson.toggle}
      />
      {lesson.panel}
      {shownClass === null ? null : (
        <p className="type-class-line">
          {describe('type.classLine', { name: plainName(shownClass, text) })}{' '}
          <code>{shownClass.term}</code>
        </p>
      )}
      {offersOwner(editing, user) ? (
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
      ) : null}
      <CheckboxField
        id={`${idPrefix}-public`}
        label={t('field.public')}
        checked={draft.isPublic}
        onChange={(checked) => {
          onChange({ ...draft, isPublic: checked });
        }}
      />
      <details
        className="advanced-fold"
        open={open || differs}
        onToggle={(event) => {
          setOpen(event.currentTarget.open);
        }}
      >
        <summary>{describe('advanced.summary')}</summary>
        {differs ? <p className="field-note">{describe('advanced.classDiffers')}</p> : null}
        <OptionSelect
          id={`${idPrefix}-class`}
          label={t('field.class')}
          options={classOptions(options.classes, text)}
          value={draft.resourceClass}
          onChange={(value) => {
            onChange({ ...draft, resourceClass: value });
          }}
        />
      </details>
    </fieldset>
  );
};

export const SaveStatus = ({ state }: { readonly state: SaveState }): ReactElement | null => {
  const { t } = useTranslator(common);
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
  const { t } = useTranslator(common);
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
