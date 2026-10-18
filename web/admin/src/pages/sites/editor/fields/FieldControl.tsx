/**
 * The one place a schema field becomes a control. `BlockForm` and
 * `LayoutDataForm` both render through here, so a layout's `data` and its
 * `layout_data` cannot drift apart in what they offer.
 *
 * Each kind is its own component, so the switch below is a table of one line
 * per kind and every control stays small. The `attachments` kind is the
 * exception: it lives outside `o:data` and needs the block's `o:attachment`
 * rows, so `BlockForm` renders it and this returns `null` for it.
 */
import type { ReactElement, ReactNode } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import type { BlockField } from '../../blockSchema';
import type { BlockKey, ValidationIssue } from '../draft';
import type { EditorOptions } from '../options';
import { AssetPicker } from './AssetPicker';
import { issueFor, issueMessage } from './errors';
import { HtmlEditor } from './HtmlEditor';
import { ItemQueryField } from './ItemQueryField';
import { PageListField } from './PageListField';
import { PageSelect } from './PageSelect';
import { RepeatField } from './RepeatField';
import type { RenderControl } from './RepeatField';

type FieldOf<Type extends BlockField['type']> = Extract<BlockField, { type: Type }>;

/** A stored scalar as the string a text control shows. */
const asText = (value: JsonValue | undefined): string => {
  if (typeof value === 'string') {
    return value;
  }
  return typeof value === 'number' || typeof value === 'boolean' ? String(value) : '';
};

/** The values of a multiple choice, keeping only the strings a select can hold. */
const asList = (value: JsonValue | undefined): readonly string[] =>
  Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === 'string') : [];

/** The props every control of one field shares. */
interface ControlProps {
  readonly id: string;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
  readonly error: ReactNode;
  readonly invalid: boolean;
  readonly siteId: SiteId;
  readonly issues: readonly ValidationIssue[];
  readonly blockKey: BlockKey | null;
  readonly idPrefix: string;
  readonly path: string;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
}

/** A label, its control and an error under it, the shape every scalar field has. */
const Labelled = ({
  id,
  label,
  error,
  children,
}: {
  readonly id: string;
  readonly label: string;
  readonly error: ReactNode;
  readonly children: ReactNode;
}): ReactElement => (
  <div className="block-field">
    <label htmlFor={id}>{label}</label>
    {children}
    {error}
  </div>
);

const TextControl = ({
  field,
  id,
  value,
  onChange,
  error,
  invalid,
}: ControlProps & { readonly field: FieldOf<'text'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <Labelled id={id} label={t(field.label)} error={error}>
      <input
        id={id}
        type="text"
        value={asText(value)}
        required={field.required}
        aria-invalid={invalid}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
    </Labelled>
  );
};

const HtmlControl = ({
  field,
  id,
  value,
  onChange,
  error,
}: ControlProps & { readonly field: FieldOf<'html'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <>
      <HtmlEditor
        id={id}
        label={t(field.label)}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
      />
      {error}
    </>
  );
};

const IntegerControl = ({
  field,
  id,
  value,
  onChange,
  error,
  invalid,
}: ControlProps & { readonly field: FieldOf<'integer'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <Labelled id={id} label={t(field.label)} error={error}>
      <input
        id={id}
        type="number"
        value={asText(value)}
        required={field.required}
        aria-invalid={invalid}
        {...(field.min === null ? {} : { min: field.min })}
        {...(field.max === null ? {} : { max: field.max })}
        onChange={(event) => {
          const raw = event.target.value;
          if (raw.trim() === '') {
            onChange(null);
            return;
          }
          const number = Number(raw);
          onChange(Number.isInteger(number) ? number : raw);
        }}
      />
    </Labelled>
  );
};

const BooleanControl = ({
  field,
  id,
  value,
  onChange,
}: ControlProps & { readonly field: FieldOf<'boolean'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <div className="block-field">
      <label htmlFor={id}>
        <input
          id={id}
          type="checkbox"
          checked={value === true}
          onChange={(event) => {
            onChange(event.target.checked);
          }}
        />
        {t(field.label)}
      </label>
    </div>
  );
};

const ChoiceControl = ({
  field,
  id,
  value,
  onChange,
  error,
  invalid,
}: ControlProps & { readonly field: FieldOf<'choice'> }): ReactElement => {
  const { t } = useTranslator();
  if (field.multiple) {
    const selected = asList(value);
    return (
      <fieldset className="block-field">
        <legend>{t(field.label)}</legend>
        {field.options.map((option) => (
          <label key={option.value}>
            <input
              type="checkbox"
              value={option.value}
              checked={selected.includes(option.value)}
              onChange={(event) => {
                onChange(
                  event.target.checked
                    ? [...selected, option.value]
                    : selected.filter((entry) => entry !== option.value),
                );
              }}
            />
            {t(option.label)}
          </label>
        ))}
        {error}
      </fieldset>
    );
  }
  const current = asText(value);
  const named = field.options.some((option) => option.value === current);
  const empty = field.options.some((option) => option.value === '');
  return (
    <Labelled id={id} label={t(field.label)} error={error}>
      <select
        id={id}
        value={current}
        required={field.required}
        aria-invalid={invalid}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {current === '' || named ? null : (
          <option value={current}>{t('field.optionFallback', { value: current })}</option>
        )}
        {empty || field.required ? null : <option value="">{t('field.selectOption')}</option>}
        {field.options.map((option) => (
          <option key={option.value} value={option.value}>
            {t(option.label)}
          </option>
        ))}
      </select>
    </Labelled>
  );
};

