import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ItemSet, ItemSetId, ItemSetWrite } from '../../api/types';
import { toValueWrites, type ResourceDraft } from '../../app/draft';
import { DeleteControl } from '../../app/delete';
import { CheckboxField, ResourceFields, SaveStatus, SubmitRow } from '../../app/fields';
import { ErrorNotice, FormErrors } from '../../app/errors';
import { loadFormOptions, type FormOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { draftErrors, openDraft, templateById } from '../../app/value-fields';
import { ValuesEditor } from '../../app/values-editor';
import { useTranslator } from '../../i18n';

interface Loaded {
  readonly itemSet: ItemSet | null;
  readonly options: FormOptions;
}

const load = async (itemSetId: ItemSetId | null): Promise<Loaded> => {
  const [itemSet, options] = await Promise.all([
    itemSetId === null ? Promise.resolve(null) : api.getItemSet(itemSetId),
    loadFormOptions(),
  ]);
  return { itemSet, options };
};

const ItemSetEditor = ({
  itemSetId,
  loaded,
}: {
  readonly itemSetId: ItemSetId | null;
  readonly loaded: Loaded;
}): ReactElement => {
  const { t } = useTranslator();
  const { options } = loaded;
  const [draft, setDraft] = useState<ResourceDraft>(() =>
    openDraft(loaded.itemSet, options.templates, options.properties),
  );
  const [isOpen, setIsOpen] = useState(loaded.itemSet?.isOpen ?? false);
  const [formErrors, setFormErrors] = useState<readonly string[]>([]);
  const { state: saveState, save } = useSave();
  const stored = loaded.itemSet;

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const errors = draftErrors(draft, options.templates, options.properties, t);
    setFormErrors(errors);
    if (errors.length > 0) {
      return;
    }
    const write: ItemSetWrite = {
      isPublic: draft.isPublic,
      owner: draft.owner,
      resourceClass: draft.resourceClass,
      resourceTemplate: draft.resourceTemplate,
      thumbnail: draft.thumbnail,
      isOpen,
      values: toValueWrites(draft.values),
    };
    save(async () => {
      if (itemSetId === null) {
        const created = await api.createItemSet(write);
        window.location.hash = `#/item-sets/${String(created.id)}`;
      } else {
        await api.updateItemSet(itemSetId, write);
      }
    });
  };

  return (
    <div>
      <form onSubmit={submit}>
        <ResourceFields idPrefix="item-set" draft={draft} onChange={setDraft} options={options} />
        <CheckboxField
          id="item-set-open"
          label={t('itemSets.open')}
          checked={isOpen}
          onChange={setIsOpen}
        />
        <ValuesEditor
          idPrefix="item-set"
          draft={draft}
          onChange={setDraft}
          template={templateById(options.templates, draft.resourceTemplate)}
          properties={options.properties}
          dataTypes={options.dataTypes}
        />
        <FormErrors errors={formErrors} />
        <SaveStatus state={saveState} />
        <SubmitRow
          label={itemSetId === null ? t('itemSets.add') : t('action.save')}
          busy={saveState.status === 'saving'}
        />
      </form>
      {stored === null ? null : (
        <DeleteControl
          label={t('itemSets.deleteLabel')}
          rowName={stored.title ?? t('itemSets.rowName', { id: stored.id })}
          onConfirm={async () => {
            await api.deleteItemSet(stored.id);
            window.location.hash = '#/item-sets';
          }}
        />
      )}
    </div>
  );
};

/** `Admin/ItemSetController::addAction` and `::editAction`. */
export const ItemSetForm = ({
  itemSetId,
}: {
  readonly itemSetId: ItemSetId | null;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(itemSetId), [itemSetId]);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.itemSet')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <ItemSetEditor
      key={itemSetId === null ? 'new' : String(itemSetId)}
      itemSetId={itemSetId}
      loaded={state.data}
    />
  );
};
