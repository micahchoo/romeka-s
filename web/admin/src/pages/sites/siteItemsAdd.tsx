/**
 * The two ways the site's Items page adds items: the item picker, where a
 * curator picks items and whole item sets, and a search. Each one builds a
 * membership command and shows what it would do, from a dry run of
 * `POST /api/sites/{id}/items`, before the button writes it.
 *
 * The picked item sets are the site's item sets (`o:site_item_set`), and the
 * site's pool (`o:item_pool`) is written from them as an `item_set_id` query,
 * so "Keep this site in sync" adds the items later created in those sets. A
 * search adds its matches once and leaves the pool alone.
 */
import type { ReactElement } from 'react';
import { useMemo, useState } from 'react';
import { api } from '../../api/client';
import { encodeSiteItemPool, encodeSiteItemSets, encodeSiteSync } from '../../api/write';
import type {
  ItemId,
  ItemSet,
  ItemSetId,
  Site,
  SiteItemsChange,
  SiteItemsCommand,
} from '../../api/types';
import { useAsync } from '../../app/useAsync';
import { useSettled } from '../../app/useSettled';
import { useTranslator } from '../../i18n';
import { ItemQueryField } from './editor/fields/ItemQueryField';
import { LazyItemPicker } from './editor/fields/LazyItemPicker';
import { formatQuery, parseQuery, queryValue, withQueryValue } from './editor/fields/query';
import { itemPoolFromQuery, itemSetPool } from './itemPool';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

const PREVIEW_DELAY_MS = 300;
const PUBLIC_KEY = 'is_public';

/** What a panel hands the page: a write that resolves to the message to announce. */
export type RunChange = (task: () => Promise<string>) => void;

export interface AddPanelProps {
  readonly site: Site;
  /** Bumped after every write, so a preview counts against the new membership. */
  readonly version: number;
  readonly busy: boolean;
  readonly run: RunChange;
  readonly onSite: (site: Site) => void;
}

/**
 * The dry run of one command, read once the command stops changing. `null`
 * while there is nothing to count or the count is on its way.
 */
const usePreview = (
  site: Site,
  command: SiteItemsCommand | null,
  version: number,
): { readonly change: SiteItemsChange | null; readonly error: Error | null } => {
  const key = command === null ? '' : JSON.stringify(command);
  const settled = useSettled(command, PREVIEW_DELAY_MS);
  const settledKey = settled === null ? '' : JSON.stringify(settled);
  const preview = useAsync(
    async () => (settled === null ? null : api.changeSiteItems(site.id, settled, true)),
    [site.id, settledKey, version],
  );
  if (key !== settledKey || preview.status === 'loading') {
    return { change: null, error: null };
  }
  return preview.status === 'error'
    ? { change: null, error: preview.error }
    : { change: preview.data, error: null };
};

