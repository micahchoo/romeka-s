/**
 * Hand-written parsers for the wire shapes in `types.ts`.
 *
 * Every parser takes `unknown` and either returns a typed value or throws a
 * `ParseError` naming the JSON path that failed. No casts, no trust.
 */
import type {
  ApiErrors,
  ApiResource,
  Asset,
  AssetId,
  ClassId,
  DataType,
  Id,
  Item,
  ItemId,
  ItemSet,
  ItemSetId,
  Job,
  JsonValue,
  Media,
  MediaId,
  Module,
  PageId,
  Path,
  Parser,
  PasswordLink,
  Property,
  PropertyId,
  ResourceClass,
  ResourceEntity,
  ResourceId,
  ResourceRef,
  ResourceTemplate,
  ResourceTemplateProperty,
  Site,
  SiteId,
  SiteItemSet,
  SitePage,
  SitePageBlock,
  SitePermission,
  TemplateId,
  ThumbnailType,
  ThumbnailUrls,
  User,
  UserId,
  Value,
  Vocabulary,
  VocabularyId,
  VocabularyMember,
} from './types';

/** The id parser a resource-entity core uses to brand its numeric `o:id`. */
type Brand<IdT extends number> = (id: number) => IdT;

export class ParseError extends Error {
  readonly path: Path;

  constructor(path: Path, expected: string) {
    super(`${formatPath(path)}: expected ${expected}`);
    this.name = 'ParseError';
    this.path = path;
  }
}

export function formatPath(path: Path): string {
  let formatted = '$';
  for (const part of path) {
    formatted += typeof part === 'number' ? `[${String(part)}]` : `.${part}`;
  }
  return formatted;
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function isUnknownArray(value: unknown): value is unknown[] {
  return Array.isArray(value);
}

type FieldParser<T> = (value: unknown, path: Path) => T;

export const parseListOf =
  <T>(parse: FieldParser<T>): Parser<readonly T[]> =>
  (raw, path = []) => {
    if (!isUnknownArray(raw)) {
      throw new ParseError(path, 'array');
    }
    return raw.map((entry, index) => parse(entry, [...path, index]));
  };

const XSD_DATE_TIME = 'http://www.w3.org/2001/XMLSchema#dateTime';

const THUMBNAIL_TYPES: readonly ThumbnailType[] = ['large', 'medium', 'square'];

const TERM_PATTERN = /^[A-Za-z0-9_-]+:[A-Za-z0-9_.-]+$/;

/**
 * JSON-LD keys that are not property terms. Anything else matching
 * `TERM_PATTERN` is read as a group of values.
 */
const ENTITY_KEYS: ReadonlySet<string> = new Set([
  '@context',
  '@id',
  '@type',
  '@reverse',
  'o:id',
  'o:is_public',
  'o:owner',
  'o:resource_class',
  'o:resource_template',
  'o:thumbnail',
  'o:title',
  'o:created',
  'o:modified',
  'thumbnail_display_urls',
  'o:primary_media',
  'o:media',
  'o:item_set',
  'o:site',
  'o:is_open',
  'o:items',
  'o:ingester',
  'o:renderer',
  'o:item',
  'o:source',
  'o:media_type',
  'o:sha256',
  'o:size',
  'o:filename',
  'o:lang',
  'o:alt_text',
  'o:original_url',
  'o:thumbnail_urls',
  'o:local_name',
  'o:label',
  'o:comment',
  'o:term',
  'o:vocabulary',
  'o:name',
  'o:email',
  'o:role',
  'o:is_active',
]);

function brandId<Name extends string>(value: number): Id<Name> {
  // eslint-disable-next-line @typescript-eslint/consistent-type-assertions -- the single place a validated number becomes a branded id
  return value as Id<Name>;
}

const asRecord = (value: unknown, path: Path): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new ParseError(path, 'object');
  }
  return value;
};

const asString = (value: unknown, path: Path): string => {
  if (typeof value !== 'string') {
    throw new ParseError(path, 'string');
  }
  return value;
};

const asNumber = (value: unknown, path: Path): number => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new ParseError(path, 'number');
  }
  return value;
};

