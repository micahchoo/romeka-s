/**
 * `Admin/ResourceTemplateController::addAction` and `::editAction`, over the
 * members `ResourceTemplateAdapter::hydrate` accepts.
 *
 * The property list is the row set the API reads: an entry per
 * `o:resource_template_property`, its position taken from its place in the list,
 * with an alternate label and comment, a data type list, the required and
 * private flags and a default language. A `PUT` replaces the stored rows with
 * the list sent, so the form always sends it.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { isTermListName } from '../../api/parse';
import type {
  ClassId,
  DataType,
  Property,
  PropertyId,
  ResourceClass,
  ResourceTemplate,
  ResourceTemplateWrite,
  TemplatePropertyWrite,
  User,
  UserId,
} from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { ErrorNotice } from '../../app/errors';
import { OptionSelect, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { classOptions, selectedOption, userOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';

export interface TemplateFormOptions {
  readonly users: readonly User[];
  readonly classes: readonly ResourceClass[];
  readonly properties: readonly Property[];
  readonly dataTypes: readonly DataType[];
}

const loadOptions = async (): Promise<TemplateFormOptions> => {
  const [users, classes, properties, dataTypes] = await Promise.all([
    allPages((page) => api.listUsers({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'name' })),
    allPages((page) =>
      api.listResourceClasses({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'label' }),
    ),
    allPages((page) => api.listProperties({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listDataTypes({ page, perPage: ALL_PAGES_PER_PAGE })),
  ]);
  return { users, classes, properties, dataTypes };
};

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

interface PropertyRowDraft {
  readonly key: string;
  readonly propertyId: PropertyId;
  readonly alternateLabel: string;
  readonly alternateComment: string;
  readonly dataTypes: readonly string[];
  readonly isRequired: boolean;
  readonly isPrivate: boolean;
  readonly defaultLang: string;
}

let nextRowKey = 0;

const rowKey = (): string => {
  nextRowKey += 1;
  return `template-row-${String(nextRowKey)}`;
};

const rowsFrom = (template: ResourceTemplate | null): readonly PropertyRowDraft[] =>
  (template?.properties ?? []).map((entry) => ({
    key: rowKey(),
    propertyId: entry.property.id,
    alternateLabel: entry.alternateLabel ?? '',
    alternateComment: entry.alternateComment ?? '',
    dataTypes: entry.dataTypes,
    isRequired: entry.isRequired,
    isPrivate: entry.isPrivate,
    defaultLang: entry.defaultLang ?? '',
  }));

const propertyTerm = (properties: readonly Property[], id: PropertyId): string =>
  properties.find((property) => property.id === id)?.term ?? `#${String(id)}`;

/** The ordered data type ids one row offers: the registered ones, then extras. */
const dataTypeIds = (options: TemplateFormOptions, row: PropertyRowDraft): readonly string[] => [
  ...options.dataTypes.map((dataType) => dataType.id),
  ...row.dataTypes.filter((id) => !options.dataTypes.some((dataType) => dataType.id === id)),
];

/**
 * The data types split the way the module's optgroup splits them: the core and
 * other types, then the term lists (`CustomVocab::getOptgroupLabel`). An empty
 * group is left out.
 */
const dataTypeGroups = (
  order: readonly string[],
): readonly { readonly key: 'core' | 'term-lists'; readonly ids: readonly string[] }[] =>
  [
    { key: 'core' as const, ids: order.filter((id) => !isTermListName(id)) },
    { key: 'term-lists' as const, ids: order.filter(isTermListName) },
  ].filter((group) => group.ids.length > 0);

const dataTypeLabel = (options: TemplateFormOptions, id: string): string =>
  options.dataTypes.find((dataType) => dataType.id === id)?.label ?? id;

const propertyOptions = (properties: readonly Property[]) =>
  properties.map((property) => ({ id: property.id, label: property.term }));

const toWriteRow = (row: PropertyRowDraft, order: readonly string[]): TemplatePropertyWrite => ({
  propertyId: row.propertyId,
  alternateLabel: nullable(row.alternateLabel),
  alternateComment: nullable(row.alternateComment),
  dataTypes: order.filter((id) => row.dataTypes.includes(id)),
  isRequired: row.isRequired,
  isPrivate: row.isPrivate,
  defaultLang: nullable(row.defaultLang),
});

