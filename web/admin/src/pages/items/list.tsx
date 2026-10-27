/**
 * `Admin/ItemController::browseAction`: the Items browse screen. The query
 * lives in the URL (`browse.ts`), so a reload, a bookmark or a link sent to a
 * colleague opens the same slice. The option lists load once; a page, filter or
 * sort change is one `/api/items` request.
 */
import type { ReactElement, RefObject } from 'react';
import { useEffect, useRef, useState } from 'react';
import { api } from '../../api/client';
import type { ApiPage, Item, ItemId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { Pager } from '../../app/list';
import { hashQuery, parseHash, replaceHashQuery } from '../../app/router';
import { useAsync } from '../../app/useAsync';
import type { AsyncState } from '../../app/useAsync';
import { useSettled } from '../../app/useSettled';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { BulkEditBar } from './BulkEditBar';
import { BulkSiteActions } from './BulkSiteActions';
import { activeFilters, browseQuery, formatBrowse, parseBrowse, withFilters, withoutFilter, withoutFilters } from './browse';
import type { BrowseFilters, ItemBrowse, View } from './browse';
import { loadBrowseOptions, NO_OPTIONS } from './browse-options';
import { FilterBar } from './FilterBar';
import { FilterChips } from './FilterChips';
import { ItemsGrid } from './ItemsGrid';
import { ItemsTable } from './ItemsTable';
import { SetCard, SetEmpty, useSetPlace } from './set-card';
import type { ItemsViewProps } from './ItemsTable';
import './browse.css';

const VIEW_KEY = 'romeka.items.view';
const SEARCH_SETTLE_MS = 300;

/** The view this reader chose last, or the table. Storage may be absent or refuse. */
const rememberedView = (): View => {
  try {
    return window.localStorage.getItem(VIEW_KEY) === 'grid' ? 'grid' : 'table';
  } catch {
    return 'table';
  }
};

const rememberView = (view: View): void => {
  try {
    window.localStorage.setItem(VIEW_KEY, view);
  } catch {
    // A refused write only loses the default for next time.
  }
};

/** The last page that loaded, kept on screen while the next one loads. */
const useLastPage = <T,>(state: AsyncState<T>): T | null => {
  const [last, setLast] = useState<T | null>(null);
  if (state.status === 'ready' && state.data !== last) {
    setLast(state.data);
  }
  return state.status === 'ready' ? state.data : last;
};

/** `/` moves focus to the search box, unless the reader is typing in a field. */
const useSlashFocus = (target: RefObject<HTMLInputElement | null>): void => {
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const at = event.target;
      const typing =
        at instanceof HTMLElement &&
        (at.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(at.tagName));
      if (event.key === '/' && !typing && !event.ctrlKey && !event.metaKey && !event.altKey) {
        event.preventDefault();
        target.current?.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
    };
  }, [target]);
};

/** The browse state, its URL, and the search box draft that settles into it. */
const useBrowseState = () => {
  const [state, setState] = useState<ItemBrowse>(() =>
    parseBrowse(hashQuery(window.location.hash), rememberedView()),
  );
  const [draft, setDraft] = useState(state.filters.search);
  const settled = useSettled(draft, SEARCH_SETTLE_MS);

  useEffect(() => {
    replaceHashQuery('items', formatBrowse(state));
  }, [state]);

  useEffect(() => {
    const search = settled.trim();
    setState((current) => (current.filters.search === search ? current : withFilters(current, { search })));
  }, [settled]);

  // A link to another slice while the list is open (the nav's Items link, an
  // address typed by hand) changes only the hash, so the list reads it again.
  useEffect(() => {
    const onHash = (): void => {
      const route = parseHash(window.location.hash);
      if (route?.page !== 'items' || route.param !== null) {
        return;
      }
      const next = parseBrowse(hashQuery(window.location.hash), rememberedView());
      setState(next);
      setDraft(next.filters.search);
    };
    window.addEventListener('hashchange', onHash);
    return () => {
      window.removeEventListener('hashchange', onHash);
    };
  }, []);

  /**
   * Every change but typing. A change that sets the search puts its text in the
   * box; any other change leaves text the reader is still typing alone.
   */
  const apply = (next: ItemBrowse): void => {
    if (next.filters.search !== state.filters.search) {
      setDraft(next.filters.search);
    }
    setState(next);
  };
  return { state, draft, setDraft, apply };
};

/** Why the list is empty: a set with no items yet, filters that match nothing, or no items at all. */
type Emptiness = 'set' | 'filtered' | 'none';

const EmptyList = ({ emptiness, onClearAll }: { readonly emptiness: Emptiness; readonly onClearAll: () => void }): ReactElement => {
  const { t } = useTranslator(common, resources);
  if (emptiness === 'set') {
    return <SetEmpty />;
  }
  return emptiness === 'filtered' ? (
    <p className="items-empty">
      {t('browse.noMatch')}{' '}
      <button type="button" onClick={onClearAll}>
        {t('browse.clearAll')}
      </button>
    </p>
  ) : (
    <p className="items-empty">
      {t('browse.noItems')} <a href="#/items/new">{t('items.add')}</a>{' '}
      <a href="#/import">{t('browse.import')}</a>
    </p>
  );
};

