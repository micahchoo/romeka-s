import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ItemSet, ItemSetId, ItemSetWrite } from '../../api/types';
import {
  emptyResourceDraft,
  literalTextOf,
  resourceDraftFrom,
  toValueWrites,
  valueErrors,
  withLiteralText,
  type ResourceDraft,
} from '../../app/draft';
import { DeleteControl } from '../../app/delete';
import {
  CheckboxField,
  ResourceFields,
  SaveStatus,
  SubmitRow,
  TextField,
  ValuesEditor,
} from '../../app/fields';
import { ErrorNotice } from '../../app/errors';
import { loadFormOptions, titleProperty, type FormOptions } from '../../app/options';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
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
  const [draft, setDraft] = useState<ResourceDraft>(() =>
    loaded.itemSet === null ? emptyResourceDraft() : resourceDraftFrom(loaded.itemSet),
  );
  const [isOpen, setIsOpen] = useState(loaded.itemSet?.isOpen ?? false);
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const title = titleProperty(
    loaded.options.properties,
    loaded.options.templates,
    draft.resourceTemplate,
  );
  const titleText = title === null ? '' : literalTextOf(draft.values, title.term);
  const excludeTerms = title === null ? [] : [title.term];
  const stored = loaded.itemSet;

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const errors = valueErrors(draft.values, t);
    const first = errors[0];
    if (first !== undefined) {
      setFormError(first);
      return;
    }
    setFormError(null);
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
        <ResourceFields
          idPrefix="item-set"
          draft={draft}
          onChange={setDraft}
          options={loaded.options}
        />
        {title === null ? null : (
          <TextField
            id="item-set-title"
            label={title.label ?? title.term}
            value={titleText}
            onChange={(value) => {
              setDraft(withLiteralText(draft, title, value));
            }}
          />
        )}
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
          properties={loaded.options.properties}
          excludeTerms={excludeTerms}
        />
        {formError === null ? null : <p role="alert">{formError}</p>}
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