const asBoolean = (value: unknown, path: Path): boolean => {
  if (typeof value !== 'boolean') {
    throw new ParseError(path, 'boolean');
  }
  return value;
};

const asIdNumber = (value: unknown, path: Path): number => {
  const id = asNumber(value, path);
  if (!Number.isSafeInteger(id) || id <= 0) {
    throw new ParseError(path, 'a positive integer');
  }
  return id;
};

const field = (object: Record<string, unknown>, key: string, path: Path): unknown => {
  if (!(key in object)) {
    throw new ParseError([...path, key], 'a value');
  }
  return object[key];
};

/** A field upstream always sends, whose value may be null. */
const nullableField = <T>(
  object: Record<string, unknown>,
  key: string,
  path: Path,
  parse: FieldParser<T>,
): T | null => {
  const value = field(object, key, path);
  if (value === null) {
    return null;
  }
  return parse(value, [...path, key]);
};

/** A field upstream omits when it has no value. */
const optionalString = (object: Record<string, unknown>, key: string, path: Path): string | null => {
  const value = object[key];
  if (value === undefined || value === null) {
    return null;
  }
  return asString(value, [...path, key]);
};

/** A field a port addition omits, kept as `null` when absent. */
const optionalJsonValue = (
  object: Record<string, unknown>,
  key: string,
  path: Path,
): JsonValue | null => {
  const value = object[key];
  if (value === undefined || value === null) {
    return null;
  }
  return asJsonValue(value, [...path, key]);
};

const parseTypes = (object: Record<string, unknown>, path: Path): readonly string[] => {
  const value = field(object, '@type', path);
  if (typeof value === 'string') {
    return [value];
  }
  const items = parseListOf(asString)(value, [...path, '@type']);
  return items;
};

const asDateTime = (value: unknown, path: Path): string => {
  const object = asRecord(value, path);
  const type = asString(field(object, '@type', path), [...path, '@type']);
  if (type !== XSD_DATE_TIME) {
    throw new ParseError([...path, '@type'], `"${XSD_DATE_TIME}"`);
  }
  const text = asString(field(object, '@value', path), [...path, '@value']);
  if (Number.isNaN(Date.parse(text))) {
    throw new ParseError([...path, '@value'], 'an ISO 8601 date time');
  }
  return text;
};

const asRef = <IdT extends number>(
  value: unknown,
  path: Path,
  brand: (id: number) => IdT,
): ResourceRef<IdT> => {
  const object = asRecord(value, path);
  return {
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    id: brand(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
  };
};

const asRefUser = (value: unknown, path: Path): ResourceRef<UserId> =>
  asRef(value, path, brandId<'UserId'>);

const asRefClass = (value: unknown, path: Path): ResourceRef<ClassId> =>
  asRef(value, path, brandId<'ClassId'>);

const asRefProperty = (value: unknown, path: Path): ResourceRef<PropertyId> =>
  asRef(value, path, brandId<'PropertyId'>);

const asRefAsset = (value: unknown, path: Path): ResourceRef<AssetId> =>
  asRef(value, path, brandId<'AssetId'>);

/** Any JSON value kept verbatim, since the wire shape does not type it. */
const asJsonValue = (value: unknown, path: Path): JsonValue => {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return value;
  }
  if (isUnknownArray(value)) {
    return value.map((entry, index) => asJsonValue(entry, [...path, index]));
  }
  if (isRecord(value)) {
    const record: Record<string, JsonValue> = {};
    for (const key of Object.keys(value)) {
      record[key] = asJsonValue(value[key], [...path, key]);
    }
    return record;
  }
  throw new ParseError(path, 'a JSON value');
};

const parseThumbnailUrls = (value: unknown, path: Path): ThumbnailUrls => {
  // A resource with no thumbnail files sends the empty PHP array (`[]`), which
  // JSON keeps as a list. A non-empty list is a shape upstream does not send.
  if (isUnknownArray(value)) {
    if (value.length > 0) {
      throw new ParseError(path, 'an empty array or an object');
    }
    return {};
  }
  const object = asRecord(value, path);
  const urls: Partial<Record<ThumbnailType, string>> = {};
  for (const type of THUMBNAIL_TYPES) {
    const entry = object[type];
    if (entry === undefined || entry === null) {
      continue;
    }
    urls[type] = asString(entry, [...path, type]);
  }
  return urls;
};

