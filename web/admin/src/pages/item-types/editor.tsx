/**
 * The item type editor: the fields on the left, the real item form they make
 * on the right (Fields and Preview tabs under 900px). It edits every member
 * `ResourceTemplateAdapter::hydrate` accepts and sends back what it read for
 * anything the user did not touch.
 */
import { useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { useTermText } from '../../i18n/termComments';
import { api } from '../../api/client';
import type { FieldCounts } from '../../api/places';
import type { Property, ResourceTemplate, TemplateId } from '../../api/types';
import { SaveStatus } from '../../app/fields';
import { ReorderStatus, useReorderable } from '../../app/reorder/useReorderable';
import { createLocalField, isLocalTerm } from '../../app/local-fields';
import { useSave } from '../../app/save';
import { StandardTermPicker } from '../../app/standard-term-picker';
import { RESOURCE_KINDS, setUsedFor, usedFor, type ResourceKind } from '../../app/type-use';
import { FieldFill } from './field-fill';
import { FieldRow, type TermLesson } from './field-row';
import { useItemTypesText } from './messages';
import {
  addField,
  freeProperties,
  removeField,
  toTemplateWrite,
  type TypeDraft,
} from './model';
import type { TypeOptions } from './options';
import { TypePreview } from './preview';
import { ClassLine, TypeSettings } from './type-settings';

type Tab = 'fields' | 'preview';

export interface TypeEditorProps {
  readonly template: ResourceTemplate | null;
  readonly initial: TypeDraft;
  readonly options: TypeOptions;
  readonly onSaved: (template: ResourceTemplate) => void;
  /** How many of the stored type's items fill each field; absent for a new type or while unknown. */
  readonly counts?: FieldCounts | null;
}

/** The properties a create added during this visit, beside the loaded ones. */
const useProperties = (
  loaded: readonly Property[],
): readonly [readonly Property[], (name: string) => Promise<Property>] => {
  const [created, setCreated] = useState<readonly Property[]>([]);
  const properties = [...loaded, ...created];
  const create = async (name: string): Promise<Property> => {
    const property = await createLocalField(name, properties);
    if (!properties.some((entry) => entry.id === property.id)) {
      setCreated((current) => [...current, property]);
    }
    return property;
  };
  return [properties, create];
};

export const TypeEditor = ({
  template,
  initial,
  options,
  onSaved,
  counts = null,
}: TypeEditorProps): ReactElement => {
  const t = useItemTypesText();
  const [draft, setDraft] = useState(initial);
  const [kinds, setKinds] = useState<readonly ResourceKind[]>(() =>
    template === null ? RESOURCE_KINDS : usedFor(template.id),
  );
  const [tab, setTab] = useState<Tab>('fields');
  const [formError, setFormError] = useState<string | null>(null);
  const [properties, createField] = useProperties(options.properties);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (draft.label.trim() === '') {
      setFormError(t('type.errorName'));
      return;
    }
    setFormError(null);
    const write = toTemplateWrite(draft);
    save(async () => {
      const saved =
        template === null
          ? await api.createResourceTemplate(write)
          : await api.updateResourceTemplate(template.id, write);
      setUsedFor(saved.id, kinds);
      onSaved(saved);
    });
  };

  return (
    <div className={`type-editor type-editor--${tab}`}>
      <EditorHead
        draft={draft}
        onChange={setDraft}
        classes={options.classes}
        creating={template === null}
        saving={saveState.status === 'saving'}
      />
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <EditorTabs tab={tab} onTab={setTab} />
      <div className="type-editor-panes">
        <form id="type-form" className="type-fields" onSubmit={submit}>
          <h2 className="type-eyebrow">{t('fields.heading')}</h2>
          <FieldList
            draft={draft}
            onChange={setDraft}
            properties={properties}
            options={options}
            createField={createField}
            fill={template === null || counts === null ? null : { template: template.id, counts }}
          />
          <StandardTermPicker
            label={t('fields.add')}
            placeholder={t('fields.addHint')}
            entries={freeProperties(draft, properties, null)}
            onPick={(property) => {
              setDraft(addField(draft, property, ['literal']));
            }}
            onCreate={async (fieldName) => {
              setDraft(addField(draft, await createField(fieldName), ['literal']));
            }}
          />
          <TypeSettings
            draft={draft}
            onChange={setDraft}
            kinds={kinds}
            onKinds={setKinds}
            options={options}
            properties={properties}
          />
        </form>
        <TypePreview
          id="type-preview"
          draft={draft}
          properties={properties}
          dataTypes={options.dataTypes}
        />
      </div>
    </div>
  );
};

