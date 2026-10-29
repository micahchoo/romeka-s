/**
 * The Items browse query: one record the screen renders from, the URL that
 * holds it (`#/items?q=&class=&set=&links=&via=&template=&field=&owner=&visibility=&site=&media=&changed=&sort=&view=&page=&list=`),
 * and the API query it sends. Every function is pure; the screen owns only the
 * effects (reading and writing the URL, the remembered view, the request).
 */
import type { ResourceQuery } from '../../api/client';
import { brandId } from '../../api/parse';
import { fieldQuery, linksQuery } from '../../api/places';
import type { LinkTarget } from '../../api/places';
import type { ClassId, ItemId, ItemSetId, PropertyId, ResourceId, SiteId, TemplateId, UserId } from '../../api/types';
import { isFilterKind, serverKindsOf } from '../../app/media';
import type { FilterKind } from '../../app/media';
import { LIST_KEY } from '../../app/open-saved-list';

export const PER_PAGE = 24;

export type Visibility = 'public' | 'private';

/** Which sites an item is in: any site, none, or one site. */
export type SiteFilter =
  | { readonly kind: 'any' }
  | { readonly kind: 'none' }
  | { readonly kind: 'site'; readonly id: SiteId };

/** What media an item has: any media, none, or a media of one kind. */
export type MediaFilter =
  | { readonly kind: 'any' }
  | { readonly kind: 'none' }
  | { readonly kind: 'of'; readonly of: FilterKind };

/** Items whose values point at one record, through one field or any. */
export interface LinksFilter {
  readonly target: ResourceId;
  readonly via: PropertyId | null;
}

/** Whether a field has no value on the item, or at least one. */
export type Presence = 'empty' | 'filled';
export const PRESENCES: readonly Presence[] = ['empty', 'filled'];

/** Items whose one field is empty, or has a value. */
export interface FieldFilter {
  readonly property: PropertyId;
  readonly presence: Presence;
}

/** An order the server keeps by a column of the item. */
export type ColumnSort = 'newest' | 'oldest' | 'modified' | 'title';
export const COLUMN_SORTS: readonly ColumnSort[] = ['newest', 'oldest', 'modified', 'title'];

/**
 * An order by a field's values, in shelf order (`AF_2` before `AF_10`), named by the property's
 * term so the link reads `sort=dcterms:identifier`.
 */
export type FieldSort = `${string}:${string}`;

export type Sort = ColumnSort | FieldSort;

const TERM = /^[A-Za-z][\w.-]*:[A-Za-z_][\w.-]*$/;

export const isFieldSort = (sort: Sort): sort is FieldSort => sort.includes(':');

/** Whether text is a property term, `prefix:localName`. */
export const isTerm = (text: string): text is FieldSort => TERM.test(text);

/** The sort a URL or a select names: a column sort, a field term, or newest first. */
export const parseSort = (text: string | null): Sort =>
  COLUMN_SORTS.find((sort) => sort === text) ?? (text !== null && isTerm(text) ? text : 'newest');

export type View = 'table' | 'grid';

/** Items in no item set, or with no item type: `set=none`, `template=none`. */
export const NONE = 'none';
export type None = typeof NONE;

/** The id a set or type filter names, or `null` when it names none or "none". */
export const chosenId = <Id>(value: Id | None | null): Id | null => (value === NONE ? null : value);

/** What narrows the list. `null` (or an empty search) is "not set". */
export interface BrowseFilters {
  readonly search: string;
  readonly classId: ClassId | null;
  readonly itemSetId: ItemSetId | None | null;
  readonly links: LinksFilter | null;
  readonly templateId: TemplateId | None | null;
  readonly field: FieldFilter | null;
  readonly ownerId: UserId | null;
  readonly visibility: Visibility | null;
  readonly site: SiteFilter | null;
  readonly media: MediaFilter | null;
  /** Items someone else changed at or after this time (RFC 3339), from the dashboard. */
  readonly changedSince: string | null;
}

