import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Item, ItemSetId, ItemWrite, Site, SiteId } from '../../api/types';
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
import { useTermText } from '../../i18n/termComments';
import { loadAssignableSites, SitesFieldset } from './sites';
import { PlaceField, locationFields } from './PlaceField';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

interface ItemEditorProps {
  /** The stored item to edit, or `null` to create one. */
  readonly item: Item | null;
  readonly options: ItemOptions;
  readonly onSaved: (item: Item) => void;
  /** The next item's edit address in the list this item was opened from, or null when none follows. */
  readonly saveNext: string | null;
  /** Leaves the form unsaved; absent on the create form, which has nowhere to return to. */
  readonly onCancel?: () => void;
}

/** The shared form lists, plus the sites this user may add items to. */
interface ItemOptions extends FormOptions {
  readonly sites: readonly Site[];
}

const loadItemOptions = async (): Promise<ItemOptions> => {
  const [options, sites] = await Promise.all([loadFormOptions(), loadAssignableSites()]);
  return { ...options, sites };
};

/** The submit button that sends the walk on to the next item. */
const SAVE_NEXT = 'next';

const ItemEditor = ({ item, options, onSaved, saveNext, onCancel }: ItemEditorProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const text = useTermText();
  const [draft, setDraft] = useState<ResourceDraft>(() =>
    openDraft(item, options.templates, options.properties),
  );
  const [itemSets, setItemSets] = useState<readonly ItemSetId[]>(
    () => item?.itemSets.map((ref) => ref.id) ?? [],
  );
  const [sites, setSites] = useState<readonly SiteId[]>(
    () => item?.sites.map((ref) => ref.id) ?? [],
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
    const submitter = event.nativeEvent instanceof SubmitEvent ? event.nativeEvent.submitter : null;
    const then = submitter instanceof HTMLButtonElement && submitter.value === SAVE_NEXT ? saveNext : null;
    const errors = draftErrors(draft, options.templates, options.properties, t, text);
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
      // A create naming no site leaves `o:site` out, so the synced sites take the item.
      sites: item === null && sites.length === 0 ? null : sites,
    };
    save(async () => {
      const saved =
        item === null ? await api.createItem(write) : await api.updateItem(item.id, write);
      onSaved(saved);
      if (then !== null) {
        window.location.hash = then;
      }
    });
  };

  return (
    <form onSubmit={submit}>
      <ResourceFields
        idPrefix="item"
        kind="items"
        draft={draft}
        onChange={setDraft}
        options={options}
        editing={item !== null}
      />
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
      {locationFields(options.sites, options.properties).map((field) => (
        <PlaceField key={field.term} field={field} draft={draft} onChange={setDraft} />
      ))}
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
      <SitesFieldset
        assignable={options.sites}
        chosen={sites}
        creating={item === null}
        onChange={setSites}
      />
      <FormErrors errors={formErrors} />
      <SaveStatus state={saveState} />
      {item === null ? <p className="field-help">{t('items.addMediaNextPage')}</p> : null}
      <SubmitRow
        label={item === null ? t('items.add') : t('action.save')}
        busy={saveState.status === 'saving'}
        also={
          <>
            {saveNext === null ? null : (
              <button type="submit" value={SAVE_NEXT} disabled={saveState.status === 'saving'}>
                {t('walk.saveNext')}
              </button>
            )}
            {onCancel === undefined ? null : (
              <button type="button" onClick={onCancel} disabled={saveState.status === 'saving'}>
                {t('action.cancel')}
              </button>
            )}
          </>
        }
      />
    </form>
  );
};

const OptionsState = ({
  load,
  children,
}: {
  readonly load: () => Promise<ItemOptions>;
  readonly children: (options: ItemOptions) => ReactElement;
}): ReactElement => {
  const { t } = useTranslator(common, resources);
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
  <OptionsState load={loadItemOptions}>
    {(options) => (
      <ItemEditor
        item={null}
        saveNext={null}
        options={options}
        onSaved={(created) => {
          writtenItems.remember(created.id, created);
          window.location.hash = `#/items/${String(created.id)}`;
        }}
      />
    )}
  </OptionsState>
);

/** The item page's values form, in place of the read list, with the item already loaded. */
export const ItemEditForm = ({
  item,
  onSaved,
  onCancel,
  saveNext,
}: {
  readonly item: Item;
  readonly onSaved: (item: Item) => void;
  readonly onCancel: () => void;
  readonly saveNext: string | null;
}): ReactElement => (
  <OptionsState load={loadItemOptions}>
    {(options) => <ItemEditor item={item} options={options} onSaved={onSaved} onCancel={onCancel} saveNext={saveNext} />}
  </OptionsState>
);
