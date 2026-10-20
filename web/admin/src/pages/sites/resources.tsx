/**
 * The site's Items page: what is on the site, how to add more, and whether the
 * site keeps in sync with its saved search.
 *
 * A site's membership is its `item_site` rows. Upstream edits it through the
 * resources form plus the `UpdateSiteItems` job; this page sends each change to
 * `POST /api/sites/{id}/items`, which runs the same change at once and answers
 * the counts, so every add shows what it will do before it writes. The listed
 * item sets are `o:site_item_set`, the pool written from them is `o:item_pool`,
 * and the sync switch is `o:assign_new_items`, all as upstream stores them.
 * Sync adds an item only when it is created, so an item removed here stays
 * removed.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useRef, useState } from 'react';
import { api } from '../../api/client';
import { encodeSiteSync } from '../../api/write';
import type { ApiPage, Item, Site, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { AccessNote, useSiteAccess } from './SiteFrame';
import { poolKind } from './itemPool';
import type { PoolKind } from './itemPool';
import { PickerPanel, SearchPanel } from './siteItemsAdd';
import type { AddPanelProps } from './siteItemsAdd';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';
import './siteItems.css';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof sites>;

const PER_PAGE = 20;

const TABS: readonly {
  readonly id: string;
  readonly label: MessageId;
  readonly Panel: (props: AddPanelProps) => ReactElement;
}[] = [
  { id: 'pick', label: 'siteItems.byPicker', Panel: PickerPanel },
  { id: 'search', label: 'siteItems.bySearch', Panel: SearchPanel },
];

/** What the sync switch says joins the site, by what the pool selects. */
const SYNC_HELP: Readonly<Record<PoolKind, MessageId>> = {
  every: 'siteItems.syncEvery',
  sets: 'siteItems.syncSets',
  search: 'siteItems.syncSearch',
};

/** The switch that stores `o:assign_new_items`, with what it adds for this site's pool. */
const SyncSwitch = ({
  site,
  disabled,
  onToggle,
}: {
  readonly site: Site;
  readonly disabled: boolean;
  readonly onToggle: (on: boolean) => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  return (
    <div className="site-items-sync">
      <label htmlFor="site-items-sync">
        <input
          id="site-items-sync"
          type="checkbox"
          role="switch"
          checked={site.assignNewItems}
          disabled={disabled}
          aria-describedby="site-items-sync-help"
          onChange={(event) => {
            onToggle(event.target.checked);
          }}
        />
        {t('siteItems.sync')}
      </label>
      <p id="site-items-sync-help" className="field-note">
        {t(SYNC_HELP[poolKind(site.itemPool)])}
      </p>
    </div>
  );
};

/** The add panels behind one tab list. */
const AddTabs = (props: AddPanelProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [current, setCurrent] = useState(0);
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const select = (at: number): void => {
    setCurrent(at);
    tabs.current[at]?.focus();
  };
  const onKey = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    if (step !== 0) {
      event.preventDefault();
      select((current + step + TABS.length) % TABS.length);
    }
  };
  const tab = TABS[current];
  return (
    <section className="site-items-add" aria-labelledby="site-items-add-heading">
      <h3 id="site-items-add-heading">{t('siteItems.addHeading')}</h3>
      <div role="tablist" aria-label={t('siteItems.addHeading')} className="site-items-tabs">
        {TABS.map((entry, at) => (
          <button
            key={entry.id}
            type="button"
            role="tab"
            id={`site-items-tab-${entry.id}`}
            aria-selected={at === current}
            aria-controls={`site-items-panel-${entry.id}`}
            tabIndex={at === current ? 0 : -1}
            ref={(element) => {
              tabs.current[at] = element;
            }}
            onClick={() => {
              select(at);
            }}
            onKeyDown={onKey}
          >
            {t(entry.label)}
          </button>
        ))}
      </div>
      {tab === undefined ? null : (
        <div
          role="tabpanel"
          id={`site-items-panel-${tab.id}`}
          aria-labelledby={`site-items-tab-${tab.id}`}
        >
          <tab.Panel {...props} />
        </div>
      )}
    </section>
  );
};