const parseValue = (raw: unknown, path: Path = []): Value => {
  const object = asRecord(raw, path);
  const base = {
    type: asString(field(object, 'type', path), [...path, 'type']),
    propertyId: brandId<'PropertyId'>(
      asIdNumber(field(object, 'property_id', path), [...path, 'property_id']),
    ),
    propertyLabel: asString(field(object, 'property_label', path), [...path, 'property_label']),
    isPublic: asBoolean(field(object, 'is_public', path), [...path, 'is_public']),
  };

  if (base.type === 'literal') {
    return {
      ...base,
      kind: 'literal',
      text: asString(field(object, '@value', path), [...path, '@value']),
      lang: optionalString(object, '@language', path),
    };
  }

  if (base.type === 'uri') {
    return {
      ...base,
      kind: 'uri',
      uri: asString(field(object, '@id', path), [...path, '@id']),
      label: optionalString(object, 'o:label', path),
      lang: optionalString(object, 'o:lang', path),
    };
  }

  if (base.type.startsWith('resource')) {
    const resourceName = asString(
      field(object, 'value_resource_name', path),
      [...path, 'value_resource_name'],
    );
    if (resourceName === 'items' || resourceName === 'item_sets' || resourceName === 'media') {
      return {
        ...base,
        kind: 'resource',
        resourceId: brandId<'ResourceId'>(
          asIdNumber(field(object, 'value_resource_id', path), [...path, 'value_resource_id']),
        ),
        resourceName,
        url: optionalString(object, 'url', path),
        displayTitle: optionalString(object, 'display_title', path),
        thumbnailUrl: optionalString(object, 'thumbnail_url', path),
        thumbnailTitle: optionalString(object, 'thumbnail_title', path),
        thumbnailType: optionalString(object, 'thumbnail_type', path),
      };
    }
  }

  return { ...base, kind: 'other' };
};

const parseValueGroups = (
  object: Record<string, unknown>,
  path: Path,
): ReadonlyMap<string, readonly Value[]> => {
  const groups = new Map<string, readonly Value[]>();
  for (const key of Object.keys(object)) {
    if (ENTITY_KEYS.has(key) || !TERM_PATTERN.test(key)) {
      continue;
    }
    groups.set(key, parseListOf(parseValue)(object[key], [...path, key]));
  }
  return groups;
};

/**
 * A `DELETE` answers the entity the flush removed. Doctrine clears its
 * identifier, so the document carries `o:id` null and the collection `@id`
 * (`crates/omeka-api/src/write/detached.rs`). Nothing consumes the body, so the
 * boundary only confirms the answer is that shape.
 */
export const assertDetached = (raw: unknown, path: Path = []): void => {
  const object = asRecord(raw, path);
  if (object['o:id'] !== null) {
    throw new ParseError([...path, 'o:id'], 'null');
  }
};

