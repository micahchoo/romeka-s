/**
 * Wire types for the admin application, derived from the JSON-LD shapes in
 * `application/src/Api/Representation/*.php` of omeka-s at 5e413f0.
 * These types describe parsed data only; raw JSON goes through `parse.ts`.
 */

/**
 * Branded id over a number, one per upstream table, mirroring the newtypes in
 * DESIGN.md. Two ids of different kinds never mix.
 */
export type Id<Name extends string> = number & { readonly __brand: Name };

export type AssetId = Id<'AssetId'>;
export type ClassId = Id<'ClassId'>;
export type ItemId = Id<'ItemId'>;
export type ItemSetId = Id<'ItemSetId'>;
export type MediaId = Id<'MediaId'>;
export type PageId = Id<'PageId'>;
export type PropertyId = Id<'PropertyId'>;
export type ResourceId = Id<'ResourceId'>;
export type SiteId = Id<'SiteId'>;
export type TemplateId = Id<'TemplateId'>;
export type UserId = Id<'UserId'>;
export type VocabularyId = Id<'VocabularyId'>;

/** Location of a value inside the raw JSON, for error messages. */
export type Path = readonly (string | number)[];

/** Turns one raw JSON value into a typed value, or throws `ParseError`. */
export type Parser<T> = (raw: unknown, path?: Path) => T;

/** A reference to another resource: `{"@id": url, "o:id": id}`. */
export interface ResourceRef<IdT extends number> {
  readonly apiUrl: string;
  readonly id: IdT;
}

export type ThumbnailType = 'large' | 'medium' | 'square';

export type ThumbnailUrls = Readonly<Partial<Record<ThumbnailType, string>>>;

export type ResourceName = 'items' | 'item_sets' | 'media';

export interface ValueBase {
  readonly type: string;
  readonly propertyId: PropertyId;
  readonly propertyLabel: string;
  readonly isPublic: boolean;
}

export interface LiteralValue extends ValueBase {
  readonly kind: 'literal';
  readonly text: string;
  readonly lang: string | null;
}

export interface UriValue extends ValueBase {
  readonly kind: 'uri';
  readonly uri: string;
  readonly label: string | null;
  readonly lang: string | null;
}

export interface ResourceValue extends ValueBase {
  readonly kind: 'resource';
  readonly resourceId: ResourceId;
  readonly resourceName: ResourceName;
  readonly url: string | null;
  readonly displayTitle: string | null;
  readonly thumbnailUrl: string | null;
  readonly thumbnailTitle: string | null;
  readonly thumbnailType: string | null;
}

/** A module data type the admin base cannot render yet. */
export interface OtherValue extends ValueBase {
  readonly kind: 'other';
}

export type Value = LiteralValue | UriValue | ResourceValue | OtherValue;

/** Fields shared by every resource entity (item, item set, media). */
export interface ResourceEntity<IdT extends number> {
  readonly id: IdT;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly isPublic: boolean;
  readonly owner: ResourceRef<UserId> | null;
  readonly resourceClass: ResourceRef<ClassId> | null;
  readonly resourceTemplate: ResourceRef<TemplateId> | null;
  readonly thumbnail: ResourceRef<AssetId> | null;
  readonly title: string | null;
  readonly created: string;
  readonly modified: string | null;
  readonly thumbnailDisplayUrls: ThumbnailUrls;
  readonly values: ReadonlyMap<string, readonly Value[]>;
}

export interface Item extends ResourceEntity<ItemId> {
  readonly primaryMedia: ResourceRef<MediaId> | null;
  readonly media: readonly ResourceRef<MediaId>[];
  readonly itemSets: readonly ResourceRef<ItemSetId>[];
  readonly sites: readonly ResourceRef<SiteId>[];
}

export interface Media extends ResourceEntity<MediaId> {
  readonly ingester: string;
  readonly renderer: string;
  readonly item: ResourceRef<ItemId> | null;
  readonly source: string | null;
  readonly mediaType: string | null;
  readonly sha256: string | null;
  readonly size: number | null;
  readonly filename: string | null;
  readonly lang: string | null;
  readonly altText: string | null;
  readonly originalUrl: string | null;
  readonly thumbnailUrls: Readonly<Record<string, string>>;
}

export interface User {
  readonly id: UserId;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly name: string;
  readonly email: string;
  readonly created: string;
  readonly role: string;
  readonly isActive: boolean;
  /**
   * The user's own `locale` setting, from the port's `o:user_settings`. Only a
   * single-user read carries it; `null` when the user stores none.
   */
  readonly localeSetting: string | null;
  /**
   * The admin locale the session resolves to, the port's `o:admin_locale`: the
   * user's setting, else the global one (`MvcListeners::bootstrapLocale`). Only
   * a sign-in and `GET /api/me` carry it; `null` when neither setting exists.
   */
  readonly adminLocale: string | null;
}

