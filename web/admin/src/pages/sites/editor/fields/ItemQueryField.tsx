/**
 * The `itemQuery` field control: a builder for the browse query a block stores,
 * over `fulltext_search`, `item_set_id`, `resource_class_id` and the sort pair.
 * It reads and writes the ordered pairs `query.ts` owns, so an advanced-search
 * key this builder does not show survives an edit.
 *
 * A query over items offers the classes items use, each with its count
 * (`app/class-choices.ts`). Those counts count items, so a query over item
 * sets or media offers every class instead.
 *
 * The count line says what the query reaches (`listReach.ts`). A zero on a site
 * with nothing of the kind links to the site's Items tab, where resources are
 * added; a zero on a site that has some blames the filter.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import { brandId } from '../../../../api/parse';
import type { ClassCount } from '../../../../api/places';
import type { ClassId, ItemSet, ResourceClass } from '../../../../api/types';
import { useClassChoices, useClassOptionText } from '../../../../app/class-choices';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { useTermLabels } from '../../../../i18n/termText';
import type { KeyOf, MessageKey } from '../../../../i18n';
import { useEditorText } from '../messages';
import type { EditorMessageKey } from '../messages';
import { Labelled } from '../Labelled';
import { siteHref } from '../../SiteFrame';
import { QUERY_KEYS, formatQuery, parseQuery, queryId, queryValue, withQueryValue } from './query';
import { filtersByItemSet, useListReach } from './listReach';
import type { ListReach, ListSite, QueryResource } from './listReach';
import { propertyName, useProperties } from './useProperties';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof common> | KeyOf<typeof sites>;

/** The sort fields `AbstractResourceEntityAdapter` orders items by, and their labels. */
const SORT_FIELDS: readonly { readonly value: string; readonly label: MessageId | null }[] = [
  { value: 'title', label: 'query.sort_title' },
  { value: 'created', label: 'query.sort_created' },
  { value: 'modified', label: 'query.sort_modified' },
  { value: 'id', label: 'query.sort_id' },
];

/** A count line: a message of the main catalogue or of the editor table. */
type CountMessage = { readonly main: MessageId } | { readonly editor: EditorMessageKey };

/** What the count line says per resource type. Items read the main
 * catalogue's count, which the site's Items page also shows. */
const COUNT_TEXT: Readonly<
  Record<
    QueryResource,
    {
      readonly count: (scoped: boolean) => CountMessage;
      readonly none: CountMessage;
      readonly siteEmpty: EditorMessageKey;
      readonly add: EditorMessageKey;
      readonly filterEmpty: EditorMessageKey;
    }
  >
