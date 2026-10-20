/**
 * "Add to site…" and "Remove from site…" for the items picked on the Items list.
 *
 * Both are one `POST /api/sites/{id}/items` naming the picked ids, so the
 * answer says how many items changed and how many the site now holds.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { ItemId, Site, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import './bulk.css';

type Mode = 'add' | 'remove';

const SITES = 200;

export const BulkSiteActions = ({
  picked,
}: {
  readonly picked: readonly ItemId[];
}): ReactElement => {
  const { t } = useTranslator(common, resources);
  const [mode, setMode] = useState<Mode | null>(null);
  const [siteId, setSiteId] = useState<SiteId | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [failure, setFailure] = useState<Error | null>(null);
  const sites = useAsync(
    async (): Promise<readonly Site[]> =>
      mode === null ? [] : (await api.listSites({ perPage: SITES, sortBy: 'title' })).items,
    [mode === null],
  );
  const options = sites.status === 'ready' ? sites.data : [];
  const site = options.find((entry) => entry.id === siteId) ?? null;

  const confirm = (chosen: Site, action: Mode): void => {
    setBusy(true);
    setFailure(null);
    api
      .changeSiteItems(chosen.id, { action, selector: { kind: 'picked', items: picked, itemSets: [] } })
      .then(
        (done) => {
          setMessage(t('items.bulkDone', { site: chosen.title, total: done.total }));
          setMode(null);
          setBusy(false);
        },
        (error: unknown) => {
          setFailure(toError(error));
          setBusy(false);
        },
      );
  };

  return (
    <div className="bulk-site-actions">
      <p className="bulk-site-row">
        <span role="status">{t('items.selected', { count: picked.length })}</span>
        <button
          type="button"
          disabled={picked.length === 0}
          aria-expanded={mode === 'add'}
          onClick={() => {
            setMode('add');
          }}
        >
          {t('items.bulkAdd')}
        </button>
        <button
          type="button"
          disabled={picked.length === 0}
          aria-expanded={mode === 'remove'}
          onClick={() => {
            setMode('remove');
          }}
        >
          {t('items.bulkRemove')}
        </button>
      </p>
      {mode === null ? null : (
        <p className="bulk-site-row">
          <label htmlFor="bulk-site">{t('items.bulkSite')}</label>
          <select
            id="bulk-site"
            value={siteId === null ? '' : String(siteId)}
            onChange={(event) => {
              setSiteId(options.find((entry) => String(entry.id) === event.target.value)?.id ?? null);
            }}
          >
            <option value="">{t('items.bulkChooseSite')}</option>
            {options.map((entry) => (
              <option key={String(entry.id)} value={String(entry.id)}>
                {entry.title}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={busy || site === null}
            onClick={() => {
              if (site !== null) {
                confirm(site, mode);
              }
            }}
          >
            {site === null
              ? t(mode === 'add' ? 'items.bulkAdd' : 'items.bulkRemove')
              : t(mode === 'add' ? 'items.bulkConfirmAdd' : 'items.bulkConfirmRemove', {
                  count: picked.length,
                  site: site.title,
                })}
          </button>
          <button
            type="button"
            className="link-button"
            onClick={() => {
              setMode(null);
            }}
          >
            {t('action.cancel')}
          </button>
        </p>
      )}
      {message === '' ? null : <p role="status">{message}</p>}
      {failure === null ? null : <ErrorNotice error={failure} />}
    </div>
  );
};