/** Fields shared by a vocabulary member (a property or a resource class). */
export interface VocabularyMember {
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly localName: string;
  readonly label: string | null;
  readonly comment: string | null;
  readonly term: string;
  readonly vocabulary: ResourceRef<VocabularyId>;
}

export interface ResourceClass extends VocabularyMember {
  readonly id: ClassId;
}

export interface Property extends VocabularyMember {
  readonly id: PropertyId;
}

export interface ItemSet extends ResourceEntity<ItemSetId> {
  readonly isOpen: boolean;
  /** The `o:items` search link, which carries only `@id`. */
  readonly itemsUrl: string;
}

/**
 * `VocabularyAdapter::hydrate`'s writable members. `prefix` is create-only in
 * `VocabularyForm::init` (`if (!$vocabulary)`), so the edit form sends the
 * other four and a partial write keeps the stored prefix.
 */
export interface VocabularyWrite {
  readonly namespaceUri: string;
  readonly prefix: string;
  readonly label: string;
  readonly comment: string | null;
  readonly owner: UserId | null;
}

/**
 * `ResourceClassAdapter::hydrate` and `PropertyAdapter::hydrate`'s writable
 * members, the five columns the two share. `vocabulary` is read on create only.
 */
export interface VocabularyMemberWrite {
  readonly localName: string;
  readonly label: string;
  readonly comment: string | null;
  readonly owner: UserId | null;
  readonly vocabulary: VocabularyId;
}

/**
 * One `o:resource_template_property` entry as a write accepts it. Its position
 * is its place in the list, one-based (`ResourceTemplateAdapter::hydrate`).
 */
export interface TemplatePropertyWrite {
  readonly propertyId: PropertyId;
  readonly alternateLabel: string | null;
  readonly alternateComment: string | null;
  readonly dataTypes: readonly string[];
  readonly isRequired: boolean;
  readonly isPrivate: boolean;
  readonly defaultLang: string | null;
}

/**
 * `ResourceTemplateAdapter::hydrate`. The property list replaces the stored
 * rows when the body carries it, and a full write that omits it keeps them.
 */
export interface ResourceTemplateWrite {
  readonly label: string;
  readonly owner: UserId | null;
  readonly resourceClass: ClassId | null;
  readonly titleProperty: PropertyId | null;
  readonly descriptionProperty: PropertyId | null;
  readonly properties: readonly TemplatePropertyWrite[];
}

/** `AssetAdapter::hydrate` on update: the two columns a request may change. */
export interface AssetWrite {
  readonly name: string;
  readonly altText: string | null;
}

/** `AssetRepresentation`: no visibility flag and no dates, so not a resource entity. */
export interface Asset {
  readonly id: AssetId;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly owner: ResourceRef<UserId> | null;
  readonly name: string;
  readonly filename: string;
  readonly mediaType: string;
  readonly assetUrl: string;
  readonly altText: string | null;
}

/** Raw JSON kept verbatim where the wire shape carries it through untyped. */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

export interface Job {
  readonly id: Id<'JobId'>;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly status: string | null;
  readonly jobClass: string;
  readonly args: JsonValue | null;
  readonly owner: ResourceRef<UserId> | null;
  readonly started: string | null;
  readonly ended: string | null;
}

/** `ModuleRepresentation`: `o:id` is the module directory name, a string. */
export interface Module {
  readonly id: string;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly state: string;
  readonly ini: JsonValue | null;
}

/** `DataTypeRepresentation`: `o:id` is the registered data type name, a string. */
export interface DataType {
  readonly id: string;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly label: string;
}

export interface Vocabulary {
  readonly id: VocabularyId;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly namespaceUri: string;
  readonly prefix: string;
  readonly label: string;
  readonly comment: string | null;
  readonly owner: ResourceRef<UserId> | null;
}

export interface ResourceTemplateProperty {
  readonly property: ResourceRef<PropertyId>;
  readonly alternateLabel: string | null;
  readonly alternateComment: string | null;
  readonly dataTypes: readonly string[];
  readonly isRequired: boolean;
  readonly isPrivate: boolean;
  readonly defaultLang: string | null;
}

export interface ResourceTemplate {
  readonly id: TemplateId;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly label: string;
  readonly owner: ResourceRef<UserId> | null;
  readonly resourceClass: ResourceRef<ClassId> | null;
  readonly titleProperty: ResourceRef<PropertyId> | null;
  readonly descriptionProperty: ResourceRef<PropertyId> | null;
  readonly properties: readonly ResourceTemplateProperty[];
}

export interface SitePermission {
  readonly user: ResourceRef<UserId>;
  readonly role: string;
}

