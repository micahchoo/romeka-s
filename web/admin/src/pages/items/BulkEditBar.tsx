/**
 * "Change items…", "Delete items…" and "Recent bulk changes" under the Items list's bulk bar. The
 * panel they open (`bulk/panel.tsx`) is its own chunk, fetched on the first click.
 *
 * The buttons follow upstream's batch privileges (`AclFactory`): an author may change and delete
 * the items she ticks (`batch_update`, `batch_delete`), a reviewer and up may change every item
 * that matches (`batch_update_all`), an editor and up delete them (`batch_delete_all`), and a
 * researcher none of it. The server checks the same rights again.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useState } from 'react';
import type { ResourceQuery } from '../../api/client';
import type { ItemId } from '../../api/types';
import { useSession } from '../../app/auth';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import type { BrowseOptions } from './browse-options';

const BulkPanel = lazy(() => import('./bulk/panel'));

const MAY_CHANGE_ALL: ReadonlySet<string> = new Set(['global_admin', 'site_admin', 'editor', 'reviewer']);
const MAY_CHANGE_TICKED: ReadonlySet<string> = new Set([...MAY_CHANGE_ALL, 'author']);
const MAY_DELETE_ALL: ReadonlySet<string> = new Set(['global_admin', 'site_admin', 'editor']);

type Opened = 'edit' | 'delete' | 'recent';

export const BulkEditBar = ({
  picked,
  query,
  total,
  filtered,
  options,
  onChanged,
}: {
  readonly picked: readonly ItemId[];
  readonly query: ResourceQuery;
  /** How many items the list matches, every page. */
  readonly total: number;
  readonly filtered: boolean;
  readonly options: BrowseOptions;
  readonly onChanged: () => void;
}): ReactElement | null => {
  const { t } = useTranslator(common, resources);
  const role = useSession()?.role ?? '';
  const [opened, setOpened] = useState<Opened | null>(null);
  if (!MAY_CHANGE_TICKED.has(role)) {
    return null;
  }
  const toggle = (next: Opened): void => {
    setOpened(opened === next ? null : next);
  };
  return (
    <div className="bulk-site-actions">
      <p className="bulk-site-row">
        <button
          type="button"
          aria-expanded={opened === 'edit'}
          onClick={() => {
            toggle('edit');
          }}
        >
          {t('items.bulkEdit')}
        </button>
        <button
          type="button"
          aria-expanded={opened === 'delete'}
          onClick={() => {
            toggle('delete');
          }}
        >
          {t('items.bulkDelete')}
        </button>
        <button
          type="button"
          className="link-button"
          aria-expanded={opened === 'recent'}
          onClick={() => {
            toggle('recent');
          }}
        >
          {t('items.bulkRecent')}
        </button>
      </p>
      {opened === null ? null : (
        <Suspense fallback={<p role="status">{t('items.bulkOpening')}</p>}>
          <BulkPanel
            key={opened}
            start={opened}
            picked={picked}
            query={query}
            reach={{
              ticked: picked.length,
              matching: total,
              filtered,
              canMatchAll: (opened === 'delete' ? MAY_DELETE_ALL : MAY_CHANGE_ALL).has(role),
            }}
            options={options}
            onClose={() => {
              setOpened(null);
            }}
            onChanged={onChanged}
          />
        </Suspense>
      )}
    </div>
  );
};