const parseEntityCore = <IdT extends number>(
  object: Record<string, unknown>,
  path: Path,
  brand: Brand<IdT>,
): ResourceEntity<IdT> => ({
  id: brand(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
  apiUrl: asString(field(object, '@id', path), [...path, '@id']),
  types: parseTypes(object, path),
  isPublic: asBoolean(field(object, 'o:is_public', path), [...path, 'o:is_public']),
  owner: nullableField(object, 'o:owner', path, (value, fieldPath) =>
    asRef(value, fieldPath, brandId<'UserId'>),
  ),
  resourceClass: nullableField(object, 'o:resource_class', path, (value, fieldPath) =>
    asRef(value, fieldPath, brandId<'ClassId'>),
  ),
  resourceTemplate: nullableField(object, 'o:resource_template', path, (value, fieldPath) =>
    asRef(value, fieldPath, brandId<'TemplateId'>),
  ),
  thumbnail: nullableField(object, 'o:thumbnail', path, (value, fieldPath) =>
    asRef(value, fieldPath, brandId<'AssetId'>),
  ),
  title: nullableField(object, 'o:title', path, asString),
  created: asDateTime(field(object, 'o:created', path), [...path, 'o:created']),
  modified: nullableField(object, 'o:modified', path, asDateTime),
  thumbnailDisplayUrls: parseThumbnailUrls(field(object, 'thumbnail_display_urls', path), [
    ...path,
    'thumbnail_display_urls',
  ]),
  values: parseValueGroups(object, path),
});

export const parseItem: Parser<Item> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    ...parseEntityCore(object, path, brandId<'ItemId'>),
    primaryMedia: nullableField(object, 'o:primary_media', path, (value, fieldPath) =>
      asRef(value, fieldPath, brandId<'MediaId'>),
    ),
    media: parseListOf((value, fieldPath) => asRef(value, fieldPath, brandId<'MediaId'>))(
      field(object, 'o:media', path),
      [...path, 'o:media'],
    ),
    itemSets: parseListOf((value, fieldPath) => asRef(value, fieldPath, brandId<'ItemSetId'>))(
      field(object, 'o:item_set', path),
      [...path, 'o:item_set'],
    ),
    sites: parseListOf((value, fieldPath) => asRef(value, fieldPath, brandId<'SiteId'>))(
      field(object, 'o:site', path),
      [...path, 'o:site'],
    ),
  };
};

export const parseMedia: Parser<Media> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    ...parseEntityCore(object, path, brandId<'MediaId'>),
    ingester: asString(field(object, 'o:ingester', path), [...path, 'o:ingester']),
    renderer: asString(field(object, 'o:renderer', path), [...path, 'o:renderer']),
    item: nullableField(object, 'o:item', path, (value, fieldPath) =>
      asRef(value, fieldPath, brandId<'ItemId'>),
    ),
    source: nullableField(object, 'o:source', path, asString),
    mediaType: nullableField(object, 'o:media_type', path, asString),
    sha256: nullableField(object, 'o:sha256', path, asString),
    size: nullableField(object, 'o:size', path, asNumber),
    filename: nullableField(object, 'o:filename', path, asString),
    lang: nullableField(object, 'o:lang', path, asString),
    altText: nullableField(object, 'o:alt_text', path, asString),
    originalUrl: nullableField(object, 'o:original_url', path, asString),
    thumbnailUrls: parseThumbnailUrls(field(object, 'o:thumbnail_urls', path), [
      ...path,
      'o:thumbnail_urls',
    ]),
  };
};

export const parseUser: Parser<User> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'UserId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    name: asString(field(object, 'o:name', path), [...path, 'o:name']),
    email: asString(field(object, 'o:email', path), [...path, 'o:email']),
    created: asString(field(object, 'o:created', path), [...path, 'o:created']),
    role: asString(field(object, 'o:role', path), [...path, 'o:role']),
    isActive: asBoolean(field(object, 'o:is_active', path), [...path, 'o:is_active']),
  };
};

/**
 * `POST /api/users/<id>/password-link`. The route is not a representation, so
 * the parser reads the three members the auth contract names.
 */
export const parsePasswordLink: Parser<PasswordLink> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    key: asString(field(object, 'key', path), [...path, 'key']),
    link: asString(field(object, 'link', path), [...path, 'link']),
    sent: asBoolean(field(object, 'sent', path), [...path, 'sent']),
  };
};

const parseVocabularyMember = (
  object: Record<string, unknown>,
  path: Path,
): VocabularyMember => ({
  apiUrl: asString(field(object, '@id', path), [...path, '@id']),
  types: parseTypes(object, path),
  localName: asString(field(object, 'o:local_name', path), [...path, 'o:local_name']),
  label: nullableField(object, 'o:label', path, asString),
  comment: nullableField(object, 'o:comment', path, asString),
  term: asString(field(object, 'o:term', path), [...path, 'o:term']),
  vocabulary: asRef(
    field(object, 'o:vocabulary', path),
    [...path, 'o:vocabulary'],
    brandId<'VocabularyId'>,
  ),
});

export const parseResourceClass: Parser<ResourceClass> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    ...parseVocabularyMember(object, path),
    id: brandId<'ClassId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
  };
};

