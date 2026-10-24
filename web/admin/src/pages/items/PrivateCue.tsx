import type { ReactElement } from 'react';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { FilterValue } from './filter-value';
import type { OnFilter } from './filter-value';

/** The word "Private" on a private item, which also filters to private items. */
export const PrivateCue = ({ onFilter }: { readonly onFilter: OnFilter }): ReactElement => {
  const { t } = useTranslator(common, resources);
  return (
    <FilterValue
      className="private-cue"
      label={t('common.private')}
      title={t('browse.onlyPrivate')}
      change={{ visibility: 'private' }}
      onFilter={onFilter}
    />
  );
};