/** The site roles `SiteAdmin/IndexController::usersAction` offers. */
export const SITE_ROLES = ['viewer', 'editor', 'admin'] as const;

/** A site role `o:site_permission` accepts. Upstream stores the raw string. */
export type SiteRole = (typeof SITE_ROLES)[number];

/** One `o:site_permission` entry as the write accepts it. */
export interface SitePermissionWrite {
  readonly userId: UserId;
  readonly role: string;
}

export interface SiteItemSet {
  readonly itemSet: ResourceRef<ItemSetId>;
}

export interface Site {
  readonly id: SiteId;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly slug: string;
  readonly theme: string;
  readonly title: string;
  readonly summary: string | null;
  readonly thumbnail: ResourceRef<AssetId> | null;
  readonly navigation: readonly JsonValue[];
  readonly homepage: ResourceRef<PageId> | null;
  readonly itemPool: JsonValue | null;
  readonly owner: ResourceRef<UserId> | null;
  readonly created: string;
  readonly modified: string | null;
  readonly isPublic: boolean;
  readonly assignNewItems: boolean;
  readonly pages: readonly ResourceRef<PageId>[];
  readonly sitePermissions: readonly SitePermission[];
  readonly siteItemSets: readonly SiteItemSet[];
  /**
   * The site's theme settings map, a port addition. Absent from upstream's
   * `SiteRepresentation`; the port stores it in `site_setting` and serves it
   * here so the SPA can render the theme settings form.
   */
  readonly themeSettings: JsonValue | null;
  /**
   * The site's settings map, a port addition. Upstream stores each key as its
   * own `site_setting` row; the port serves the map here so the SPA can render
   * the settings form. Absent when the site stores none.
   */
  readonly siteSettings: JsonValue | null;
  readonly siteItemsUrl: string | null;
}

/**
 * One `o:attachment` entry (`SiteBlockAttachmentRepresentation::jsonSerialize`):
 * the item and the media it may name, plus the caption. A missing item or media
 * is `null`, and the media is only stored when it belongs to the item.
 */
export interface SiteBlockAttachment {
  readonly item: ResourceRef<ItemId> | null;
  readonly media: ResourceRef<MediaId> | null;
  readonly caption: string;
}

/**
 * One entry of a site page's `o:block`
 * (`SitePageBlockRepresentation::jsonSerialize`). `data` is the layout's own
 * payload; `layoutData` is the block's layout metadata, absent upstream as
 * `null`. Both go through `php_json` on write, so an empty object round-trips
 * as an empty array.
 */
export interface SitePageBlock {
  readonly layout: string;
  readonly data: JsonValue;
  readonly layoutData: JsonValue | null;
  /** The block's `site_block_attachment` rows, in position order. */
  readonly attachments: readonly SiteBlockAttachment[];
}

/**
 * `SitePageRepresentation::getJsonLd`. A page is not a resource entity: no
 * class, template, owner or values, and its blocks travel with it.
 */
export interface SitePage {
  readonly id: PageId;
  readonly apiUrl: string;
  readonly types: readonly string[];
  readonly slug: string;
  readonly title: string;
  readonly isPublic: boolean;
  readonly layout: string | null;
  readonly layoutData: JsonValue;
  readonly blocks: readonly SitePageBlock[];
  readonly site: ResourceRef<SiteId>;
  readonly created: string;
  readonly modified: string | null;
}

/** One `o:attachment` entry as a write accepts it. The ids are resolved server-side. */
export interface SiteBlockAttachmentWrite {
  readonly item: ItemId | null;
  readonly media: MediaId | null;
  readonly caption: string;
}

/**
 * A block as `POST`/`PUT` accepts it. `layoutData` is omitted from the body
 * when null, because a present null would store a null rather than the column's
 * absent value. `attachments` is optional because a caller that does not know
 * about attachments must still compile; a full write that omits it clears the
 * block's stored attachments.
 */
export interface SiteBlockWrite {
  readonly layout: string;
  readonly data: JsonValue;
  readonly layoutData: JsonValue | null;
  readonly attachments?: readonly SiteBlockAttachmentWrite[];
}

/**
 * The `SiteAdapter::hydrate` members a new site sets. It names no navigation
 * or homepage, so the server links the welcome page first and the site opens
 * on it.
 */
export interface SiteCreate {
  readonly slug: string;
  readonly title: string;
  readonly theme: string;
  readonly summary: string | null;
  readonly itemPool: JsonValue | null;
  readonly isPublic: boolean;
  readonly assignNewItems: boolean;
  readonly owner: UserId | null;
}

/** `SitePageAdapter::hydrate` members; the update replaces `blocks`. */
export interface SitePageWrite {
  readonly site: SiteId;
  readonly title: string;
  readonly slug: string;
  readonly isPublic: boolean;
  readonly layout: string | null;
  readonly layoutData: JsonValue;
  readonly blocks: readonly SiteBlockWrite[];
}