export const parseProperty: Parser<Property> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    ...parseVocabularyMember(object, path),
    id: brandId<'PropertyId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
  };
};

export const parseVocabulary: Parser<Vocabulary> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'VocabularyId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    namespaceUri: asString(field(object, 'o:namespace_uri', path), [...path, 'o:namespace_uri']),
    prefix: asString(field(object, 'o:prefix', path), [...path, 'o:prefix']),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
    comment: nullableField(object, 'o:comment', path, asString),
    owner: nullableField(object, 'o:owner', path, asRefUser),
  };
};

export const parseItemSet: Parser<ItemSet> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const itemsPath = [...path, 'o:items'];
  const items = asRecord(field(object, 'o:items', path), itemsPath);
  return {
    ...parseEntityCore(object, path, brandId<'ItemSetId'>),
    isOpen: asBoolean(field(object, 'o:is_open', path), [...path, 'o:is_open']),
    itemsUrl: asString(field(items, '@id', itemsPath), [...itemsPath, '@id']),
  };
};

export const parseAsset: Parser<Asset> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'AssetId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    owner: nullableField(object, 'o:owner', path, asRefUser),
    name: asString(field(object, 'o:name', path), [...path, 'o:name']),
    filename: asString(field(object, 'o:filename', path), [...path, 'o:filename']),
    mediaType: asString(field(object, 'o:media_type', path), [...path, 'o:media_type']),
    assetUrl: asString(field(object, 'o:asset_url', path), [...path, 'o:asset_url']),
    altText: nullableField(object, 'o:alt_text', path, asString),
  };
};

export const parseJob: Parser<Job> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'JobId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    status: nullableField(object, 'o:status', path, asString),
    jobClass: asString(field(object, 'o:job_class', path), [...path, 'o:job_class']),
    args: nullableField(object, 'o:args', path, asJsonValue),
    owner: nullableField(object, 'o:owner', path, asRefUser),
    started: nullableField(object, 'o:started', path, asDateTime),
    ended: nullableField(object, 'o:ended', path, asDateTime),
  };
};

export const parseModule: Parser<Module> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: asString(field(object, 'o:id', path), [...path, 'o:id']),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    state: asString(field(object, 'o:state', path), [...path, 'o:state']),
    ini: nullableField(object, 'o:ini', path, asJsonValue),
  };
};

/** `DataTypeRepresentation`: `o:id` is the registered name, a string. */
export const parseDataType: Parser<DataType> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: asString(field(object, 'o:id', path), [...path, 'o:id']),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
  };
};

const parseTemplateProperty = (raw: unknown, path: Path = []): ResourceTemplateProperty => {
  const object = asRecord(raw, path);
  return {
    property: asRef(
      field(object, 'o:property', path),
      [...path, 'o:property'],
      brandId<'PropertyId'>,
    ),
    alternateLabel: nullableField(object, 'o:alternate_label', path, asString),
    alternateComment: nullableField(object, 'o:alternate_comment', path, asString),
    dataTypes: parseListOf(asString)(field(object, 'o:data_type', path), [...path, 'o:data_type']),
    isRequired: asBoolean(field(object, 'o:is_required', path), [...path, 'o:is_required']),
    isPrivate: asBoolean(field(object, 'o:is_private', path), [...path, 'o:is_private']),
    defaultLang: nullableField(object, 'o:default_lang', path, asString),
  };
};

export const parseResourceTemplate: Parser<ResourceTemplate> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'TemplateId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    label: asString(field(object, 'o:label', path), [...path, 'o:label']),
    owner: nullableField(object, 'o:owner', path, asRefUser),
    resourceClass: nullableField(object, 'o:resource_class', path, asRefClass),
    titleProperty: nullableField(object, 'o:title_property', path, asRefProperty),
    descriptionProperty: nullableField(object, 'o:description_property', path, asRefProperty),
    properties: parseListOf(parseTemplateProperty)(
      field(object, 'o:resource_template_property', path),
      [...path, 'o:resource_template_property'],
    ),
  };
};

const parseSitePermission = (raw: unknown, path: Path = []): SitePermission => {
  const object = asRecord(raw, path);
  return {
    user: asRef(field(object, 'o:user', path), [...path, 'o:user'], brandId<'UserId'>),
    role: asString(field(object, 'o:role', path), [...path, 'o:role']),
  };
};