export type FilterKey = keyof BrowseFilters;

export interface ItemBrowse {
  readonly filters: BrowseFilters;
  readonly sort: Sort;
  readonly view: View;
  readonly page: number;
  /** The saved list the reader opened this from, kept through filter and order changes. */
  readonly list: string | null;
}

export const NO_FILTERS: BrowseFilters = {
  search: '',
  classId: null,
  itemSetId: null,
  links: null,
  templateId: null,
  field: null,
  ownerId: null,
  visibility: null,
  site: null,
  media: null,
  changedSince: null,
};

/** Chip order: the bar's order, the always-shown filters first. */
export const FILTER_KEYS: readonly FilterKey[] = [
  'templateId',
  'search',
  'itemSetId',
  'links',
  'media',
  'classId',
  'field',
  'ownerId',
  'visibility',
  'site',
  'changedSince',
];

/** The filters behind "More filters". */
export const MORE_KEYS: readonly FilterKey[] = ['classId', 'field', 'ownerId', 'visibility', 'site'];

export const isSet = (filters: BrowseFilters, key: FilterKey): boolean =>
  key === 'search' ? filters.search !== '' : filters[key] !== null;

export const activeFilters = (filters: BrowseFilters): readonly FilterKey[] =>
  FILTER_KEYS.filter((key) => isSet(filters, key));

/** A filter change starts the list again at page 1. */
export const withFilters = (state: ItemBrowse, change: Partial<BrowseFilters>): ItemBrowse => ({
  ...state,
  filters: { ...state.filters, ...change },
  page: 1,
});

export const withoutFilter = (state: ItemBrowse, key: FilterKey): ItemBrowse =>
  withFilters(state, { [key]: NO_FILTERS[key] });

export const withoutFilters = (state: ItemBrowse): ItemBrowse => withFilters(state, NO_FILTERS);

const positiveInt = (text: string | null): number | null =>
  text !== null && /^[1-9]\d{0,14}$/.test(text) ? Number(text) : null;

const idOf =
  <Name extends string>() =>
  (text: string | null) => {
    const value = positiveInt(text);
    return value === null ? null : brandId<Name>(value);
  };

const classIdOf = idOf<'ClassId'>();
const itemSetIdOf = idOf<'ItemSetId'>();
const templateIdOf = idOf<'TemplateId'>();
const userIdOf = idOf<'UserId'>();
const siteIdOf = idOf<'SiteId'>();
const resourceIdOf = idOf<'ResourceId'>();
const propertyIdOf = idOf<'PropertyId'>();

/** `links=<id>` with an optional `via=<property id>`; `via` alone narrows nothing and is dropped. */
const parseLinks = (links: string | null, via: string | null): LinksFilter | null => {
  const target = resourceIdOf(links);
  return target === null ? null : { target, via: propertyIdOf(via) };
};

const oneOf = <T extends string>(choices: readonly T[], text: string | null): T | null =>
  choices.find((choice) => choice === text) ?? null;

/** `field=<property id>:empty` or `:filled`. */
const parseField = (text: string | null): FieldFilter | null => {
  const [id = null, presenceText = null, ...rest] = text?.split(':') ?? [];
  const property = propertyIdOf(id);
  const presence = oneOf(PRESENCES, presenceText);
  return property === null || presence === null || rest.length > 0 ? null : { property, presence };
};

const fieldText = (field: FieldFilter): string => `${String(field.property)}:${field.presence}`;

/** `set=` or `template=`: an id, or `none`. */
const idOrNone =
  <Id>(idOf: (text: string | null) => Id | null) =>
  (text: string | null): Id | None | null =>
    text === NONE ? NONE : idOf(text);

const TIME = /^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}(Z|[+-]\d{2}:\d{2}))?$/;

const parseTime = (text: string | null): string | null => (text !== null && TIME.test(text) ? text : null);