interface FieldListProps {
  readonly draft: TypeDraft;
  readonly onChange: (draft: TypeDraft) => void;
  readonly properties: readonly Property[];
  readonly options: TypeOptions;
  readonly createField: (name: string) => Promise<Property>;
  readonly fill: { readonly template: TemplateId; readonly counts: FieldCounts } | null;
}

const FieldList = ({
  draft,
  onChange,
  properties,
  options,
  createField,
  fill,
}: FieldListProps): ReactElement => {
  const t = useItemTypesText();
  const text = useTermText();
  const nameOf = (field: TypeDraft['fields'][number]): string => {
    const property = properties.find((entry) => entry.id === field.propertyId);
    return (
      field.alternateLabel ??
      (property === undefined ? null : text.label(property.term, property.label)) ??
      property?.term ??
      `#${String(field.propertyId)}`
    );
  };
  const reorder = useReorderable({
    items: draft.fields,
    onReorder: (fields) => {
      onChange({ ...draft, fields });
    },
    labelOf: nameOf,
  });
  if (draft.fields.length === 0) {
    return <p className="field-note">{t('fields.empty')}</p>;
  }
  const isLocal = (field: TypeDraft['fields'][number]): boolean =>
    isLocalTerm(properties.find((entry) => entry.id === field.propertyId)?.term ?? '');
  const firstStandard = draft.fields.findIndex((field) => !isLocal(field));
  const firstLocal = draft.fields.findIndex(isLocal);
  const termLesson = (index: number): TermLesson =>
    index === firstStandard ? 'term' : index === firstLocal ? 'local-field' : null;
  return (
    <>
      <ol className="type-field-list">
        {draft.fields.map((field, index) => (
          <FieldRow
            key={field.key}
            field={field}
            index={index}
            reorder={reorder}
            name={nameOf(field)}
            draft={draft}
            onChange={onChange}
            onRemove={(key) => {
              onChange(removeField(draft, key));
            }}
            properties={properties}
            freeProperties={freeProperties(draft, properties, field.propertyId)}
            dataTypes={options.dataTypes}
            createField={createField}
            termLesson={termLesson(index)}
            fill={
              fill === null ? null : (
                <FieldFill template={fill.template} property={field.propertyId} counts={fill.counts} />
              )
            }
          />
        ))}
      </ol>
      <ReorderStatus reorder={reorder} />
    </>
  );
};

/** The name, the standard class line, Cancel and Save. */
const EditorHead = ({
  draft,
  onChange,
  classes,
  creating,
  saving,
}: {
  readonly draft: TypeDraft;
  readonly onChange: (draft: TypeDraft) => void;
  readonly classes: TypeOptions['classes'];
  readonly creating: boolean;
  readonly saving: boolean;
}): ReactElement => {
  const t = useItemTypesText();
  return (
    <header className="type-editor-head">
      <div className="type-editor-title">
        <input
          className="type-name"
          type="text"
          aria-label={t('type.name')}
          placeholder={t('type.namePlaceholder')}
          value={draft.label}
          onChange={(event) => {
            onChange({ ...draft, label: event.target.value });
          }}
        />
        <ClassLine draft={draft} classes={classes} onChange={onChange} />
      </div>
      <p className="type-editor-actions">
        <a className="button-link" href="#/resource-templates">
          {t('itemTypes.actionCancel')}
        </a>
        <button type="submit" form="type-form" disabled={saving}>
          {creating ? t('type.create') : t('type.save')}
        </button>
      </p>
    </header>
  );
};

/** Fields and Preview, shown under 900px where the panes stack. */
const EditorTabs = ({
  tab,
  onTab,
}: {
  readonly tab: Tab;
  readonly onTab: (tab: Tab) => void;
}): ReactElement => {
  const t = useItemTypesText();
  return (
    <div className="type-tabs" role="tablist" aria-label={t('tabs.label')}>
      {(['fields', 'preview'] as const).map((entry) => (
        <button
          key={entry}
          type="button"
          role="tab"
          aria-selected={tab === entry}
          aria-controls={entry === 'fields' ? 'type-form' : 'type-preview'}
          onClick={() => {
            onTab(entry);
          }}
        >
          {t(entry === 'fields' ? 'tabs.fields' : 'tabs.preview')}
        </button>
      ))}
    </div>
  );
};