/**
 * A value as `POST`/`PUT` accepts it, one per `ValueHydrator` value object.
 * The resource key is the property term; `propertyId` is what the wire object
 * carries as `property_id`. `kind` picks the members the data type reads and
 * `type` is the data type name sent as is, so `resource:item` or a module's
 * type survives an edit.
 */
export type ValueWrite =
  | {
      readonly term: string;
      readonly propertyId: PropertyId;
      readonly kind: 'literal';
      readonly type: string;
      readonly text: string;
      readonly lang: string | null;
      readonly isPublic: boolean;
    }
  | {
      readonly term: string;
      readonly propertyId: PropertyId;
      readonly kind: 'uri';
      readonly type: string;
      readonly uri: string;
      readonly label: string | null;
      readonly lang: string | null;
      readonly isPublic: boolean;
    }
  | {
      readonly term: string;
      readonly propertyId: PropertyId;
      readonly kind: 'resource';
      readonly type: string;
      readonly resourceId: ResourceId;
      readonly isPublic: boolean;
    };

/**
 * The shared resource columns `AbstractResourceEntityAdapter::hydrate` writes.
 * A `PUT` resets every one of them, so a form sends all of them on save.
 */
export interface ResourceWrite {
  readonly isPublic: boolean;
  readonly owner: UserId | null;
  readonly resourceClass: ClassId | null;
  readonly resourceTemplate: TemplateId | null;
  readonly thumbnail: AssetId | null;
  readonly values: readonly ValueWrite[];
}

export interface ItemWrite extends ResourceWrite {
  readonly itemSets: readonly ItemSetId[];
  readonly primaryMedia: MediaId | null;
  /**
   * The sites the item belongs to. A full update replaces `o:site`, so an edit
   * sends the sites the item had. `null` omits the member, which a create does
   * so the sites that keep in sync with their search can take the item.
   */
  readonly sites: readonly SiteId[] | null;
}

export interface ItemSetWrite extends ResourceWrite {
  readonly isOpen: boolean;
}

export interface UserWrite {
  readonly name: string;
  readonly email: string;
  readonly role: string;
  readonly isActive: boolean;
  /**
   * The user's `locale` setting, sent in `o:user_settings`. `''` is the
   * Default option, which removes it; absent leaves the stored one alone.
   */
  readonly localeSetting?: string;
}

/**
 * The global settings `GET /api/settings` answers, a port route. Upstream's
 * `SettingForm` writes these server-side; the port's SPA reads them here.
 */
export interface GlobalSettings {
  /** The global `locale`, or `null` when none is stored. */
  readonly locale: string | null;
}

/**
 * The answer to `POST /api/users/<id>/password-link`, an admin minting the
 * one-time key that lets a user set a password. `link` carries the key, so an
 * administrator can hand it over when no SMTP service is configured, and `sent`
 * says whether the invitation also went out by mail.
 */
/**
 * A change to which items a site holds (`POST /api/sites/{id}/items`, a Romeka
 * route). `add` attaches the selected items, `remove` detaches them, `replace`
 * makes them the site's whole membership.
 */
export type SiteItemsAction = 'add' | 'remove' | 'replace';

/** The items a site change acts on: a stored-shape item query, items, or item sets. */
export type SiteItemsSelector =
  | { readonly kind: 'query'; readonly query: JsonValue }
  | { readonly kind: 'items'; readonly items: readonly ItemId[] }
  | { readonly kind: 'itemSets'; readonly itemSets: readonly ItemSetId[] };

export interface SiteItemsCommand {
  readonly action: SiteItemsAction;
  readonly selector: SiteItemsSelector;
}

/** What a site change did, or would do on a dry run. */
export interface SiteItemsChange {
  /** Items the selector named. */
  readonly matched: number;
  readonly added: number;
  readonly removed: number;
  /** Items on the site after the change. */
  readonly total: number;
}

export interface PasswordLink {
  readonly key: string;
  readonly link: string;
  readonly sent: boolean;
}

/** `MediaAdapter::hydrate` on update: the shared columns plus these. */
export interface MediaWrite extends ResourceWrite {
  readonly lang: string | null;
  readonly altText: string | null;
}

/** One entry of `GET /api`: a registered API resource. */
export interface ApiResource {
  readonly id: string;
  readonly apiUrl: string;
  readonly types: readonly string[];
}

/** One page of a search response, with pagination from the response headers. */
export interface ApiPage<T> {
  readonly items: readonly T[];
  readonly total: number;
  readonly page: number;
  readonly perPage: number;
  readonly lastPage: number;
}

/** The upstream API error shape: `{"errors": {"field": ["message", ...]}}`. */
export type ApiErrors = Readonly<Record<string, readonly string[]>>;