const TemplateEditor = ({
  template,
  options,
  onSaved,
}: {
  readonly template: ResourceTemplate | null;
  readonly options: TemplateFormOptions;
  readonly onSaved: (template: ResourceTemplate) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const create = template === null;
  const [label, setLabel] = useState(template?.label ?? '');
  const [owner, setOwner] = useState<UserId | null>(template?.owner?.id ?? null);
  const [resourceClass, setResourceClass] = useState<ClassId | null>(
    template?.resourceClass?.id ?? null,
  );
  const [titleProperty, setTitleProperty] = useState<PropertyId | null>(
    template?.titleProperty?.id ?? null,
  );
  const [descriptionProperty, setDescriptionProperty] = useState<PropertyId | null>(
    template?.descriptionProperty?.id ?? null,
  );
  const [rows, setRows] = useState<readonly PropertyRowDraft[]>(rowsFrom(template));
  const [pending, setPending] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const patchRow = (key: string, patch: Partial<PropertyRowDraft>): void => {
    setRows(rows.map((row) => (row.key === key ? { ...row, ...patch } : row)));
  };

  const moveRow = (index: number, delta: number): void => {
    const target = index + delta;
    if (target < 0 || target >= rows.length) {
      return;
    }
    const next = [...rows];
    const moved = next[index];
    const replaced = next[target];
    if (moved === undefined || replaced === undefined) {
      return;
    }
    next[index] = replaced;
    next[target] = moved;
    setRows(next);
  };

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (label.trim() === '') {
      setFormError(t('form.errorEmptyLabel'));
      return;
    }
    setFormError(null);
    const write: ResourceTemplateWrite = {
      label,
      owner,
      resourceClass,
      titleProperty,
      descriptionProperty,
      properties: rows.map((row) => toWriteRow(row, dataTypeIds(options, row))),
    };
    save(async () => {
      if (template === null) {
        const created = await api.createResourceTemplate(write);
        window.location.hash = `#/resource-templates/${String(created.id)}`;
      } else {
        onSaved(await api.updateResourceTemplate(template.id, write));
      }
    });
  };

  const addable = options.properties.filter(
    (property) => !rows.some((row) => row.propertyId === property.id),
  );

  return (
    <form onSubmit={submit}>
      <fieldset>
        <legend>{t('resourceTemplates.legend')}</legend>
        <TextField
          id="template-label"
          label={t('column.label')}
          value={label}
          onChange={setLabel}
        />
        <OptionSelect
          id="template-class"
          label={t('resourceTemplates.suggestedClass')}
          options={classOptions(options.classes)}
          value={resourceClass}
          onChange={setResourceClass}
        />
        <OptionSelect
          id="template-title-property"
          label={t('detail.titleProperty')}
          options={propertyOptions(options.properties)}
          value={titleProperty}
          onChange={setTitleProperty}
        />
        <OptionSelect
          id="template-description-property"
          label={t('detail.descriptionProperty')}
          options={propertyOptions(options.properties)}
          value={descriptionProperty}
          onChange={setDescriptionProperty}
        />
        <OptionSelect
          id="template-owner"
          label={t('field.owner')}
          options={userOptions(options.users)}
          value={owner}
          onChange={setOwner}
          emptyLabel={t('common.noOwner')}
        />
      </fieldset>

      <fieldset>
        <legend>{t('resourceTemplates.propertiesLegend')}</legend>
        {rows.length === 0 ? (
          <p>{t('resourceTemplates.noProperties')}</p>
        ) : (
          <ul className="template-rows">
            {rows.map((row, index) => {
              const term = propertyTerm(options.properties, row.propertyId);
              const order = dataTypeIds(options, row);
              return (
                <li key={row.key} className="template-row">
                  <h3>{term}</h3>
                  <p>
                    <label htmlFor={`${row.key}-alternate-label`}>
                      {t('detail.alternateLabel')}
                    </label>
                    <input
                      id={`${row.key}-alternate-label`}
                      type="text"
                      value={row.alternateLabel}
                      onChange={(event) => {
                        patchRow(row.key, { alternateLabel: event.target.value });
                      }}
                    />
                  </p>
                  <p>
                    <label htmlFor={`${row.key}-alternate-comment`}>
                      {t('detail.alternateComment')}
                    </label>
                    <input
                      id={`${row.key}-alternate-comment`}
                      type="text"
                      value={row.alternateComment}
                      onChange={(event) => {
                        patchRow(row.key, { alternateComment: event.target.value });
                      }}
                    />
                  </p>
                  <fieldset>
                    <legend>{t('detail.dataTypes')}</legend>
                    {dataTypeGroups(order).map((group) => (
                      <div key={group.key} className="data-type-group">
                        {group.key === 'term-lists' ? (
                          <p className="data-type-group-label">{t('termLists.dataTypeGroup')}</p>
                        ) : null}
                        {group.ids.map((id) => (
                          <label key={id}>
                            <input
                              type="checkbox"
                              checked={row.dataTypes.includes(id)}
                              onChange={(event) => {
                                patchRow(row.key, {
                                  dataTypes: event.target.checked
                                    ? [...row.dataTypes, id]
                                    : row.dataTypes.filter((value) => value !== id),
                                });
                              }}
                            />
                            {dataTypeLabel(options, id)}
                          </label>
                        ))}
                      </div>
                    ))}
                  </fieldset>
                  <p>
                    <label htmlFor={`${row.key}-default-lang`}>{t('detail.defaultLang')}</label>
                    <input
                      id={`${row.key}-default-lang`}
                      type="text"
                      value={row.defaultLang}
                      onChange={(event) => {
                        patchRow(row.key, { defaultLang: event.target.value });
                      }}
                    />
                  </p>
                  <p>
                    <label htmlFor={`${row.key}-required`}>
                      <input
                        id={`${row.key}-required`}
                        type="checkbox"
                        checked={row.isRequired}
                        onChange={(event) => {
                          patchRow(row.key, { isRequired: event.target.checked });
                        }}
                      />
                      {t('detail.required')}
                    </label>
                  </p>
                  <p>
                    <label htmlFor={`${row.key}-private`}>
                      <input
                        id={`${row.key}-private`}
                        type="checkbox"
                        checked={row.isPrivate}
                        onChange={(event) => {
                          patchRow(row.key, { isPrivate: event.target.checked });
                        }}
                      />
                      {t('common.private')}
                    </label>
                  </p>
                  <p>
                    <button
                      type="button"
                      aria-label={t('resourceTemplates.moveUp', { term })}
                      disabled={index === 0}
                      onClick={() => {
                        moveRow(index, -1);
                      }}
                    >
                      {t('navigation.moveUpShort')}
                    </button>
                    <button
                      type="button"
                      aria-label={t('resourceTemplates.moveDown', { term })}
                      disabled={index === rows.length - 1}
                      onClick={() => {
                        moveRow(index, 1);
                      }}
                    >
                      {t('navigation.moveDownShort')}
                    </button>
                    <button
                      type="button"
                      aria-label={t('resourceTemplates.removeRow', { term })}
                      onClick={() => {
                        setRows(rows.filter((entry) => entry.key !== row.key));
                      }}
                    >
                      {t('action.remove')}
                    </button>
                  </p>
                </li>
              );
            })}
          </ul>
        )}
        <p>
          <label htmlFor="template-add-property">{t('field.addProperty')}</label>
          <select
            id="template-add-property"
            value={pending}
            onChange={(event) => {
              setPending(event.target.value);
            }}
          >
            <option value="">{t('field.selectProperty')}</option>
            {addable.map((property) => (
              <option key={String(property.id)} value={String(property.id)}>
                {property.term}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={pending === ''}
            onClick={() => {
              const property = selectedOption(propertyOptions(addable), pending);
              if (property === null) {
                return;
              }
              setRows([
                ...rows,
                {
                  key: rowKey(),
                  propertyId: property,
                  alternateLabel: '',
                  alternateComment: '',
                  dataTypes: [],
                  isRequired: false,
                  isPrivate: false,
                  defaultLang: '',
                },
              ]);
              setPending('');
            }}
          >
            {t('field.addProperty')}
          </button>
        </p>
      </fieldset>

      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow
        label={create ? t('resourceTemplates.add') : t('action.save')}
        busy={saveState.status === 'saving'}
      />
    </form>
  );
};

/** The edit form on the resource template detail view. */
export const ResourceTemplateEditForm = ({
  template,
  onSaved,
}: {
  readonly template: ResourceTemplate;
  readonly onSaved: (template: ResourceTemplate) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(loadOptions, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <TemplateEditor
      key={String(template.id)}
      template={template}
      options={state.data}
      onSaved={onSaved}
    />
  );
};

/** `#/resource-templates/new`. */
export const ResourceTemplateCreatePage = (): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(loadOptions, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.resourceTemplateForm')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <TemplateEditor template={null} options={state.data} onSaved={() => undefined} />
  );
};