const parseSiteItemSet = (raw: unknown, path: Path = []): SiteItemSet => {
  const object = asRecord(raw, path);
  return {
    itemSet: asRef(
      field(object, 'o:item_set', path),
      [...path, 'o:item_set'],
      brandId<'ItemSetId'>,
    ),
  };
};

const asNullableUrlId = (object: Record<string, unknown>, key: string, path: Path): string | null => {
  const value = object[key];
  if (value === undefined || value === null) {
    return null;
  }
  const refPath = [...path, key];
  const ref = asRecord(value, refPath);
  return asString(field(ref, '@id', refPath), [...refPath, '@id']);
};

export const parseSite: Parser<Site> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const pageRef = (value: unknown, fieldPath: Path) =>
    asRef(value, fieldPath, brandId<'PageId'>);
  return {
    id: brandId<'SiteId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    slug: asString(field(object, 'o:slug', path), [...path, 'o:slug']),
    theme: asString(field(object, 'o:theme', path), [...path, 'o:theme']),
    title: asString(field(object, 'o:title', path), [...path, 'o:title']),
    summary: nullableField(object, 'o:summary', path, asString),
    thumbnail: nullableField(object, 'o:thumbnail', path, asRefAsset),
    navigation: parseListOf(asJsonValue)(
      field(object, 'o:navigation', path),
      [...path, 'o:navigation'],
    ),
    homepage: nullableField(object, 'o:homepage', path, pageRef),
    itemPool: nullableField(object, 'o:item_pool', path, asJsonValue),
    owner: nullableField(object, 'o:owner', path, asRefUser),
    created: asDateTime(field(object, 'o:created', path), [...path, 'o:created']),
    modified: nullableField(object, 'o:modified', path, asDateTime),
    isPublic: asBoolean(field(object, 'o:is_public', path), [...path, 'o:is_public']),
    assignNewItems: asBoolean(
      field(object, 'o:assign_new_items', path),
      [...path, 'o:assign_new_items'],
    ),
    pages: parseListOf(pageRef)(field(object, 'o:page', path), [...path, 'o:page']),
    sitePermissions: parseListOf(parseSitePermission)(
      field(object, 'o:site_permission', path),
      [...path, 'o:site_permission'],
    ),
    siteItemSets: parseListOf(parseSiteItemSet)(
      field(object, 'o:site_item_set', path),
      [...path, 'o:site_item_set'],
    ),
    themeSettings: optionalJsonValue(object, 'o:theme_settings', path),
    siteItemsUrl: asNullableUrlId(object, 'o:site_item', path),
  };
};

const parseSitePageBlock = (raw: unknown, path: Path = []): SitePageBlock => {
  const object = asRecord(raw, path);
  return {
    layout: asString(field(object, 'o:layout', path), [...path, 'o:layout']),
    data: asJsonValue(field(object, 'o:data', path), [...path, 'o:data']),
    layoutData: nullableField(object, 'o:layout_data', path, asJsonValue),
  };
};

export const parseSitePage: Parser<SitePage> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: brandId<'PageId'>(asIdNumber(field(object, 'o:id', path), [...path, 'o:id'])),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
    slug: asString(field(object, 'o:slug', path), [...path, 'o:slug']),
    title: asString(field(object, 'o:title', path), [...path, 'o:title']),
    isPublic: asBoolean(field(object, 'o:is_public', path), [...path, 'o:is_public']),
    layout: nullableField(object, 'o:layout', path, asString),
    layoutData: asJsonValue(field(object, 'o:layout_data', path), [...path, 'o:layout_data']),
    blocks: parseListOf(parseSitePageBlock)(
      field(object, 'o:block', path),
      [...path, 'o:block'],
    ),
    site: asRef(field(object, 'o:site', path), [...path, 'o:site'], brandId<'SiteId'>),
    created: asDateTime(field(object, 'o:created', path), [...path, 'o:created']),
    modified: nullableField(object, 'o:modified', path, asDateTime),
  };
};

