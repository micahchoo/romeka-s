/**
 * The Items list's CSV download: every item the screen matches, on every page, and the ticked
 * items alone once a row is ticked. Each is a plain link to `GET /api/romeka/items.csv`, so the
 * browser saves the file itself; the server answers the same rows `/api/items` counts.
 */
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ItemId } from '../../api/types';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { everyMatchQuery, tickedListQuery } from './browse';
import type { ItemBrowse } from './browse';

const ITEMS_CSV = '/api/romeka/items.csv';

export const CsvDownload = ({
  state,
  total,
  picked,
}: {
  readonly state: ItemBrowse;
  /** How many items the list matches, every page. */
  readonly total: number;
  readonly picked: readonly ItemId[];
}): ReactElement | null => {
  const { t, number } = useTranslator(common, resources);
  if (total === 0 && picked.length === 0) {
    return null;
  }
  return (
    <p className="items-download">
      {total === 0 ? null : (
        <a href={api.downloadUrl(ITEMS_CSV, everyMatchQuery(state))} download>
          {total === 1 ? t('browse.csvOne') : t('browse.csvMany', { count: number(total) })}
        </a>
      )}
      {picked.length === 0 ? null : (
        <a href={api.downloadUrl(ITEMS_CSV, tickedListQuery(state, picked))} download>
          {picked.length === 1
            ? t('browse.csvTickedOne')
            : t('browse.csvTickedMany', { count: number(picked.length) })}
        </a>
      )}
    </p>
  );
};
