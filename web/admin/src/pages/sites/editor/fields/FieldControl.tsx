/**
 * The one place a schema field becomes a control. `BlockForm` and
 * `LayoutDataForm` both render through here, so a layout's `data` and its
 * `layout_data` cannot drift apart in what they offer.
 *
 * Each kind is its own component, so the switch below is a table of one line
 * per kind and every control stays small. The `attachments` kind is the
 * exception: it lives outside `o:data` and needs the block's `o:attachment`
 * rows, so `BlockForm` renders it and this returns `null` for it.
 *
 * A field whose `when` test fails renders nothing. Every control shows the
 * field's help line, so the schema is the one place a field explains itself.
 * A required field says it is empty only after the user has left it once.
 */
import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import { useEditorText } from '../messages';
import type { BlockField } from '../../blockSchema';
import type { BlockKey, ValidationIssue } from '../draft';
import type { EditorOptions } from '../options';
import { Checked, Help, Labelled, helpId } from '../Labelled';
import { AssetPicker } from './AssetPicker';
import { ColorField } from './ColorField';
import { issueFor, issueMessage } from './errors';
import { HtmlEditor } from './HtmlEditor';
import { ItemQueryField, isQueryResource } from './ItemQueryField';
import { LinkField } from './LinkField';
import { PageListField } from './PageListField';
import { PageSelect } from './PageSelect';
import { RepeatField } from './RepeatField';
import type { RenderControl } from './RepeatField';
import { SidesField } from './SidesField';
import { isShown } from './visibility';
import { editor } from '../../../../i18n/namespaces/editor';

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
  /** The values beside this one, which `link`, `sides` and `itemQuery` read. */
  readonly record: Readonly<Record<string, JsonValue>>;
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
}