interface ResultsProps {
  readonly page: ApiPage<Item>;
  /** The next page is loading; this one stays on screen, dimmed. */
  readonly busy: boolean;
  readonly view: View;
  /** The match count line; the set card states it in the set's place. */
  readonly showCount: boolean;
  readonly emptiness: Emptiness;
  readonly viewProps: Omit<ItemsViewProps, 'items'>;
  readonly onClearAll: () => void;
  readonly onPage: (page: number) => void;
}

const Results = ({ page, busy, view, showCount, emptiness, viewProps, onClearAll, onPage }: ResultsProps): ReactElement => {
  const { t, number } = useTranslator(common, resources);
  return (
    <div className="items-results" aria-busy={busy}>
      {showCount ? (
        <p className="items-count" aria-live="polite">
          {page.total === 1 ? t('browse.matchOne') : t('browse.matchMany', { count: number(page.total) })}
        </p>
      ) : null}
      {page.items.length === 0 ? (
        <EmptyList emptiness={emptiness} onClearAll={onClearAll} />
      ) : view === 'grid' ? (
        <ItemsGrid items={page.items} {...viewProps} />
      ) : (
        <ItemsTable items={page.items} {...viewProps} />
      )}
      <Pager page={page.page} lastPage={page.lastPage} total={page.total} label={t('resource.items')} onPage={onPage} />
    </div>
  );
};

export const ItemsList = (): ReactElement => {
  const { t } = useTranslator(common, resources);
  const { state, draft, setDraft, apply } = useBrowseState();
  const [picked, setPicked] = useState<readonly ItemId[]>([]);
  const searchRef = useRef<HTMLInputElement>(null);
  useSlashFocus(searchRef);

  const optionsState = useAsync(loadBrowseOptions, []);
  const options = optionsState.status === 'ready' ? optionsState.data : NO_OPTIONS;
  const query = browseQuery(state);
  /** Bumped when a bulk change ends, so the page shows what it wrote. */
  const [revision, setRevision] = useState(0);
  const itemsState = useAsync(() => api.listItems(query), [JSON.stringify(query), revision]);
  const shown: ApiPage<Item> | null = useLastPage(itemsState);

  const active = activeFilters(state.filters);
  const filtered = active.length > 0;
  const setId = state.filters.itemSetId;
  const place = useSetPlace(setId, revision);
  /** Filters besides the set narrow the set's items. */
  const narrowed = active.some((key) => key !== 'itemSetId');
  const emptiness: Emptiness = setId !== null && !narrowed ? 'set' : filtered ? 'filtered' : 'none';
  const hidden = place.status === 'unavailable';
  const onFilter = (change: Partial<BrowseFilters>): void => {
    apply(withFilters(state, change));
  };
  const onClearAll = (): void => {
    apply(withoutFilters(state));
    setDraft('');
  };
  const pageIds = shown?.items.map((item) => item.id) ?? [];
  const viewProps = {
    options,
    picked,
    onPick: (id: ItemId, on: boolean) => {
      setPicked((current) => (on ? [...current, id] : current.filter((entry) => entry !== id)));
    },
    onPickPage: (on: boolean) => {
      setPicked((current) => [...current.filter((id) => !pageIds.includes(id)), ...(on ? pageIds : [])]);
    },
    onFilter,
  };

  return (
    <section className="items-browse">
      <SetCard place={place} matching={narrowed && shown !== null ? shown.total : null} />
      <p>
        <a href="#/items/new">{t('items.add')}</a>
      </p>
      <FilterBar
        state={state}
        draft={draft}
        searchRef={searchRef}
        options={options}
        onDraft={setDraft}
        onClearSearch={() => {
          apply(withFilters(state, { search: '' }));
          setDraft('');
        }}
        onFilter={onFilter}
        onSort={(sort) => {
          apply({ ...state, sort, page: 1 });
        }}
        onView={(view) => {
          rememberView(view);
          apply({ ...state, view });
        }}
      />
      {/* A refused list says it once; a refused option list alone still leaves the rows. */}
      {optionsState.status === 'error' && itemsState.status !== 'error' ? (
        <ErrorNotice error={optionsState.error} />
      ) : null}
      <FilterChips
        filters={state.filters}
        options={options}
        onRemove={(key) => {
          apply(withoutFilter(state, key));
        }}
        onClearAll={onClearAll}
      />
      {hidden ? null : <BulkSiteActions picked={picked} />}
      {shown === null || hidden ? null : (
        <BulkEditBar
          picked={picked}
          query={query}
          total={shown.total}
          filtered={filtered}
          options={options}
          onChanged={() => {
            setRevision((current) => current + 1);
          }}
        />
      )}
      {hidden ? null : itemsState.status === 'error' ? (
        <ErrorNotice error={itemsState.error} />
      ) : shown === null ? (
        <p role="status">{t('list.loading', { resource: t('resource.items') })}</p>
      ) : (
        <Results
          page={shown}
          busy={itemsState.status === 'loading'}
          view={state.view}
          showCount={filtered && setId === null}
          emptiness={emptiness}
          viewProps={viewProps}
          onClearAll={onClearAll}
          onPage={(page) => {
            apply({ ...state, page });
          }}
        />
      )}
    </section>
  );
};