export const parseApiResource: Parser<ApiResource> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    id: asString(field(object, 'o:id', path), [...path, 'o:id']),
    apiUrl: asString(field(object, '@id', path), [...path, '@id']),
    types: parseTypes(object, path),
  };
};

/** Parses a resource id out of a URL fragment such as `#/items/12`. */
export function itemIdFromParam(text: string): ItemId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'ItemId'>(value);
}

/** Parses a media id out of a URL fragment such as `#/media/12`. */
export function mediaIdFromParam(text: string): MediaId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'MediaId'>(value);
}

/** Parses an item set id out of a URL fragment such as `#/item-sets/12`. */
export function itemSetIdFromParam(text: string): ItemSetId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'ItemSetId'>(value);
}

/** Parses a site id out of a URL fragment such as `#/sites/12`. */
export function siteIdFromParam(text: string): SiteId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'SiteId'>(value);
}

/** Parses a site page id out of a URL fragment such as `#/sites/1/pages/12`. */
export function pageIdFromParam(text: string): PageId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'PageId'>(value);
}

/**
 * Parses the text of a JSON editor field into a `JsonValue`. The textarea is a
 * boundary: a syntax error becomes a `ParseError` the form shows, not a crash.
 */
export function parseJsonText(text: string): JsonValue {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new ParseError([], 'valid JSON');
  }
  return asJsonValue(value, []);
}

/** A block's `o:data`: as `parseJsonText`, but a bare scalar is invalid. */
export function parseJsonObjectOrArrayText(text: string): JsonValue {
  const value = parseJsonText(text);
  if (!isRecord(value) && !isUnknownArray(value)) {
    throw new ParseError([], 'a JSON object or array');
  }
  return value;
}

/** A navigation or item pool field: a JSON array. */
export function parseJsonArrayText(text: string): readonly JsonValue[] {
  return parseListOf(asJsonValue)(parseJsonText(text), []);
}

/** Parses a user id out of a URL fragment such as `#/users/12`. */
export function userIdFromParam(text: string): UserId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'UserId'>(value);
}

/** Parses a resource id from a form field. */
export function resourceIdFromParam(text: string): ResourceId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'ResourceId'>(value);
}

/** Parses a vocabulary id out of a URL fragment such as `#/vocabularies/12`. */
export function vocabularyIdFromParam(text: string): VocabularyId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'VocabularyId'>(value);
}

/** Parses a property id out of a URL fragment such as `#/properties/12`. */
export function propertyIdFromParam(text: string): PropertyId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'PropertyId'>(value);
}

/** Parses a resource class id out of a URL fragment such as `#/resource-classes/12`. */
export function classIdFromParam(text: string): ClassId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'ClassId'>(value);
}

/** Parses a resource template id out of a URL fragment such as `#/resource-templates/12`. */
export function templateIdFromParam(text: string): TemplateId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'TemplateId'>(value);
}

/** Parses an asset id out of a URL fragment such as `#/assets/12`. */
export function assetIdFromParam(text: string): AssetId | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'AssetId'>(value);
}

/** Parses a job id out of a URL fragment such as `#/jobs/12`. */
export function jobIdFromParam(text: string): Id<'JobId'> | null {
  const value = positiveInt(text);
  return value === null ? null : brandId<'JobId'>(value);
}

const positiveInt = (text: string): number | null => {
  if (!/^\d+$/.test(text)) {
    return null;
  }
  const value = Number.parseInt(text, 10);
  return Number.isSafeInteger(value) && value > 0 ? value : null;
};

export function parseApiErrors(value: unknown): ApiErrors | null {
  if (!isRecord(value)) {
    return null;
  }
  const rawErrors = value['errors'];
  if (!isRecord(rawErrors)) {
    return null;
  }
  const errors: Record<string, readonly string[]> = {};
  for (const key of Object.keys(rawErrors)) {
    const entry = rawErrors[key];
    if (typeof entry === 'string') {
      errors[key] = [entry];
      continue;
    }
    if (isUnknownArray(entry)) {
      const messages: string[] = [];
      for (const message of entry) {
        if (typeof message === 'string') {
          messages.push(message);
        }
      }
      errors[key] = messages;
    }
  }
  return errors;
}