/** A control that renders its own label, followed by the field's help line. */
const WithHelp = ({
  id,
  help,
  children,
}: {
  readonly id: string;
  readonly help: string;
  readonly children: ReactNode;
}): ReactElement => (
  <div className="block-field-help">
    {children}
    <Help id={id} text={help} />
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
  const e = useEditorText();
  return (
    <Labelled id={id} label={e(field.label)} help={e(field.help)} error={error}>
      <input
        id={id}
        type="text"
        value={asText(value)}
        required={field.required}
        aria-invalid={invalid}
        aria-describedby={helpId(id)}
        {...(field.placeholder === null ? {} : { placeholder: e(field.placeholder) })}
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
  const e = useEditorText();
  return (
    <WithHelp id={id} help={e(field.help)}>
      <HtmlEditor
        id={id}
        label={e(field.label)}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
      />
      {error}
    </WithHelp>
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
  const e = useEditorText();
  return (
    <Labelled id={id} label={e(field.label)} help={e(field.help)} error={error}>
      <input
        id={id}
        type="number"
        value={asText(value)}
        required={field.required}
        aria-invalid={invalid}
        aria-describedby={helpId(id)}
        {...(typeof field.default === 'number' ? { placeholder: String(field.default) } : {})}
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
  const e = useEditorText();
  return (
    <Checked
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      checked={value === true}
      onChange={onChange}
    />
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
  const { t } = useTranslator(editor);
  const e = useEditorText();
  if (field.multiple) {
    const selected = asList(value);
    return (
      <fieldset className="block-field" aria-describedby={helpId(id)}>
        <legend>{e(field.label)}</legend>
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
            {e(option.label)}
          </label>
        ))}
        <Help id={id} text={e(field.help)} />
        {error}
      </fieldset>
    );
  }
  // A block stored without the key renders the schema default, so show that.
  const current = asText(value ?? (typeof field.default === 'string' ? field.default : undefined));
  const named = field.options.some((option) => option.value === current);
  // "Not set" is a state only a field with no default and no empty choice of
  // its own can be in; a default stands in for an empty value, so it never is.
  const unsettable =
    !field.required &&
    field.default === null &&
    !field.options.some((option) => option.value === '');
  return (
    <Labelled id={id} label={e(field.label)} help={e(field.help)} error={error}>
      <select
        id={id}
        value={current}
        required={field.required}
        aria-invalid={invalid}
        aria-describedby={helpId(id)}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {current === '' || named ? null : (
          <option value={current}>{t('field.optionFallback', { value: current })}</option>
        )}
        {unsettable ? <option value="">{e('choice.unset')}</option> : null}
        {field.options.map((option) => (
          <option key={option.value} value={option.value}>
            {e(option.label)}
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
  const e = useEditorText();
  return (
    <WithHelp id={id} help={e(field.help)}>
      <PageSelect
        id={id}
        label={e(field.label)}
        siteId={siteId}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
        {...(options?.pages === undefined ? {} : { pages: options.pages })}
      />
      {error}
    </WithHelp>
  );
};

const AssetRefControl = ({
  field,
  id,
  value,
  onChange,
  error,
}: ControlProps & { readonly field: FieldOf<'assetRef'> }): ReactElement => {
  const e = useEditorText();
  return (
    <>
      <AssetPicker
        id={id}
        label={e(field.label)}
        description={e(field.help)}
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
  siteId,
  options,
  record,
}: ControlProps & { readonly field: FieldOf<'itemQuery'> }): ReactElement => {
  const e = useEditorText();
  const resource = field.resourceKey === null ? 'items' : record[field.resourceKey];
  return (
    <WithHelp id={id} help={e(field.help)}>
      <ItemQueryField
        id={id}
        label={e(field.label)}
        value={asText(value)}
        required={field.required}
        onChange={onChange}
        siteId={siteId}
        resource={isQueryResource(resource) ? resource : 'items'}
        {...(options?.itemSets === undefined ? {} : { itemSets: options.itemSets })}
        {...(options?.classes === undefined ? {} : { classes: options.classes })}
      />
      {error}
    </WithHelp>
  );
};

const RepeatControl = (props: ControlProps & { readonly field: FieldOf<'repeat'> }): ReactElement => {
  const e = useEditorText();
  const { field, value, onChange, issues, blockKey, siteId, idPrefix, path, error, options } = props;
  const entries = Array.isArray(value) ? value : [];
  const renderControl: RenderControl = (nested) => (
    <FieldControl
      key={nested.path}
      {...nested}
      issues={issues}
      blockKey={blockKey}
      siteId={siteId}
      idPrefix={idPrefix}
      {...(options === undefined ? {} : { options })}
    />
  );
  return (
    <WithHelp id={props.id} help={e(field.help)}>
      <RepeatField
        label={e(field.label)}
        fields={field.fields}
        value={entries}
        onChange={onChange}
        renderControl={renderControl}
        path={path}
      />
      {error}
    </WithHelp>
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
  const e = useEditorText();
  return (
    <WithHelp id={id} help={e(field.help)}>
      <PageListField
        id={id}
        label={e(field.label)}
        value={asText(value)}
        siteId={siteId}
        {...(options?.pages === undefined ? {} : { pages: options.pages })}
        onChange={onChange}
      />
      {error}
    </WithHelp>
  );
};

const ColorControl = ({
  field,
  id,
  value,
  onChange,
}: ControlProps & { readonly field: FieldOf<'color'> }): ReactElement => {
  const e = useEditorText();
  return (
    <ColorField
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      value={asText(value)}
      onChange={onChange}
    />
  );
};

const SidesControl = ({
  field,
  id,
  record,
  onPatch,
}: ControlProps & { readonly field: FieldOf<'sides'> }): ReactElement => {
  const e = useEditorText();
  return (
    <SidesField
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      keys={field.keys}
      record={record}
      onPatch={onPatch}
    />
  );
};

const LinkControl = ({
  field,
  id,
  record,
  onPatch,
  siteId,
  options,
}: ControlProps & { readonly field: FieldOf<'link'> }): ReactElement => {
  const e = useEditorText();
  return (
    <LinkField
      id={id}
      label={e(field.label)}
      help={e(field.help)}
      pageKey={field.key}
      urlKey={field.urlKey}
      record={record}
      onPatch={onPatch}
      siteId={siteId}
      {...(options?.pages === undefined ? {} : { pages: options.pages })}
    />
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
  /** The stored values of this field's list, which a `when` test and the
   * multi-key controls read. */
  readonly record: Readonly<Record<string, JsonValue>>;
  /** The fields of the same list, whose defaults a `when` test falls back to. */
  readonly siblings: readonly BlockField[];
  /** Writes several keys of the list as one edit. */
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
}

const control = (common: ControlProps & FieldControlProps): ReactElement | null => {
  switch (common.field.type) {
    case 'text':
      return <TextControl {...common} field={common.field} />;
    case 'html':
      return <HtmlControl {...common} field={common.field} />;
    case 'integer':
      return <IntegerControl {...common} field={common.field} />;
    case 'boolean':
      return <BooleanControl {...common} field={common.field} />;
    case 'choice':
      return <ChoiceControl {...common} field={common.field} />;
    case 'pageRef':
      return <PageRefControl {...common} field={common.field} />;
    case 'assetRef':
      return <AssetRefControl {...common} field={common.field} />;
    case 'itemQuery':
      return <ItemQueryControl {...common} field={common.field} />;
    case 'repeat':
      return <RepeatControl {...common} field={common.field} />;
    case 'pageList':
      return <PageListControl {...common} field={common.field} />;
    case 'color':
      return <ColorControl {...common} field={common.field} />;
    case 'sides':
      return <SidesControl {...common} field={common.field} />;
    case 'link':
      return <LinkControl {...common} field={common.field} />;
    case 'attachments':
      return null;
  }
};

export const FieldControl = (props: FieldControlProps): ReactElement | null => {
  const { t } = useTranslator(editor);
  // A new block starts with its required fields empty, so "required" waits
  // until the user has been in the field and left it.
  const [visited, setVisited] = useState(false);
  if (!isShown(props.field, props.record, props.siblings)) {
    return null;
  }
  const id = `${props.idPrefix}-${props.path}`;
  const found = issueFor(props.issues, props.blockKey, props.path);
  const issue = found?.kind === 'required' && !visited ? null : found;
  const message = issue === null ? null : issueMessage(issue);
  const error =
    message === null ? null : (
      <p className="field-error" role="alert">
        {t(message.key, message.params)}
      </p>
    );
  const rendered = control({ ...props, id, error, invalid: issue !== null });
  if (!props.field.required) {
    return rendered;
  }
  return (
    <div
      className="field-visit"
      onBlur={() => {
        setVisited(true);
      }}
    >
      {rendered}
    </div>
  );
};
