import type { ReactElement } from 'react';
import { useEffect, useRef } from 'react';
import type { DataType, Item, ResourceTemplate } from '../../api/types';
import { ValuesList } from '../../app/values';
import { useTranslator } from '../../i18n';
import { ItemEditForm } from './form';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';

interface ItemValuesProps {
  readonly item: Item;
  readonly template: ResourceTemplate | null;
  readonly dataTypes: readonly DataType[];
  /** The form shows in place of the read list. */
  readonly editing: boolean;
  /** The last edit saved, so the read list says so until the next edit. */
  readonly justSaved: boolean;
  /** The page was opened to edit (`?edit=1`), so the form scrolls into view on arrival. */
  readonly openedToEdit: boolean;
  readonly onSaved: (item: Item) => void;
  readonly onCancel: () => void;
  readonly saveNext: string | null;
}

/**
 * The item's values in one place: the read list, or the form that edits it. Only one copy of
 * the values is on the page at a time, so the list and the form never disagree.
 */
export const ItemValues = ({
  item,
  template,
  dataTypes,
  editing,
  justSaved,
  openedToEdit,
  onSaved,
  onCancel,
  saveNext,
}: ItemValuesProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const section = useRef<HTMLElement>(null);
  useEffect(() => {
    if (openedToEdit) {
      section.current?.scrollIntoView({ block: 'start' });
    }
  }, [openedToEdit]);
  return (
    <section className="item-values" ref={section}>
      <h2>{t('items.valuesHeading')}</h2>
      {editing ? (
        <ItemEditForm item={item} onSaved={onSaved} onCancel={onCancel} saveNext={saveNext} />
      ) : (
        <>
          {justSaved ? <p role="status">{t('field.saved')}</p> : null}
          <ValuesList values={item.values} template={template} dataTypes={dataTypes} />
        </>
      )}
    </section>
  );
};
