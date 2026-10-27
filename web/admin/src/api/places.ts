/**
 * The reads behind a set's place and a record's "Linked from": Romeka's own count routes
 * (`crates/omeka-api/src/read/place_counts.rs`), the one-record read that names a link target,
 * and the `res` property search each link row opens. Every read runs as the caller, so a count
 * equals the total of the list it opens, and a record the caller may not see answers like a
 * missing one.
 */
import { api } from './client';
import type { ResourceQuery } from './client';
import { asCount, list, member, oneOf, strict } from './import-read';
import { ParseError, asIdNumber, asRecord, asString, brandId, isUnknownArray } from './parse';
import type { ItemId, ItemSetId, MediaId, Parser, Path, PropertyId, ResourceId } from './types';

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
  'property[0][type]': 'res',
  'property[0][text]': String(target),
  ...(via === null ? {} : { 'property[0][property]': String(via) }),
});

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
  async record(id: ResourceId): Promise<RecordSummary> {
    return parseRecordSummary(await api.requestJson(`/api/resources/${String(id)}`));
  },
};