const PageRefControl = ({
  field,
  id,
  value,
  onChange,
  error,
  siteId,
  options,
}: ControlProps & { readonly field: FieldOf<'pageRef'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <>
      <PageSelect
        id={id}
        label={t(field.label)}
        siteId={siteId}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
        {...(options?.pages === undefined ? {} : { pages: options.pages })}
      />
      {error}
    </>
  );
};

const AssetRefControl = ({
  field,
  id,
  value,
  onChange,
  error,
}: ControlProps & { readonly field: FieldOf<'assetRef'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <>
      <AssetPicker
        id={id}
        label={t(field.label)}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
      />
      {error}
    </>
  );
};

const ItemQueryControl = ({
  field,
  id,
  value,
  onChange,
  error,
  options,
}: ControlProps & { readonly field: FieldOf<'itemQuery'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <>
      <ItemQueryField
        id={id}
        label={t(field.label)}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
        {...(options?.itemSets === undefined ? {} : { itemSets: options.itemSets })}
        {...(options?.classes === undefined ? {} : { classes: options.classes })}
      />
      {error}
    </>
  );
};

const RepeatControl = (props: ControlProps & { readonly field: FieldOf<'repeat'> }): ReactElement => {
  const { t } = useTranslator();
  const { field, value, onChange, issues, blockKey, siteId, idPrefix, path, error, options } = props;
  const entries = Array.isArray(value) ? value : [];
  const renderControl: RenderControl = (nested) => (
    <FieldControl
      {...nested}
      issues={issues}
      blockKey={blockKey}
      siteId={siteId}
      idPrefix={idPrefix}
      {...(options === undefined ? {} : { options })}
    />
  );
  return (
    <>
      <RepeatField
        label={t(field.label)}
        fields={field.fields}
        value={entries}
        onChange={onChange}
        renderControl={renderControl}
        path={path}
      />
      {error}
    </>
  );
};

const PageListControl = ({
  field,
  id,
  value,
  onChange,
  error,
  siteId,
  options,
}: ControlProps & { readonly field: FieldOf<'pageList'> }): ReactElement => {
  const { t } = useTranslator();
  return (
    <>
      <PageListField
        id={id}
        label={t(field.label)}
        value={asText(value)}
        siteId={siteId}
        {...(options?.pages === undefined ? {} : { pages: options.pages })}
        onChange={onChange}
      />
      {error}
    </>
  );
};

export interface FieldControlProps {
  readonly field: BlockField;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
  /** The dotted `o:data` path, also the tail of every DOM id the control uses. */
  readonly path: string;
  readonly issues: readonly ValidationIssue[];
  /** The block the issues belong to; `null` for page-level or layout-data issues. */
  readonly blockKey: BlockKey | null;
  readonly siteId: SiteId;
  readonly idPrefix: string;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
}

export const FieldControl = (props: FieldControlProps): ReactElement | null => {
  const { t } = useTranslator();
  const id = `${props.idPrefix}-${props.path}`;
  const issue = issueFor(props.issues, props.blockKey, props.path);
  const message = issue === null ? null : issueMessage(issue);
  const error =
    message === null ? null : (
      <p className="field-error" role="alert">
        {t(message.key, message.params)}
      </p>
    );
  const common = {
    ...props,
    id,
    error,
    invalid: issue !== null,
  };
  switch (props.field.type) {
    case 'text':
      return <TextControl {...common} field={props.field} />;
    case 'html':
      return <HtmlControl {...common} field={props.field} />;
    case 'integer':
      return <IntegerControl {...common} field={props.field} />;
    case 'boolean':
      return <BooleanControl {...common} field={props.field} />;
    case 'choice':
      return <ChoiceControl {...common} field={props.field} />;
    case 'pageRef':
      return <PageRefControl {...common} field={props.field} />;
    case 'assetRef':
      return <AssetRefControl {...common} field={props.field} />;
    case 'itemQuery':
      return <ItemQueryControl {...common} field={props.field} />;
    case 'repeat':
      return <RepeatControl {...common} field={props.field} />;
    case 'pageList':
      return <PageListControl {...common} field={props.field} />;
    case 'attachments':
      return null;
  }
};
