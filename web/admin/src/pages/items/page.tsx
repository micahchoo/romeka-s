import type { ReactElement } from 'react';
import { itemIdFromParam } from '../../api/parse';
import type { PageProps } from '../../app/router';
import { useTranslator } from '../../i18n';
import { ItemDetail } from './detail';
import { ItemForm } from './form';
import { ItemsList } from './list';
import { resources } from '../../i18n/namespaces/resources';

/** `#/items` lists, `#/items/new` creates, `#/items/<id>` shows and edits. */
export const ItemsPage = ({ param }: PageProps): ReactElement => {
  const { t } = useTranslator(resources);
  if (param === null) {
    return <ItemsList />;
  }
  if (param === 'new') {
    return <ItemForm />;
  }
  const itemId = itemIdFromParam(param);
  if (itemId === null) {
    return <p role="alert">{t('route.invalidItemId', { param })}</p>;
  }
  return <ItemDetail key={String(itemId)} itemId={itemId} />;
};
