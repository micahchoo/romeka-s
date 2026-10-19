import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Item, ItemSetId, ItemWrite } from '../../api/types';
import { toValueWrites, type ResourceDraft } from '../../app/draft';
import { CheckboxField, ResourceFields, SaveStatus, SubmitRow } from '../../app/fields';
import { ErrorNotice, FormErrors } from '../../app/errors';
import { loadFormOptions, type FormOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { draftErrors, openDraft, templateById } from '../../app/value-fields';
import { createLocalField } from '../../app/local-fields';
import { ValuesEditor } from '../../app/values-editor';
import { writtenItems } from '../../app/written';
import { useTranslator } from '../../i18n';

interface ItemEditorProps {
  /** The stored item to edit, or `null` to create one. */
  readonly item: Item | null;
  readonly options: FormOptions;
  readonly onSaved: (item: Item) => void;
}

const ItemEditor = ({ item, options, onSaved }: ItemEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [draft, setDraft] = useState<ResourceDraft>(() =>
    openDraft(item, options.templates, options.properties),
  );
  const [itemSets, setItemSets] = useState<readonly ItemSetId[]>(
    () => item?.itemSets.map((ref) => ref.id) ?? [],
  );
  const [formErrors, setFormErrors] = useState<readonly string[]>([]);
  const { state: saveState, save } = useSave();

  const toggleItemSet = (id: ItemSetId, checked: boolean): void => {
    setItemSets((current) =>
      checked ? [...current, id] : current.filter((entry) => entry !== id),
    );
  };

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const errors = draftErrors(draft, options.templates, options.properties, t);
    setFormErrors(errors);
    if (errors.length > 0) {
      return;
    }
    const write: ItemWrite = {
      isPublic: draft.isPublic,
      owner: draft.owner,
      resourceClass: draft.resourceClass,
      resourceTemplate: draft.resourceTemplate,
      thumbnail: draft.thumbnail,
      values: toValueWrites(draft.values),
      itemSets,
      primaryMedia: item?.primaryMedia?.id ?? null,
      sites: item === null ? null : item.sites.map((ref) => ref.id),
    };
    save(async () => {
      const saved =
        item === null ? await api.createItem(write) : await api.updateItem(item.id, write);
      onSaved(saved);
    });
  };

  return (
    <form onSubmit={submit}>
      <ResourceFields idPrefix="item" kind="items" draft={draft} onChange={setDraft} options={options} />
      <ValuesEditor
        idPrefix="item"
        draft={draft}
        onChange={setDraft}
        template={templateById(options.templates, draft.resourceTemplate)}
        properties={options.properties}
        dataTypes={options.dataTypes}
        createField={createLocalField}
        showLessons
      />
      <fieldset>
        <legend>{t('items.setsLegend')}</legend>
        {options.itemSets.length === 0 ? (
          <p>{t('items.noSets')}</p>
        ) : (
          options.itemSets.map((entry) => (
            <CheckboxField
              key={String(entry.id)}
              id={`item-set-${String(entry.id)}`}
              label={entry.title ?? t('common.untitled')}
              checked={itemSets.includes(entry.id)}
              onChange={(checked) => {
                toggleItemSet(entry.id, checked);
              }}
            />
          ))
        )}
      </fieldset>
      <FormErrors errors={formErrors} />
      <SaveStatus state={saveState} />
      <SubmitRow
        label={item === null ? t('items.add') : t('action.save')}
        busy={saveState.status === 'saving'}
      />
    </form>
  );
};

const OptionsState = ({
  load,
  children,
}: {
  readonly load: () => Promise<FormOptions>;
  readonly children: (options: FormOptions) => ReactElement;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(load, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return children(state.data);
};

/** `Admin/ItemController::addAction`: a new item with the current admin rules. */
export const ItemForm = (): ReactElement => (
  <OptionsState load={loadFormOptions}>
    {(options) => (
      <ItemEditor
        item={null}
        options={options}
        onSaved={(created) => {
          writtenItems.remember(created.id, created);
          window.location.hash = `#/items/${String(created.id)}`;
        }}
      />
    )}
  </OptionsState>
);

/** The edit half of the item show page, which already has the item loaded. */
export const ItemEditForm = ({
  item,
  onSaved,
}: {
  readonly item: Item;
  readonly onSaved: (item: Item) => void;
}): ReactElement => (
  <OptionsState load={loadFormOptions}>
    {(options) => <ItemEditor item={item} options={options} onSaved={onSaved} />}
  </OptionsState>
);