/** One page of the items on the site, each with a remove button when the caller may remove. */
const SiteItemsList = ({
  items,
  page,
  lastPage,
  busy,
  onPage,
  onRemove,
}: {
  readonly items: readonly Item[];
  readonly page: number;
  readonly lastPage: number;
  readonly busy: boolean;
  readonly onPage: (page: number) => void;
  readonly onRemove: ((item: Item) => void) | null;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  if (items.length === 0) {
    return <p>{t('siteItems.empty')}</p>;
  }
  return (
    <>
      <ul className="site-items-list" aria-label={t('siteItems.listHeading')}>
        {items.map((item) => {
          const title = item.title ?? t('common.untitled');
          const thumbnail = item.thumbnailDisplayUrls.square;
          return (
            <li key={String(item.id)}>
              {thumbnail === undefined ? (
                <span className="site-items-thumb" aria-hidden="true" />
              ) : (
                <img className="site-items-thumb" src={thumbnail} alt="" />
              )}
              <a href={`#/items/${String(item.id)}`}>{title}</a>
              {onRemove === null ? null : (
                <button
                  type="button"
                  className="link-button"
                  disabled={busy}
                  aria-label={t('siteItems.removeLabel', { title })}
                  onClick={() => {
                    onRemove(item);
                  }}
                >
                  {t('action.remove')}
                </button>
              )}
            </li>
          );
        })}
      </ul>
      {lastPage > 1 ? (
        <p className="site-items-pager">
          <button
            type="button"
            disabled={page <= 1}
            onClick={() => {
              onPage(page - 1);
            }}
          >
            {t('action.previous')}
          </button>
          <button
            type="button"
            disabled={page >= lastPage}
            onClick={() => {
              onPage(page + 1);
            }}
          >
            {t('action.next')}
          </button>
        </p>
      ) : null}
    </>
  );
};

const SiteItemsPane = ({
  initial,
  canChange,
}: {
  readonly initial: Site;
  readonly canChange: boolean;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [site, setSite] = useState(initial);
  const [version, setVersion] = useState(0);
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [failure, setFailure] = useState<Error | null>(null);
  const listing = useAsync(
    () => api.listItems({ siteId: site.id, page, perPage: PER_PAGE, sortBy: 'title' }),
    [site.id, page, version],
  );
  // The last page read stays on screen while the next one loads, so a change
  // does not blank the count and the list.
  const lastPage = useRef<ApiPage<Item> | null>(null);
  if (listing.status === 'ready') {
    lastPage.current = listing.data;
  }
  const shown = lastPage.current;

  const run = (task: () => Promise<string>): void => {
    setBusy(true);
    setFailure(null);
    task().then(
      (done) => {
        setMessage(done);
        setVersion((current) => current + 1);
        setBusy(false);
      },
      (error: unknown) => {
        setFailure(toError(error));
        setBusy(false);
      },
    );
  };
  const toggleSync = (on: boolean): void => {
    run(async () => {
      setSite(await api.patchSite(site.id, encodeSiteSync(on)));
      return t('field.saved');
    });
  };
  const remove = (item: Item): void => {
    run(async () => {
      await api.changeSiteItems(site.id, {
        action: 'remove',
        selector: { kind: 'picked', items: [item.id], itemSets: [] },
      });
      return t('siteItems.removed', { title: item.title ?? t('common.untitled') });
    });
  };

  return (
    <div className="site-items">
      <header className="site-items-header">
        <p className="site-items-total" role="status">
          {shown === null ? t('siteItems.counting') : t('siteItems.count', { count: shown.total })}
        </p>
        <SyncSwitch site={site} disabled={busy || !canChange} onToggle={toggleSync} />
      </header>
      {canChange ? null : <AccessNote>{t('siteAccess.items')}</AccessNote>}
      <p className="site-items-message" role="status">
        {message}
      </p>
      {failure === null ? null : <ErrorNotice error={failure} />}
      {canChange ? (
        <AddTabs site={site} version={version} busy={busy} run={run} onSite={setSite} />
      ) : null}
      <section aria-labelledby="site-items-list-heading">
        <h3 id="site-items-list-heading">{t('siteItems.listHeading')}</h3>
        {listing.status === 'error' ? (
          <ErrorNotice error={listing.error} />
        ) : shown === null ? (
          <p role="status">{t('siteItems.counting')}</p>
        ) : (
          <SiteItemsList
            items={shown.items}
            page={shown.page}
            lastPage={shown.lastPage}
            busy={busy}
            onPage={setPage}
            onRemove={canChange ? remove : null}
          />
        )}
      </section>
    </div>
  );
};

/** Loads the site, then renders its Items page. */
export const SiteResourcesPage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(() => api.getSite(siteId), [siteId]);
  const access = useSiteAccess();
  if (state.status === 'loading' || access === null) {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <SiteItemsPane
      key={String(siteId)}
      initial={state.data}
      canChange={access.has('changeSite')}
    />
  );
};
