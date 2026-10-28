/**
 * The item set delete, on the set's card and on its edit form. The set moves to the trash, so
 * the confirmation states what it changes and what Undo puts back: the items stay and leave the
 * set, and the sites that show the set stop showing it, until Undo.
 */
import type { ReactElement } from 'react';
import type { ItemSet } from '../../api/types';
import { useAsync } from '../../app/useAsync';
import type { AsyncState } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { TrashOpener } from '../trash/opener';
import { loadSetCounts } from './facts';
import type { SetCounts } from './facts';

const Consequences = ({ counts }: { readonly counts: AsyncState<SetCounts> }): ReactElement | null => {
  const { t, number } = useTranslator(common, resources);
  if (counts.status === 'loading') {
    return <p role="status">{t('setDelete.counting')}</p>;
  }
  // A failed count leaves the confirmation as it was before the counts existed.
  if (counts.status === 'error') {
    return null;
  }
  const { itemCount, sites } = counts.data;
  return (
    <>
      <p>
        {itemCount === 0
          ? t('setDelete.itemsNone')
          : itemCount === 1
            ? t('setDelete.itemsOne')
            : t('setDelete.itemsMany', { count: number(itemCount) })}
      </p>
      {sites.length === 0 ? null : (
        <p>
          {sites.length === 1
            ? t('setDelete.sitesOne')
            : t('setDelete.sitesMany', { count: number(sites.length) })}
        </p>
      )}
    </>
  );
};

export const SetDeleteControl = ({
  set,
  counts,
}: {
  readonly set: ItemSet;
  readonly counts: AsyncState<SetCounts>;
}): ReactElement => {
  const { t } = useTranslator(common, resources);
  return (
    <TrashOpener
      label={t('itemSets.deleteLabel')}
      target={{ of: 'item_sets', id: set.id }}
      name={set.title ?? t('itemSets.rowName', { id: set.id })}
      detail={<Consequences counts={counts} />}
    />
  );
};

/** The edit form's delete: it reads the counts itself, so a failed count never blocks the form. */
export const SetDeleteReadingCounts = ({ set }: { readonly set: ItemSet }): ReactElement => {
  const counts = useAsync(() => loadSetCounts(set.id), [set.id]);
  return <SetDeleteControl set={set} counts={counts} />;
};
