/**
 * The `itemQuery` field control: a builder for the browse query a block stores,
 * over `fulltext_search`, `item_set_id`, `resource_class_id` and the sort pair.
 * It reads and writes the ordered pairs `query.ts` owns, so an advanced-search
 * key this builder does not show survives an edit.
 *
 * The match count comes from `Omeka-S-Total-Results` on the list endpoint of
 * the resource the query searches (`/api/items`, `/api/item_sets` or
 * `/api/media`), which the client reads into `ApiPage.total`; the request asks
 * for one row and only uses the header. Only items can be filtered by item set
 * (`QueryFilter::push_item`), so the item set select shows for items alone.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { ResourceQuery } from '../../../../api/client';
import type { ApiPage, ItemSet, ResourceClass, SiteId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { useSettled } from '../../../../app/useSettled';
import { useTranslator } from '../../../../i18n';
import { useTermLabels } from '../../../../i18n/termText';
import type { KeyOf, MessageKey } from '../../../../i18n';
import { useEditorText } from '../messages';
import type { EditorMessageKey } from '../messages';
import { Labelled } from '../Labelled';
import { QUERY_KEYS, formatQuery, parseQuery, queryId, queryValue, withQueryValue } from './query';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof sites>;

/** How long the builder waits before it asks for a match count. */
const COUNT_DELAY_MS = 400;

/** The sort fields `AbstractResourceEntityAdapter` orders items by, and their labels. */
const SORT_FIELDS: readonly { readonly value: string; readonly label: MessageId | null }[] = [
  { value: 'title', label: 'query.sort_title' },
  { value: 'created', label: 'query.sort_created' },
  { value: 'modified', label: 'query.sort_modified' },
  { value: 'id', label: 'query.sort_id' },
];

/** The resource types a browse query can search (`browsePreview.resource_type`). */
export type QueryResource = 'items' | 'item_sets' | 'media';

export const isQueryResource = (value: unknown): value is QueryResource =>
  value === 'items' || value === 'item_sets' || value === 'media';

/** What differs per resource type: the list it counts and what the count says. */
const RESOURCES: Readonly<
  Record<
    QueryResource,
    {
      readonly list: (query: ResourceQuery) => Promise<ApiPage<unknown>>;
      readonly byItemSet: boolean;
      /** The count line's message: the main catalogue's for items, which the
       * site's Items page also shows, the editor table's otherwise. */
      readonly countMessage: (
        count: number,
        scoped: boolean,
      ) => { readonly main: MessageId } | { readonly editor: EditorMessageKey };
    }
  >
> = {
  items: {
    list: (query) => api.listItems(query),
    byItemSet: true,
    countMessage: (count, scoped) => ({
      main:
        count === 0
          ? scoped
            ? 'query.siteNoMatch'
            : 'query.noMatch'
          : scoped
            ? 'query.siteMatchCount'
            : 'query.matchCount',
    }),
  },
  item_sets: {
    list: (query) => api.listItemSets(query),
    byItemSet: false,
    countMessage: () => ({ editor: 'query.itemSetCount' }),
  },
  media: {
    list: (query) => api.listMedia(query),
    byItemSet: false,
    countMessage: () => ({ editor: 'query.mediaCount' }),
  },
};

export interface ItemQueryFieldProps {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly required?: boolean;
  /** The lists when the editor already loaded them; the control then makes no request. */
  readonly itemSets?: readonly ItemSet[];
  readonly classes?: readonly ResourceClass[];
  /** The site whose items a block shows. The count is scoped to it, because the
   * public render adds `site_id` to the stored query. */
  readonly siteId?: SiteId;
  /** False for a query that selects a set rather than lists one, such as a
   * site's item pool: no order applies, so the sort row is left out. */
  readonly sortable?: boolean;
  /** False when the caller shows its own count, such as the site's Items page,
   * which counts what an add would change; the control then asks for none. */
  readonly showCount?: boolean;
  /** The resource the query searches; items unless a sibling choice says otherwise. */
  readonly resource?: QueryResource;
}

/** The list request one stored query makes: one row, for the total header. */
const countQuery = (
  value: string,
  siteId: SiteId | undefined,
  byItemSet: boolean,
): ResourceQuery => {
  const pairs = parseQuery(value);
  const fulltext = queryValue(pairs, QUERY_KEYS.fulltext).trim();
  const itemSet = queryId(pairs, QUERY_KEYS.itemSet);
  const resourceClass = queryId(pairs, QUERY_KEYS.resourceClass);
  const sortBy = queryValue(pairs, QUERY_KEYS.sortBy).trim();
  const sortOrder = queryValue(pairs, QUERY_KEYS.sortOrder).trim();
  return {
    perPage: 1,
    ...(siteId === undefined ? {} : { site_id: siteId }),
    ...(fulltext === '' ? {} : { fulltext_search: fulltext }),
    ...(itemSet === null || !byItemSet ? {} : { item_set_id: itemSet }),
    ...(resourceClass === null ? {} : { resource_class_id: resourceClass }),
    ...(sortBy === '' ? {} : { sortBy }),
    ...(sortOrder === 'asc' || sortOrder === 'desc' ? { sortOrder } : {}),
  };
};