/** The line under a panel that says what its button would do. */
const PreviewLine = ({
  change,
  error,
  idle,
  describe,
}: {
  readonly change: SiteItemsChange | null;
  readonly error: Error | null;
  readonly idle: boolean;
  readonly describe: (change: SiteItemsChange) => string;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  return (
    <p className="site-items-preview" role="status">
      {idle
        ? null
        : error !== null
          ? t('siteItems.countError', { message: error.message })
          : change === null
            ? t('siteItems.counting')
            : describe(change)}
    </p>
  );
};

/**
 * The site's item sets and the pool written from them, in one write. With no
 * set left the pool is empty, which would let every new item join, so the same
 * write turns sync off.
 */
const saveSets = async (site: Site, sets: readonly ItemSetId[]): Promise<Site> =>
  api.patchSite(site.id, {
    ...encodeSiteItemSets(sets),
    ...encodeSiteItemPool(itemSetPool(sets)),
    ...(sets.length === 0 ? encodeSiteSync(false) : {}),
  });

const attachedSetIds = (site: Site): readonly ItemSetId[] =>
  site.siteItemSets.map((entry) => entry.itemSet.id);

/** The item sets on the site, each with a Detach button. */
const AttachedSets = ({
  site,
  busy,
  run,
  onSite,
}: AddPanelProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const attached = attachedSetIds(site);
  const key = attached.join(',');
  const listed = useAsync(
    async (): Promise<readonly ItemSet[]> =>
      attached.length === 0
        ? []
        : (await api.listItemSets({ id: key, perPage: attached.length })).items,
    [key],
  );
  const label = (set: ItemSet): string => set.title ?? t('common.untitled');
  const detach = (set: ItemSet): void => {
    run(async () => {
      onSite(await saveSets(site, attached.filter((id) => id !== set.id)));
      return t('field.saved');
    });
  };
  return (
    <section className="site-items-attached-sets" aria-labelledby="site-items-sets-heading">
      <h4 id="site-items-sets-heading">{t('siteItems.attachedSets')}</h4>
      {attached.length === 0 ? (
        <p className="field-note">{t('siteItems.noSetsYet')}</p>
      ) : (
        <>
          <p className="field-note">{t('siteItems.detachHelp')}</p>
          <ul className="site-items-attached">
            {(listed.status === 'ready' ? listed.data : []).map((set) => (
              <li key={String(set.id)}>
                <span>{label(set)}</span>
                <button
                  type="button"
                  className="link-button"
                  disabled={busy}
                  aria-label={t('siteItems.detachLabel', { title: label(set) })}
                  onClick={() => {
                    detach(set);
                  }}
                >
                  {t('siteItems.detach')}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
};

/**
 * Items and sets: pick items and whole item sets, then add them or make them
 * the site's whole membership. A picked set becomes one of the site's sets.
 */
export const PickerPanel = (props: AddPanelProps): ReactElement => {
  const { site, version, busy, run, onSite } = props;
  const { t } = useTranslator(common, sites);
  const [items, setItems] = useState<readonly ItemId[]>([]);
  const [sets, setSets] = useState<readonly ItemSetId[]>([]);
  const [confirming, setConfirming] = useState(false);
  const nothing = items.length === 0 && sets.length === 0;
  const command = useMemo<SiteItemsCommand | null>(
    () => (nothing ? null : { action: 'add', selector: { kind: 'picked', items, itemSets: sets } }),
    [nothing, items, sets],
  );
  const replaceCommand = useMemo<SiteItemsCommand | null>(
    () =>
      command === null || !confirming ? null : { action: 'replace', selector: command.selector },
    [command, confirming],
  );
  const { change, error } = usePreview(site, command, version);
  const replacing = usePreview(site, replaceCommand, version);
  const attached = attachedSetIds(site);
  const newSets = sets.filter((set) => !attached.includes(set));

  const clear = (): void => {
    setItems([]);
    setSets([]);
    setConfirming(false);
  };
  const add = (): void => {
    if (command === null) {
      return;
    }
    run(async () => {
      if (newSets.length > 0) {
        onSite(await saveSets(site, [...attached, ...newSets]));
      }
      const done = await api.changeSiteItems(site.id, command);
      clear();
      return t('siteItems.added', { count: done.added });
    });
  };
  const replace = (): void => {
    if (replaceCommand === null) {
      return;
    }
    run(async () => {
      onSite(await saveSets(site, sets));
      const done = await api.changeSiteItems(site.id, replaceCommand);
      clear();
      return t('siteItems.replaced', { total: done.total });
    });
  };

  return (
    <div className="site-items-panel">
      <LazyItemPicker
        id="site-item-picker"
        label={t('siteItems.pickLabel')}
        mode="many"
        picked={items}
        onChange={(next) => {
          setItems(next);
          setConfirming(false);
        }}
        sets={{
          picked: sets,
          onChange: (next) => {
            setSets(next);
            setConfirming(false);
          },
        }}
      />
      <PreviewLine
        change={change}
        error={error}
        idle={nothing}
        describe={(counted) =>
          t('siteItems.pickPreview', { count: counted.matched, added: counted.added })
        }
      />
      <p className="form-actions">
        <button
          type="button"
          className="site-items-apply"
          disabled={busy || change === null || (change.added === 0 && newSets.length === 0)}
          onClick={add}
        >
          {t('siteItems.addCount', { count: change?.added ?? 0 })}
        </button>
        <button
          type="button"
          disabled={busy || change === null}
          aria-expanded={confirming}
          onClick={() => {
            setConfirming(true);
          }}
        >
          {t('siteItems.replace')}
        </button>
      </p>
      {!confirming ? null : (
        <div className="site-items-confirm" role="group" aria-label={t('siteItems.replace')}>
          <PreviewLine
            change={replacing.change}
            error={replacing.error}
            idle={false}
            describe={(counted) =>
              t('siteItems.replaceConfirm', {
                matched: counted.matched,
                removed: counted.removed,
                added: counted.added,
              })
            }
          />
          <p className="form-actions">
            <button
              type="button"
              className="site-items-apply"
              disabled={busy || replacing.change === null}
              onClick={replace}
            >
              {t('siteItems.replaceYes')}
            </button>
            <button
              type="button"
              className="link-button"
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t('action.cancel')}
            </button>
          </p>
        </div>
      )}
      <AttachedSets {...props} />
    </div>
  );
};

/** The query with `is_public=1` set or dropped. */
const withPublicOnly = (query: string, on: boolean): string => {
  const pairs = parseQuery(query);
  return formatQuery(
    on ? withQueryValue(pairs, PUBLIC_KEY, '1') : pairs.filter((pair) => pair.key !== PUBLIC_KEY),
  );
};

/** By search: what a search matches, and a button that adds the items the site lacks. */
export const SearchPanel = ({ site, version, busy, run }: AddPanelProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [query, setQuery] = useState('');
  const pool = useMemo(() => itemPoolFromQuery(query), [query]);
  const command = useMemo<SiteItemsCommand>(
    () => ({ action: 'add', selector: { kind: 'query', query: pool } }),
    [pool],
  );
  const { change, error } = usePreview(site, command, version);
  const publicOnly = queryValue(parseQuery(query), PUBLIC_KEY) === '1';

  const add = (): void => {
    run(async () => {
      const done = await api.changeSiteItems(site.id, command);
      return t('siteItems.added', { count: done.added });
    });
  };

  return (
    <div className="site-items-panel">
      <ItemQueryField
        id="site-item-search"
        label={t('siteItems.searchLegend')}
        value={query}
        onChange={setQuery}
        sortable={false}
        showCount={false}
      />
      <label className="site-items-check" htmlFor="site-search-public">
        <input
          id="site-search-public"
          type="checkbox"
          checked={publicOnly}
          aria-describedby="site-search-public-help"
          onChange={(event) => {
            setQuery(withPublicOnly(query, event.target.checked));
          }}
        />
        {t('siteItems.publicOnly')}
      </label>
      <p id="site-search-public-help" className="field-note">
        {t('siteItems.publicOnlyHelp')}
      </p>
      <PreviewLine
        change={change}
        error={error}
        idle={false}
        describe={(counted) =>
          t('siteItems.searchPreview', { matched: counted.matched, added: counted.added })
        }
      />
      <p className="field-note">{t('siteItems.searchOnce')}</p>
      <p className="form-actions">
        <button
          type="button"
          className="site-items-apply"
          disabled={busy || change === null || change.added === 0}
          onClick={add}
        >
          {t('siteItems.addCount', { count: change?.added ?? 0 })}
        </button>
      </p>
    </div>
  );
};
