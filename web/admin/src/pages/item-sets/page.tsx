import type { ReactElement } from 'react';
import { itemSetIdFromParam } from '../../api/parse';
import { useTranslator } from '../../i18n';
import { ItemSetForm } from './form';
import { ItemSetList } from './list';
import { resources } from '../../i18n/namespaces/resources';

/** `#/item-sets` lists, `#/item-sets/new` creates, `#/item-sets/<id>` edits. */
export const ItemSetsPage = ({ param = null }: { readonly param?: string | null } = {}): ReactElement => {
  const { t } = useTranslator(resources);
  if (param === null) {
    return <ItemSetList />;
  }
  if (param === 'new') {
    return <ItemSetForm itemSetId={null} />;
  }
  const itemSetId = itemSetIdFromParam(param);
  if (itemSetId === null) {
    return <p role="alert">{t('route.invalidItemSetId', { param })}</p>;
  }
  return <ItemSetForm key={String(itemSetId)} itemSetId={itemSetId} />;
};
