/**
 * The site resources page (`SiteAdmin/IndexController::resourcesAction`).
 *
 * Upstream sets the stored item pool from a search query and attaches item
 * sets, then dispatches `Omeka\Job\UpdateSiteItems` to add, replace or remove
 * the site's items. The port has no job dispatch over the API, so this page
 * edits the two members the API accepts: `o:item_pool` (the query the site
 * browse uses) and `o:site_item_set` (the item sets the site shows). It also
 * shows the item count the site currently holds, which is what upstream prints
 * after the job runs.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { parseJsonObjectOrArrayText } from '../../api/parse';
import { encodeSiteResources } from '../../api/write';
import type { ItemSet, ItemSetId, Site, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow } from '../../app/fields';
import { formatJson } from '../../app/json';
import { useSave } from '../../app/save';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';

interface Loaded {
  readonly site: Site;
  readonly itemSets: readonly ItemSet[];
  readonly itemCount: number;
}

const load = async (siteId: SiteId): Promise<Loaded> => {
  const [site, itemSets] = await Promise.all([
    api.getSite(siteId),
    api.listItemSets({ perPage: 200, sortBy: 'title' }),
  ]);
  const siteItems = await api.listItems({ siteId, perPage: 1 });
  return { site, itemSets: itemSets.items, itemCount: siteItems.total };
};

const SiteResourcesEditor = ({
  siteId,
  loaded,
}: {
  readonly siteId: SiteId;
  readonly loaded: Loaded;
}): ReactElement => {
  const { t } = useTranslator();
  const [itemPool, setItemPool] = useState(() => formatJson(loaded.site.itemPool ?? []));
  const [attached, setAttached] = useState<readonly ItemSetId[]>(() =>
    loaded.site.siteItemSets.map((entry) => entry.itemSet.id),
  );
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const toggle = (id: ItemSetId, on: boolean): void => {
    setAttached((current) => (on ? [...current, id] : current.filter((entry) => entry !== id)));
  };

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    let pool;
    try {
      pool = parseJsonObjectOrArrayText(itemPool);
    } catch (error) {
      setFormError(`${t('sites.itemPool')}: ${toError(error).message}`);
      return;
    }
    setFormError(null);
    save(async () => {
      await api.patchSite(siteId, encodeSiteResources(pool, attached));
    });
  };

  return (
    <form onSubmit={submit}>
      <p role="status">{t('sites.itemCount', { count: loaded.itemCount })}</p>
      <p>
        <label htmlFor="site-item-pool">{t('sites.itemPool')}</label>
        <textarea
          id="site-item-pool"
          rows={4}
          value={itemPool}
          onChange={(event) => {
            setItemPool(event.target.value);
          }}
        />
      </p>
      <fieldset>
        <legend>{t('sites.itemSets')}</legend>
        {loaded.itemSets.length === 0 ? <p>{t('sites.noItemSets')}</p> : null}
        {loaded.itemSets.map((itemSet) => (
          <p key={String(itemSet.id)}>
            <label htmlFor={`site-item-set-${String(itemSet.id)}`}>
              <input
                id={`site-item-set-${String(itemSet.id)}`}
                type="checkbox"
                checked={attached.includes(itemSet.id)}
                onChange={(event) => {
                  toggle(itemSet.id, event.target.checked);
                }}
              />
              {itemSet.title ?? t('common.pageFallback', { id: itemSet.id })}
            </label>
          </p>
        ))}
      </fieldset>
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};

/** Loads the resources the page edits, then renders the editor. */
export const SiteResourcesPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <SiteResourcesEditor key={String(siteId)} siteId={siteId} loaded={state.data} />;
};