/** A select over one id list, keeping a stored id the list does not name. */
const IdSelect = ({
  id,
  value,
  options,
  onChange,
}: {
  readonly id: string;
  readonly value: string;
  readonly options: readonly { readonly id: number; readonly label: string }[];
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const named = options.some((option) => String(option.id) === value);
  return (
    <select
      id={id}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
      }}
    >
      <option value="">{t('query.any')}</option>
      {value !== '' && !named ? (
        <option value={value}>{t('query.optionFallback', { id: value })}</option>
      ) : null}
      {options.map((option) => (
        <option key={String(option.id)} value={String(option.id)}>
          {option.label}
        </option>
      ))}
    </select>
  );
};

export const ItemQueryField = ({
  id,
  label,
  value,
  onChange,
  required = false,
  itemSets,
  classes,
  siteId,
  sortable = true,
  showCount = true,
  resource = 'items',
}: ItemQueryFieldProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const text = useTermLabels();
  const e = useEditorText();
  const countText = (
    message: { readonly main: MessageId } | { readonly editor: EditorMessageKey },
    count: number,
  ): string => ('main' in message ? t(message.main, { count }) : e(message.editor, { count }));
  const searched = RESOURCES[resource];
  const pairs = parseQuery(value);
  const fulltext = queryValue(pairs, QUERY_KEYS.fulltext);
  const itemSet = queryValue(pairs, QUERY_KEYS.itemSet);
  const resourceClass = queryValue(pairs, QUERY_KEYS.resourceClass);
  const sortBy = queryValue(pairs, QUERY_KEYS.sortBy);
  const sortOrder = queryValue(pairs, QUERY_KEYS.sortOrder);
  const itemSetState = useAsync(
    async (): Promise<readonly ItemSet[]> =>
      itemSets ?? (await api.listItemSets({ perPage: 200, sortBy: 'title' })).items,
    [itemSets],
  );
  const classState = useAsync(
    async (): Promise<readonly ResourceClass[]> =>
      classes ?? (await api.listResourceClasses({ perPage: 200, sortBy: 'label' })).items,
    [classes],
  );
  const counted = useSettled(value, COUNT_DELAY_MS);
  const matches = useAsync(
    async () =>
      showCount
        ? (await searched.list(countQuery(counted, siteId, searched.byItemSet))).total
        : 0,
    [counted, siteId, showCount, searched],
  );
  const itemSetOptions = itemSets ?? (itemSetState.status === 'ready' ? itemSetState.data : []);
  const classOptions = classes ?? (classState.status === 'ready' ? classState.data : []);

  const set = (key: string, next: string): void => {
    onChange(formatQuery(withQueryValue(pairs, key, next)));
  };
  const sortFields =
    sortBy !== '' && !SORT_FIELDS.some((field) => field.value === sortBy)
      ? [{ value: sortBy, label: null }, ...SORT_FIELDS]
      : SORT_FIELDS;

  return (
    <fieldset className="block-field query-field">
      <legend>{label}</legend>
      <Labelled id={`${id}-fulltext`} label={t('query.fulltext')}>
        <input
          id={`${id}-fulltext`}
          type="search"
          value={fulltext}
          aria-required={required}
          onChange={(event) => {
            set(QUERY_KEYS.fulltext, event.target.value);
          }}
        />
      </Labelled>
      {searched.byItemSet ? (
        <Labelled id={`${id}-item-set`} label={t('query.itemSet')}>
          <IdSelect
            id={`${id}-item-set`}
            value={itemSet}
            onChange={(next) => {
              set(QUERY_KEYS.itemSet, next);
            }}
            options={itemSetOptions.map((entry) => ({
              id: entry.id,
              label: entry.title ?? t('common.untitled'),
            }))}
          />
        </Labelled>
      ) : null}
      <Labelled id={`${id}-class`} label={t('query.resourceClass')}>
        <IdSelect
          id={`${id}-class`}
          value={resourceClass}
          onChange={(next) => {
            set(QUERY_KEYS.resourceClass, next);
          }}
          options={classOptions.map((entry) => ({
            id: entry.id,
            label: text.label(entry.term, entry.label) ?? entry.term,
          }))}
        />
      </Labelled>
      {sortable ? (
        <Labelled id={`${id}-sort-by`} label={t('query.sort')}>
          <div className="block-field-pair">
            <select
              id={`${id}-sort-by`}
              value={sortBy}
              onChange={(event) => {
                set(QUERY_KEYS.sortBy, event.target.value);
              }}
            >
              <option value="">{t('query.sortNone')}</option>
              {sortFields.map((field) => (
                <option key={field.value} value={field.value}>
                  {field.label === null ? t('query.optionFallback', { id: field.value }) : t(field.label)}
                </option>
              ))}
            </select>
            <select
              id={`${id}-sort-order`}
              aria-label={t('query.sortOrder')}
              value={sortOrder}
              onChange={(event) => {
                set(QUERY_KEYS.sortOrder, event.target.value);
              }}
            >
              <option value="">{t('query.sortNone')}</option>
              <option value="asc">{t('query.orderAsc')}</option>
              <option value="desc">{t('query.orderDesc')}</option>
            </select>
          </div>
        </Labelled>
      ) : null}
      {showCount ? (
        <p className="query-count" role="status">
          {matches.status === 'loading'
            ? t('query.counting')
            : matches.status === 'error'
              ? t('query.countError')
              : countText(searched.countMessage(matches.data, siteId !== undefined), matches.data)}
        </p>
      ) : null}
    </fieldset>
  );
};