> = {
  items: {
    count: (scoped) => ({ main: scoped ? 'query.siteMatchCount' : 'query.matchCount' }),
    none: { main: 'query.noMatch' },
    siteEmpty: 'query.siteEmptyItems',
    add: 'query.addItems',
    filterEmpty: 'query.filterEmptyItems',
  },
  item_sets: {
    count: () => ({ editor: 'query.itemSetCount' }),
    none: { editor: 'query.itemSetCount' },
    siteEmpty: 'query.siteEmptyItemSets',
    add: 'query.addItemSets',
    filterEmpty: 'query.filterEmptyItemSets',
  },
  media: {
    count: () => ({ editor: 'query.mediaCount' }),
    none: { editor: 'query.mediaCount' },
    siteEmpty: 'query.siteEmptyMedia',
    add: 'query.addMedia',
    filterEmpty: 'query.filterEmptyMedia',
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
  readonly classCounts?: readonly ClassCount[];
  /** The site whose items a block shows. The count is scoped to it, as the
   * public render scopes the stored query (`listReach.ts`). */
  readonly site?: ListSite;
  /** False for a query that selects a set rather than lists one, such as a
   * site's item pool: no order applies, so the sort row is left out. */
  readonly sortable?: boolean;
  /** False when the caller shows its own count, such as the site's Items page,
   * which counts what an add would change; the control then asks for none. */
  readonly showCount?: boolean;
  /** The resource the query searches; items unless a sibling choice says otherwise. */
  readonly resource?: QueryResource;
  /** True where the query's reader sorts by a property's value (a block's
   * public render does); the sort then offers every property. */
  readonly propertySort?: boolean;
}

/** The sort select's properties, loaded only where a property sort applies. A
 * stored term the list does not hold yet keeps its own option. */
const PropertySortOptions = ({ current }: { readonly current: string }): ReactElement => {
  const e = useEditorText();
  const state = useProperties();
  const properties = state.status === 'ready' ? state.data : [];
  const listed = !current.includes(':') || properties.some((property) => property.term === current);
  return (
    <optgroup label={e('query.sortProperties')}>
      {listed ? null : <option value={current}>{current}</option>}
      {properties.map((property) => (
        <option key={String(property.id)} value={property.term}>
          {propertyName(property)}
        </option>
      ))}
    </optgroup>
  );
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

/** The stored class id, typed. */
const classId = (id: number | null): ClassId | null => (id === null ? null : brandId<'ClassId'>(id));

/** The class select of a query over items: the classes items use, and the stored one. */
const UsedClassSelect = ({
  id,
  value,
  held,
  counts,
  onChange,
}: {
  readonly id: string;
  readonly value: string;
  readonly held: ClassId | null;
  readonly counts: readonly ClassCount[] | undefined;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const choices = useClassChoices(held, counts);
  const optionText = useClassOptionText();
  return (
    <IdSelect
      id={id}
      value={value}
      onChange={onChange}
      options={choices.map((choice) => ({ id: choice.id, label: optionText(choice) }))}
    />
  );
};

/** The class select of a query over item sets or media: every class. */
const EveryClassSelect = ({
  id,
  value,
  classes,
  onChange,
}: {
  readonly id: string;
  readonly value: string;
  readonly classes: readonly ResourceClass[] | undefined;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const text = useTermLabels();
  const state = useAsync(
    async (): Promise<readonly ResourceClass[]> =>
      classes ?? (await api.listResourceClasses({ perPage: 200, sortBy: 'label' })).items,
    [classes],
  );
  const options = classes ?? (state.status === 'ready' ? state.data : []);
  return (
    <IdSelect
      id={id}
      value={value}
      onChange={onChange}
      options={options.map((entry) => ({ id: entry.id, label: text.label(entry.term, entry.label) ?? entry.term }))}
    />
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
  classCounts,
  site,
  sortable = true,
  showCount = true,
  resource = 'items',
  propertySort = false,
}: ItemQueryFieldProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const e = useEditorText();
  const countText = (message: CountMessage, count: number): string =>
    'main' in message ? t(message.main, { count }) : e(message.editor, { count });
  const byItemSet = filtersByItemSet(resource);
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
  const reach = useListReach(resource, showCount ? value : null, site);
  const itemSetOptions = itemSets ?? (itemSetState.status === 'ready' ? itemSetState.data : []);
  const siteId = site?.id;

  const words = COUNT_TEXT[resource];
  const reachLine = (shown: ListReach): ReactElement | string => {
    switch (shown.kind) {
      case 'matches':
        return countText(words.count(site !== undefined), shown.count);
      case 'none':
        return countText(words.none, 0);
      case 'filterEmpty':
        return e(words.filterEmpty);
      case 'siteEmpty':
        return (
          <>
            {e(words.siteEmpty)}{' '}
            {siteId === undefined ? null : <a href={siteHref(siteId, 'resources')}>{e(words.add)}</a>}
          </>
        );
    }
  };

  const set = (key: string, next: string): void => {
    onChange(formatQuery(withQueryValue(pairs, key, next)));
  };
  const sortFields =
    sortBy !== '' &&
    !SORT_FIELDS.some((field) => field.value === sortBy) &&
    !(propertySort && sortBy.includes(':'))
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
      {byItemSet ? (
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
        {resource === 'items' ? (
          <UsedClassSelect
            id={`${id}-class`}
            value={resourceClass}
            held={classId(queryId(pairs, QUERY_KEYS.resourceClass))}
            counts={classCounts}
            onChange={(next) => {
              set(QUERY_KEYS.resourceClass, next);
            }}
          />
        ) : (
          <EveryClassSelect
            id={`${id}-class`}
            value={resourceClass}
            classes={classes}
            onChange={(next) => {
              set(QUERY_KEYS.resourceClass, next);
            }}
          />
        )}
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
              {propertySort ? <PropertySortOptions current={sortBy} /> : null}
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
          {reach.status === 'loading'
            ? t('query.counting')
            : reach.status === 'error'
              ? t('query.countError')
              : reach.data === null
                ? null
                : reachLine(reach.data)}
        </p>
      ) : null}
    </fieldset>
  );
};
