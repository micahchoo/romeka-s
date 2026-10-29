/**
 * The reads behind a set's place, a record's "Linked from", an item type's field counts, the
 * Items browse's item type chips and the item picker's class filter:
 * Romeka's own count routes (`crates/omeka-api/src/read/place_counts.rs`), the one-record read
 * that names a link target, and the property searches the counts open (`res` for a link row,
 * `ex` and `nex` for a field). Every read runs as the caller, so a count
 * equals the total of the list it opens, and a record the caller may not see answers like a
 * missing one.
 */
import { api } from './client';
import type { ResourceQuery } from './client';
import { asCount, list, member, oneOf, strict } from './import-read';
import { ParseError, asIdNumber, asRecord, asString, brandId, isUnknownArray } from './parse';
import type { ClassId, ItemId, ItemSetId, MediaId, Parser, Path, PropertyId, ResourceId, TemplateId } from './types';

/** The most set ids one counts request may name (`MOST_SET_IDS`). */
export const MOST_SET_IDS = 200;

/** The kinds of record a link row counts, each the name of the endpoint it opens. */
export const LINK_KINDS = ['items', 'item_sets', 'media'] as const;
export type LinkKind = (typeof LINK_KINDS)[number];

/** The records of one kind whose values in one property point at the target. */
export interface LinkRow {
  readonly kind: LinkKind;
  readonly propertyId: PropertyId;
  readonly term: string;
  /** The property's stored label, which `termText` translates while it is the built-in English. */
  readonly label: string;
  readonly count: number;
}

/** What a link target is, as much as a link to it and its name need. */
export type RecordSummary = { readonly title: string | null } & (
  | { readonly kind: 'items'; readonly id: ItemId }
  | { readonly kind: 'item_sets'; readonly id: ItemSetId }
  | { readonly kind: 'media'; readonly id: MediaId }
);

/** A record others may link to: items, item sets and media share one id space (`resource`). */
export type LinkTarget = ResourceId | ItemId | ItemSetId | MediaId;

/** Each set's caller-visible item count. A set the answer leaves out has no count. */
export type SetItemCounts = ReadonlyMap<ItemSetId, number>;

const parseSetCount = (raw: unknown, path: Path): readonly [ItemSetId, number] => {
  const object = strict(raw, path, ['id', 'items']);
  return [brandId<'ItemSetId'>(member(object, 'id', path, asIdNumber)), member(object, 'items', path, asCount)];
};

export const parseSetItemCounts: Parser<SetItemCounts> = (raw, path = []) => {
  const object = strict(raw, path, ['item_sets']);
  return new Map(member(object, 'item_sets', path, list(parseSetCount)));
};

/**
 * The Items browse's item type chips: each type's item total (every type, an unused one at 0),
 * the items with no type, and the items with a class. Each is a total over the whole collection
 * the caller may see, not under the browse's other filters.
 */
export interface ItemTypeCounts {
  readonly items: ReadonlyMap<TemplateId, number>;
  readonly noItemType: number;
  readonly withClass: number;
}

const parseTypeCount = (raw: unknown, path: Path): readonly [TemplateId, number] => {
  const object = strict(raw, path, ['id', 'items']);
  return [brandId<'TemplateId'>(member(object, 'id', path, asIdNumber)), member(object, 'items', path, asCount)];
};

export const parseItemTypeCounts: Parser<ItemTypeCounts> = (raw, path = []) => {
  const object = strict(raw, path, ['item_types', 'no_item_type', 'with_class']);
  return {
    items: new Map(member(object, 'item_types', path, list(parseTypeCount))),
    noItemType: member(object, 'no_item_type', path, asCount),
    withClass: member(object, 'with_class', path, asCount),
  };
};

const parseLinkRow = (raw: unknown, path: Path): LinkRow => {
  const object = strict(raw, path, ['kind', 'property_id', 'term', 'label', 'count']);
  return {
    kind: member(object, 'kind', path, oneOf(LINK_KINDS)),
    propertyId: brandId<'PropertyId'>(member(object, 'property_id', path, asIdNumber)),
    term: member(object, 'term', path, asString),
    label: member(object, 'label', path, asString),
    count: member(object, 'count', path, asCount),
  };
};

export const parseLinkRows: Parser<readonly LinkRow[]> = (raw, path = []) =>
  member(strict(raw, path, ['links']), 'links', path, list(parseLinkRow));

/** The JSON-LD type each kind's document carries (`AbstractResourceEntityRepresentation::getJsonLdType`). */
const KIND_OF_TYPE: Readonly<Record<string, LinkKind>> = {
  'o:Item': 'items',
  'o:ItemSet': 'item_sets',
  'o:Media': 'media',
};

const kindOf = (raw: unknown, path: Path): LinkKind => {
  const types = isUnknownArray(raw) ? raw : [raw];
  for (const type of types) {
    const kind = typeof type === 'string' ? KIND_OF_TYPE[type] : undefined;
    if (kind !== undefined) {
      return kind;
    }
  }
  throw new ParseError(path, 'o:Item, o:ItemSet or o:Media');
};

