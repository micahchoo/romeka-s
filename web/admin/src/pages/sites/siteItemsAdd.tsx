/**
 * The three ways the site's Items page adds items: by item set, by search, and
 * by picking. Each one builds a membership command and shows what it would do,
 * from a dry run of `POST /api/sites/{id}/items`, before the button writes it.
 */
import type { ReactElement } from 'react';
import { useMemo, useState } from 'react';
import { api } from '../../api/client';
import { encodeSiteItemPool, encodeSiteItemSets } from '../../api/write';
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
import { ItemPicker } from './editor/fields/ItemPicker';
import { ItemQueryField } from './editor/fields/ItemQueryField';
import { formatQuery, parseQuery, queryValue, withQueryValue } from './editor/fields/query';
import { itemPoolFromQuery, itemPoolQuery } from './itemPool';

const PREVIEW_DELAY_MS = 300;
const SET_RESULTS = 20;
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
  const { t } = useTranslator();
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

/** By item set: find a set, see what it would add, attach it with its items. */
export const ItemSetPanel = ({ site, version, busy, run, onSite }: AddPanelProps): ReactElement => {
  const { t } = useTranslator();
  const [term, setTerm] = useState('');
  const [chosen, setChosen] = useState<ItemSet | null>(null);
  const [withItems, setWithItems] = useState(true);
  const settledTerm = useSettled(term.trim(), PREVIEW_DELAY_MS);
  const attached = site.siteItemSets.map((entry) => entry.itemSet.id);
  const attachedKey = attached.join(',');
  const results = useAsync(
    async () =>
      (
        await api.listItemSets({
          perPage: SET_RESULTS,
          sortBy: 'title',
          ...(settledTerm === '' ? {} : { search: settledTerm }),
        })
      ).items,
    [settledTerm],
  );
  const attachedSets = useAsync(
    async (): Promise<readonly ItemSet[]> =>
      attached.length === 0
        ? []
        : (await api.listItemSets({ id: attachedKey, perPage: attached.length })).items,
    [attachedKey],
  );
  const command = useMemo<SiteItemsCommand | null>(
    () =>
      chosen === null ? null : { action: 'add', selector: { kind: 'itemSets', itemSets: [chosen.id] } },
    [chosen],
  );
  const { change, error } = usePreview(site, command, version);
  const isAttached = chosen !== null && attached.includes(chosen.id);
  const adds = withItems && change !== null && change.added > 0;
  const label = (title: string | null): string => title ?? t('common.untitled');

  const saveSets = async (ids: readonly ItemSetId[]): Promise<void> => {
    onSite(await api.patchSite(site.id, encodeSiteItemSets(ids)));
  };
  const attach = (set: ItemSet): void => {
    run(async () => {
      if (!attached.includes(set.id)) {
        await saveSets([...attached, set.id]);
      }
      if (!withItems) {
        return t('field.saved');
      }
      const done = await api.changeSiteItems(site.id, {
        action: 'add',
        selector: { kind: 'itemSets', itemSets: [set.id] },
      });
      return t('siteItems.added', { count: done.added });
    });
  };
  const detach = (set: ItemSet): void => {
    run(async () => {
      await saveSets(attached.filter((id) => id !== set.id));
      return t('field.saved');
    });
  };

  return (
    <div className="site-items-panel">
      <label htmlFor="site-set-search">{t('siteItems.setSearch')}</label>
      <input
        id="site-set-search"
        type="search"
        value={term}
        onChange={(event) => {
          setTerm(event.target.value);
        }}
      />
      {results.status === 'ready' && results.data.length === 0 ? (
        <p className="field-note">{t('siteItems.noSets')}</p>
      ) : null}
      <ul className="site-items-sets" aria-label={t('page.itemSets')}>
        {(results.status === 'ready' ? results.data : []).map((set) => (
          <li key={String(set.id)}>
            <button
              type="button"
              aria-pressed={chosen?.id === set.id}
              onClick={() => {
                setChosen(set);
              }}
            >
              {label(set.title)}
              {attached.includes(set.id) ? (
                <span className="site-items-badge">{t('siteItems.attached')}</span>
              ) : null}
            </button>
          </li>
        ))}
      </ul>
      <label className="site-items-check" htmlFor="site-set-with-items">
        <input
          id="site-set-with-items"
          type="checkbox"
          checked={withItems}
          aria-describedby="site-set-with-items-help"
          onChange={(event) => {
            setWithItems(event.target.checked);
          }}
        />
        {t('siteItems.setWithItems')}
      </label>
      <p id="site-set-with-items-help" className="field-note">
        {t('siteItems.setWithItemsHelp')}
      </p>
      <PreviewLine
        change={change}
        error={error}
        idle={chosen === null}
        describe={(counted) =>
          t('siteItems.setPreview', {
            title: label(chosen?.title ?? null),
            matched: counted.matched,
            added: counted.added,
          })
        }
      />
      {chosen === null ? null : (
        <p className="form-actions">
          <button
            type="button"
            className="site-items-apply"
            disabled={busy || change === null || (isAttached && !adds)}
            onClick={() => {
              attach(chosen);
            }}
          >
            {isAttached
              ? t('siteItems.addCount', { count: change?.added ?? 0 })
              : adds
                ? t('siteItems.attachWithItems', { count: change.added })
                : t('siteItems.attachSet')}
          </button>
        </p>
      )}
      <h4>{t('siteItems.attachedSets')}</h4>
      <p className="field-note">{t('siteItems.detachHelp')}</p>
      <ul className="site-items-attached">
        {(attachedSets.status === 'ready' ? attachedSets.data : []).map((set) => (
          <li key={String(set.id)}>
            <span>{label(set.title)}</span>
            <button
              type="button"
              className="link-button"
              disabled={busy}
              aria-label={t('siteItems.detachLabel', { title: label(set.title) })}
              onClick={() => {
                detach(set);
              }}
            >
              {t('siteItems.detach')}
            </button>
          </li>
        ))}
      </ul>
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

/** By search: the saved search, what it matches, and a button that adds the rest. */
export const SearchPanel = ({ site, version, busy, run, onSite }: AddPanelProps): ReactElement => {
  const { t } = useTranslator();
  const [query, setQuery] = useState(() => itemPoolQuery(site.itemPool));
  const pool = useMemo(() => itemPoolFromQuery(query), [query]);
  const command = useMemo<SiteItemsCommand>(
    () => ({ action: 'add', selector: { kind: 'query', query: pool } }),
    [pool],
  );
  const { change, error } = usePreview(site, command, version);
  const publicOnly = queryValue(parseQuery(query), PUBLIC_KEY) === '1';

  const save = (): void => {
    run(async () => {
      onSite(await api.patchSite(site.id, encodeSiteItemPool(pool)));
      const done = await api.changeSiteItems(site.id, command);
      return done.added === 0
        ? t('field.saved')
        : t('siteItems.added', { count: done.added });
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
      <p className="field-note">{t('siteItems.searchHelp')}</p>
      <p className="form-actions">
        <button
          type="button"
          className="site-items-apply"
          disabled={busy || change === null}
          onClick={save}
        >
          {change !== null && change.added > 0
            ? t('siteItems.addCount', { count: change.added })
            : t('siteItems.saveSearch')}
        </button>
      </p>
    </div>
  );
};

/** Pick items: choose them one by one and add the ones the site lacks. */
export const PickPanel = ({ site, version, busy, run }: AddPanelProps): ReactElement => {
  const { t } = useTranslator();
  const [picked, setPicked] = useState<readonly ItemId[]>([]);
  const command = useMemo<SiteItemsCommand | null>(
    () => (picked.length === 0 ? null : { action: 'add', selector: { kind: 'items', items: picked } }),
    [picked],
  );
  const { change, error } = usePreview(site, command, version);

  const add = (): void => {
    if (command === null) {
      return;
    }
    run(async () => {
      const done = await api.changeSiteItems(site.id, command);
      setPicked([]);
      return t('siteItems.added', { count: done.added });
    });
  };

  return (
    <div className="site-items-panel">
      <ItemPicker id="site-item-picker" label={t('siteItems.pickLabel')} picked={picked} onChange={setPicked} />
      <PreviewLine
        change={change}
        error={error}
        idle={picked.length === 0}
        describe={(counted) =>
          t('siteItems.pickPreview', { count: counted.matched, added: counted.added })
        }
      />
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