const idText = (value: number | None | null): string | null => (value === null ? null : String(value));

const parseSite = (text: string | null): SiteFilter | null => {
  if (text === 'any' || text === 'none') {
    return { kind: text };
  }
  const id = siteIdOf(text);
  return id === null ? null : { kind: 'site', id };
};

const parseMedia = (text: string | null): MediaFilter | null => {
  if (text === 'any' || text === 'none') {
    return { kind: text };
  }
  return text !== null && isFilterKind(text) ? { kind: 'of', of: text } : null;
};

/**
 * The browse state a URL query names. A value that is unknown or malformed is
 * dropped, so a hand-edited or stale link still opens a list. `view` falls back
 * to the reader's last choice when the URL does not say.
 */
export const parseBrowse = (query: URLSearchParams, fallbackView: View): ItemBrowse => ({
  filters: {
    search: query.get('q')?.trim() ?? '',
    classId: classIdOf(query.get('class')),
    itemSetId: idOrNone(itemSetIdOf)(query.get('set')),
    links: parseLinks(query.get('links'), query.get('via')),
    templateId: idOrNone(templateIdOf)(query.get('template')),
    field: parseField(query.get('field')),
    ownerId: userIdOf(query.get('owner')),
    visibility: oneOf<Visibility>(['public', 'private'], query.get('visibility')),
    site: parseSite(query.get('site')),
    media: parseMedia(query.get('media')),
    changedSince: parseTime(query.get('changed')),
  },
  sort: parseSort(query.get('sort')),
  view: oneOf<View>(['table', 'grid'], query.get('view')) ?? fallbackView,
  page: positiveInt(query.get('page')) ?? 1,
  list: query.get(LIST_KEY) || null,
});

const siteText = (site: SiteFilter): string => (site.kind === 'site' ? String(site.id) : site.kind);

const mediaText = (media: MediaFilter): string => (media.kind === 'of' ? media.of : media.kind);

/**
 * The URL query for a browse state. Defaults are left out (newest first, page
 * 1, the table), so the plain `#/items` stays plain.
 */
export const formatBrowse = (state: ItemBrowse): URLSearchParams => {
  const { filters } = state;
  const entries: readonly (readonly [string, string | null])[] = [
    ['q', filters.search === '' ? null : filters.search],
    ['class', filters.classId === null ? null : String(filters.classId)],
    ['set', idText(filters.itemSetId)],
    ['links', filters.links === null ? null : String(filters.links.target)],
    ['via', filters.links === null || filters.links.via === null ? null : String(filters.links.via)],
    ['template', idText(filters.templateId)],
    ['field', filters.field === null ? null : fieldText(filters.field)],
    ['owner', filters.ownerId === null ? null : String(filters.ownerId)],
    ['visibility', filters.visibility],
    ['site', filters.site === null ? null : siteText(filters.site)],
    ['media', filters.media === null ? null : mediaText(filters.media)],
    ['changed', filters.changedSince],
    ['sort', state.sort === 'newest' ? null : state.sort],
    ['view', state.view === 'table' ? null : state.view],
    ['page', state.page === 1 ? null : String(state.page)],
    [LIST_KEY, state.list],
  ];
  return new URLSearchParams(
    entries.flatMap(([name, value]) => (value === null ? [] : [[name, value]])),
  );
};

/**
 * The query a saved list stores: the filters and the order, without the page, the view and the
 * list it came from. The nav compares it with the open address the same way
 * (`app/saved-list-links.tsx`).
 */
export const savedQuery = (state: ItemBrowse): string =>
  formatBrowse({ ...state, view: 'table', page: 1, list: null }).toString();

/** Whether the browse narrows or orders the list, so saving it says more than "every item". */
export const isSavable = (state: ItemBrowse): boolean => savedQuery(state) !== '';

type SortQuery = Pick<ResourceQuery, 'sortBy' | 'sortOrder'>;

