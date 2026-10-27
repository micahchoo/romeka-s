import type { ReactElement } from 'react';
import { useEffect } from 'react';
import { itemSetIdFromParam } from '../../api/parse';
import type { ItemSetId } from '../../api/types';
import type { PageProps } from '../../app/router';
import { useTranslator } from '../../i18n';
import { ItemSetForm } from './form';
import { ItemSetList } from './list';
import { setPlaceHref } from './facts';
import { resources } from '../../i18n/namespaces/resources';

/**
 * The set's own address opens its place. The entry is replaced, so Back from
 * the place returns to where the reader came from, not to this redirect.
 */
const OpenPlace = ({ itemSetId }: { readonly itemSetId: ItemSetId }): null => {
  useEffect(() => {
    window.location.replace(setPlaceHref(itemSetId));
  }, [itemSetId]);
  return null;
};

/**
 * `#/item-sets` lists, `#/item-sets/new` creates, `#/item-sets/<id>` opens the
 * set's place on Items, `#/item-sets/<id>/edit` edits.
 */
export const ItemSetsPage = ({ param = null, rest = [] }: Partial<PageProps> = {}): ReactElement => {
  const { t } = useTranslator(resources);
  if (param === null) {
    return <ItemSetList />;
  }
  if (param === 'new') {
    return <ItemSetForm itemSetId={null} />;
  }
  const itemSetId = itemSetIdFromParam(param);
  const action = rest[1];
  if (itemSetId === null || (action !== undefined && action !== 'edit') || rest.length > 2) {
    return <p role="alert">{t('route.invalidItemSetId', { param: rest.join('/') || param })}</p>;
  }
  if (action === 'edit') {
    return <ItemSetForm key={String(itemSetId)} itemSetId={itemSetId} />;
  }
  return <OpenPlace itemSetId={itemSetId} />;
};