/** An item, item set or media document from `/api/resources/{id}`, read for its kind and title only. */
export const parseRecordSummary: Parser<RecordSummary> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const stored = object['o:title'];
  const title = stored === undefined || stored === null ? null : asString(stored, [...path, 'o:title']);
  const id = member(object, 'o:id', path, asIdNumber);
  switch (member(object, '@type', path, kindOf)) {
    case 'items':
      return { kind: 'items', id: brandId<'ItemId'>(id), title };
    case 'item_sets':
      return { kind: 'item_sets', id: brandId<'ItemSetId'>(id), title };
    case 'media':
      return { kind: 'media', id: brandId<'MediaId'>(id), title };
  }
};

/**
 * The search that lists the records linking to `target`, through `via` or any property: the
 * query each link row's count is the total of. It is a plain list filter, so it stacks with the
 * browse's other filters and a bulk change selects through it.
 */
export const linksQuery = (target: LinkTarget, via: PropertyId | null): ResourceQuery => ({
  // `buildPropertyQuery` skips a row without a `property` key; an empty one is "any field".
  'property[0][property]': via === null ? '' : String(via),
  'property[0][type]': 'res',
  'property[0][text]': String(target),
});

/**
 * The search that lists the items whose `property` is empty (`nex`) or has a value (`ex`): the
 * query a field count is the total of. Its row index is not the links filter's, so both apply.
 */
export const fieldQuery = (property: PropertyId, presence: 'empty' | 'filled'): ResourceQuery => ({
  'property[1][property]': String(property),
  'property[1][type]': presence === 'empty' ? 'nex' : 'ex',
});

/** The resource types a class count counts: each is a list endpoint. */
export type ClassCountType = 'items' | 'item_sets' | 'media';

/**
 * A class at least one resource of the counted type the caller may see has, with how many such
 * resources have it: the total of `GET /api/<type>?resource_class_id=<id>`.
 */
export interface ClassCount {
  readonly id: ClassId;
  readonly term: string;
  /** The class's stored label; two classes of different standards may share it. */
  readonly label: string;
  readonly vocabularyLabel: string;
  readonly count: number;
}

const parseClassCount = (raw: unknown, path: Path): ClassCount => {
  const object = strict(raw, path, ['id', 'term', 'label', 'vocabulary_label', 'count']);
  return {
    id: brandId<'ClassId'>(member(object, 'id', path, asIdNumber)),
    term: member(object, 'term', path, asString),
    label: member(object, 'label', path, asString),
    vocabularyLabel: member(object, 'vocabulary_label', path, asString),
    count: member(object, 'count', path, asCount),
  };
};

/** The used classes, in the server's order: by label, then the standard's label. */
export const parseClassCounts: Parser<readonly ClassCount[]> = (raw, path = []) =>
  member(strict(raw, path, ['classes']), 'classes', path, list(parseClassCount));

/** How many items of one type the caller may see, and how many of them fill each field. */
export interface FieldCounts {
  readonly total: number;
  /** A field absent here is filled by none of them. */
  readonly filled: ReadonlyMap<PropertyId, number>;
}

const parseFieldCount = (raw: unknown, path: Path): readonly [PropertyId, number] => {
  const object = strict(raw, path, ['property_id', 'filled']);
  return [
    brandId<'PropertyId'>(member(object, 'property_id', path, asIdNumber)),
    member(object, 'filled', path, asCount),
  ];
};

export const parseFieldCounts: Parser<FieldCounts> = (raw, path = []) => {
  const object = strict(raw, path, ['total', 'fields']);
  return {
    total: member(object, 'total', path, asCount),
    filled: new Map(member(object, 'fields', path, list(parseFieldCount))),
  };
};

export const placesApi = {
  /** One request for a page of sets; past `MOST_SET_IDS` the server refuses it. */
  async setItemCounts(ids: readonly ItemSetId[]): Promise<SetItemCounts> {
    if (ids.length === 0) {
      return new Map();
    }
    const params = new URLSearchParams(ids.map((id) => ['id[]', String(id)]));
    return parseSetItemCounts(await api.requestJson('/api/romeka/item-sets/counts', params));
  },
  async links(target: LinkTarget): Promise<readonly LinkRow[]> {
    return parseLinkRows(await api.requestJson(`/api/romeka/links/${String(target)}`));
  },
  async fieldCounts(template: TemplateId): Promise<FieldCounts> {
    const params = new URLSearchParams({ resource_template_id: String(template) });
    return parseFieldCounts(await api.requestJson('/api/romeka/field-counts', params));
  },
  async itemTypeCounts(): Promise<ItemTypeCounts> {
    return parseItemTypeCounts(await api.requestJson('/api/romeka/item-types/counts'));
  },
  async classCounts(resourceType: ClassCountType = 'items'): Promise<readonly ClassCount[]> {
    const params = new URLSearchParams({ resource_type: resourceType });
    return parseClassCounts(await api.requestJson('/api/romeka/classes/counts', params));
  },
  async record(id: ResourceId): Promise<RecordSummary> {
    return parseRecordSummary(await api.requestJson(`/api/resources/${String(id)}`));
  },
};