const COLUMN_QUERY: Readonly<Record<ColumnSort, SortQuery>> = {
  newest: { sortBy: 'id', sortOrder: 'desc' },
  oldest: { sortBy: 'id', sortOrder: 'asc' },
  modified: { sortBy: 'modified', sortOrder: 'desc' },
  title: { sortBy: 'title', sortOrder: 'asc' },
};

const sortQuery = (sort: Sort): SortQuery =>
  isFieldSort(sort) ? { sortBy: sort, sortOrder: 'asc' } : COLUMN_QUERY[sort];

const siteQuery = (site: SiteFilter | null): ResourceQuery => {
  if (site === null) {
    return {};
  }
  return site.kind === 'site' ? { siteId: site.id } : { inSites: site.kind === 'any' ? 1 : 0 };
};

const mediaQuery = (media: MediaFilter | null): ResourceQuery => {
  if (media === null) {
    return {};
  }
  return media.kind === 'of'
    ? { hasMediaKind: serverKindsOf(media.of) }
    : { hasMedia: media.kind === 'any' ? 1 : 0 };
};

const setQuery = (set: ItemSetId | None | null): ResourceQuery => {
  if (set === null) {
    return {};
  }
  return set === NONE ? { romekaNoItemSet: 1 } : { itemSetId: set };
};

const templateQuery = (template: TemplateId | None | null): ResourceQuery => {
  if (template === null) {
    return {};
  }
  return template === NONE ? { romekaNoResourceTemplate: 1 } : { resourceTemplateId: template };
};

/** The `/api/items` query for one page of a browse state; `ResourceQuery` snake-cases each name. */
export const browseQuery = (state: ItemBrowse): ResourceQuery => ({
  page: state.page,
  perPage: PER_PAGE,
  ...everyMatchQuery(state),
});

/**
 * Every item a browse state matches, on every page: its filters and its order. The list page and
 * the CSV download both read this, so a filter added here reaches both.
 */
export const everyMatchQuery = (state: ItemBrowse): ResourceQuery => {
  const { filters } = state;
  return {
    ...sortQuery(state.sort),
    ...(filters.search === '' ? {} : { search: filters.search }),
    ...(filters.classId === null ? {} : { resourceClassId: filters.classId }),
    ...setQuery(filters.itemSetId),
    ...(filters.links === null ? {} : linksQuery(filters.links.target, filters.links.via)),
    ...templateQuery(filters.templateId),
    ...(filters.field === null ? {} : fieldQuery(filters.field.property, filters.field.presence)),
    ...(filters.ownerId === null ? {} : { ownerId: filters.ownerId }),
    ...(filters.visibility === null ? {} : { isPublic: filters.visibility === 'public' ? 1 : 0 }),
    ...siteQuery(filters.site),
    ...mediaQuery(filters.media),
    ...(filters.changedSince === null ? {} : { romekaChangedByOthersSince: filters.changedSince }),
  };
};

/** The ticked items alone, in the list's order. */
export const tickedListQuery = (state: ItemBrowse, ticked: readonly ItemId[]): ResourceQuery => ({
  ...sortQuery(state.sort),
  id: ticked.join(','),
});

/** The Items screen filtered to the items that link to `target` through `via`, under the record's card. */
export const linksPlaceHref = (target: LinkTarget, via: PropertyId | null): string =>
  via === null ? `#/items?links=${String(target)}` : `#/items?links=${String(target)}&via=${String(via)}`;

/** The Items screen narrowed by `filters`, in the default order and view. */
export const itemsHref = (filters: Partial<BrowseFilters>): string =>
  `#/items?${formatBrowse({ filters: { ...NO_FILTERS, ...filters }, sort: 'newest', view: 'table', page: 1, list: null }).toString()}`;

/** The Items screen filtered to the items of one type whose field is empty, from that type's page. */
export const fieldPlaceHref = (template: TemplateId, property: PropertyId): string =>
  itemsHref({ templateId: template, field: { property, presence: 'empty' } });
