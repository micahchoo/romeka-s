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
export type TermListId = Id<'TermListId'>;
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

/**
 * The port's media kind names, one per `MediaKind` variant, as the server sends
 * them in `o:kind` and accepts them in `romeka_kind` and `has_media_kind`.
 */
export type ServerMediaKind =
  | 'local-image'
  | 'iiif-image'
  | 'audio'
  | 'video'
  | 'pdf'
  | 'document'
  | 'model-3d'
  | 'captions'
  | 'embed'
  | 'iiif-presentation'
  | 'other-file';

export const SERVER_MEDIA_KINDS: readonly ServerMediaKind[] = [
  'local-image',
  'iiif-image',
  'audio',
  'video',
  'pdf',
  'document',
  'model-3d',
  'captions',
  'embed',
  'iiif-presentation',
  'other-file',
];

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

/**
 * Where a media's derivatives stand, from `o-module-romeka:preview`. A create
 * outside an import answers `pending` and a worker makes the preview; `failed`
 * carries the tool's reason; `none` means there is nothing to wait for.
 */
export interface MediaPreview {
  readonly state: 'pending' | 'ready' | 'failed' | 'none';
  readonly reason: string | null;
  /** 0-100 while a long derivative runs. */
  readonly progress: number | null;
}

export interface Media extends ResourceEntity<MediaId> {
  readonly ingester: string;
  readonly renderer: string;
  /**
   * The server's kind, from the port's `o:kind`: one stable name per
   * `omeka_media::kind::MediaKind` variant. `null` on an older server that
   * does not send it, which is when the admin falls back to its own table.
   */
  readonly kind: ServerMediaKind | null;
  readonly item: ResourceRef<ItemId> | null;
  readonly source: string | null;
  readonly mediaType: string | null;
  readonly sha256: string | null;
  readonly size: number | null;
  readonly filename: string | null;
  readonly lang: string | null;
  readonly altText: string | null;
  readonly originalUrl: string | null;
  /**
   * The derivative state the port adds. `null` on an older server that does not
   * send the member; the preview then reads as before.
   */
  readonly preview: MediaPreview | null;
  /**
   * The playable copy the port plans, from `o-module-romeka:access`: `ready`
   * with the copy's URL when a player opens it instead of the original, and
   * `pending` while it is made. `null` on an older server.
   */
  readonly access: MediaAccess | null;
  readonly thumbnailUrls: Readonly<Record<string, string>>;
}

/** Where a media's playable copy stands (`o-module-romeka:access`). */
export interface MediaAccess {
  readonly state: MediaPreview['state'];
  readonly reason: string | null;
  readonly progress: number | null;
  /** The copy a player opens when `state` is `ready`. */
  readonly url: string | null;
  readonly mediaType: string | null;
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
   * The user's lesson settings, from the port's `o:user_settings`. A single-user
   * read, a sign-in and `GET /api/me` carry them; any other read, and a user who
   * stores none, reads as lessons shown and none seen.
   */
  readonly lessons: LessonSettings;
  /**
   * The admin locale the session resolves to, the port's `o:admin_locale`: the
   * user's setting, else the global one (`MvcListeners::bootstrapLocale`). Only
   * a sign-in and `GET /api/me` carry it; `null` when neither setting exists.
   */
  readonly adminLocale: string | null;
  /**
   * The Items lists the user saved, the port-only `romeka_saved_lists`. Only the
   * user's own read carries them (`GET /api/me`, a sign-in, their own account);
   * any other read is none.
   */
  readonly savedLists: readonly SavedList[];
}

/**
 * A saved list: a name, unique among the user's lists, and the Items screen's
 * URL query it opens (`#/items?<query>`).
 */
export interface SavedList {
  readonly name: string;
  readonly query: string;
}

/** `romeka_hide_lessons` and `romeka_lessons_seen`, two port-only user settings. */
export interface LessonSettings {
  readonly hidden: boolean;
  /** Lesson ids as stored. The admin's lesson registry decides what they name. */
  readonly seen: readonly string[];
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

/** The serialisations a vocabulary import reads (`VocabularyForm`'s `format` select). */
export type VocabularyFormat = 'guess' | 'rdfxml' | 'turtle' | 'ntriples' | 'jsonld';

/** Where an import or an update reads its RDF: an uploaded file's text, or a URL. */
export type VocabularyOrigin =
  | { readonly kind: 'upload'; readonly text: string }
  | { readonly kind: 'url'; readonly url: string };

/**
 * `VocabularyForm`'s file and advanced fieldsets. An empty language keeps the
 * first label in the file; an empty property keeps upstream's default path.
 */
export interface VocabularySource {
  readonly origin: VocabularyOrigin;
  readonly format: VocabularyFormat;
  readonly lang: string;
  readonly labelProperty: string;
  readonly commentProperty: string;
}

/** One class or property a file declares. */
export interface VocabularyTerm {
  readonly localName: string;
  readonly label: string;
  readonly comment: string | null;
}

/** A label or comment the file changes, from the stored text to the file's. */
export interface VocabularyTextChange<T> {
  readonly localName: string;
  readonly from: T;
  readonly to: T;
}

/** What an update from a file would do to one kind of member (`RdfImporter::calculateDiff`). */
export interface VocabularyMemberDiff {
  readonly added: readonly VocabularyTerm[];
  readonly label: readonly VocabularyTextChange<string>[];
  readonly comment: readonly VocabularyTextChange<string | null>[];
  /** Stored members the file no longer declares. They are kept. */
  readonly removed: readonly { readonly localName: string; readonly label: string }[];
}

export interface VocabularyDiff {
  readonly classes: VocabularyMemberDiff;
  readonly properties: VocabularyMemberDiff;
}

/** The data type a term list registers: `customvocab:<id>`. */
export type TermListName = string & { readonly __brand: 'TermListName' };

/**
 * What a term list offers. The module stores three nullable columns and
 * `CustomVocabRepresentation::type` reads them by precedence: an item set,
 * then URIs, then terms. `empty` is a stored list with none of the three.
 */
export type TermListSource =
  | { readonly kind: 'terms'; readonly terms: readonly string[] }
  | { readonly kind: 'uris'; readonly uris: readonly TermListUri[] }
  | { readonly kind: 'items'; readonly itemSet: ResourceRef<ItemSetId> }
  | { readonly kind: 'empty' };

export interface TermListUri {
  readonly uri: string;
  /** Empty when the list shows the URI itself. */
  readonly label: string;
}

/** `CustomVocabRepresentation` (Custom Vocab module v2.1.0). */
export interface TermList {
  readonly id: TermListId;
  readonly apiUrl: string;
  readonly dataType: TermListName;
  readonly label: string;
  readonly lang: string | null;
  readonly source: TermListSource;
  readonly owner: ResourceRef<UserId> | null;
}

/** The members `CustomVocabAdapter::hydrate` reads. One source is sent. */
export interface TermListWrite {
  readonly label: string;
  readonly lang: string | null;
  readonly source:
    | { readonly kind: 'terms'; readonly terms: readonly string[] }
    | { readonly kind: 'uris'; readonly uris: readonly TermListUri[] }
    | { readonly kind: 'items'; readonly itemSet: ItemSetId };
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

/** The global roles `Omeka\Permissions\Acl` registers, highest first (`Acl.php:10-15`). */
export const USER_ROLES = ['global_admin', 'site_admin', 'editor', 'reviewer', 'author', 'researcher'] as const;

/** A global role as `o:role` carries it. */
export type UserRole = (typeof USER_ROLES)[number];

export const isUserRole = (value: string): value is UserRole =>
  USER_ROLES.some((role) => role === value);

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

/** Who saved a page version: a user the history routes name. */
export interface PageAuthor {
  readonly id: UserId;
  readonly name: string;
}

/** One entry of `GET /api/site_pages/{id}/versions`, a Romeka route. */
export interface PageVersionSummary {
  readonly version: number;
  /** When this state went live. */
  readonly created: string;
  /** `null` for the state a page had before its first recorded save, or a deleted user. */
  readonly author: PageAuthor | null;
  readonly blockCount: number;
}

/**
 * What a page update answered: the stored page, or the refusal of a save made
 * from an older `o:modified` than the stored page's, with who saved since.
 */
export type PageSaveResult =
  | { readonly kind: 'saved'; readonly page: SitePage }
  | {
      readonly kind: 'stale';
      readonly modified: string | null;
      readonly author: PageAuthor | null;
    };

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
  /** `romeka_hide_lessons`, sent in `o:user_settings`; absent leaves it alone. */
  readonly lessonsHidden?: boolean;
}

/**
 * A user's settings write, the port's `o:user_settings` member. A member left
 * out keeps what the user stores, so each caller sends only what it changed.
 */
export interface UserSettingsWrite {
  /** `''` is the Default option, which removes the row. */
  readonly locale?: string | undefined;
  readonly lessonsHidden?: boolean | undefined;
  readonly lessonsSeen?: readonly string[] | undefined;
  /** The whole list: `[]` removes every saved list. */
  readonly savedLists?: readonly SavedList[] | undefined;
}

/**
 * The global settings `GET /api/settings` answers, a port route. Upstream's
 * `SettingForm` writes these server-side; the port's SPA reads them here.
 */
export interface GlobalSettings {
  /** The global `locale`, or `null` when none is stored. */
  readonly locale: string | null;
  /**
   * `media_alt_text_property`: the property term whose first value describes a
   * media that stores no alt text, or `null` when the fallback is off.
   */
  readonly mediaAltTextProperty: string | null;
  /**
   * `media_type_whitelist` as it applies: the stored list, else upstream's
   * default, which the server answers in its place.
   */
  readonly mediaTypeWhitelist: readonly string[];
  /** `extension_whitelist` as it applies, the same way. */
  readonly extensionWhitelist: readonly string[];
  /** `disable_file_validation`: every file passes when `true`. */
  readonly disableFileValidation: boolean;
  /**
   * `import_allow_private_urls`, a port setting: every fetch an ingester makes
   * may reach loopback, link-local and private addresses when `true`.
   */
  readonly importAllowPrivateUrls: boolean;
  /**
   * `romeka_annotations_public`, the annotation off switch: notes show to
   * visitors when this is `true`. Captions show either way.
   */
  readonly annotationsPublic: boolean;
  /** `installation_title`, or `null` when none is stored. */
  readonly installationTitle: string | null;
  /** `administrator_email`, or `null` when none is stored. */
  readonly administratorEmail: string | null;
  /** `version_notifications`, stored only: the port checks for no new version. */
  readonly versionNotifications: boolean;
  /** `time_zone` as it applies, `UTC` when none is stored. */
  readonly timeZone: string;
  /** `pagination_per_page` as it applies, 25 when none is stored. */
  readonly perPage: number;
  /** `default_to_private_item_sets`. */
  readonly defaultPrivateItemSets: boolean;
  /** The job a write with `indexFulltextSearch` started, else `null`. */
  readonly indexJob: number | null;
}

/**
 * The settings every admin screen applies (`GET /api/romeka/installation`, a
 * port route any caller may read), each as it applies.
 */
export interface Installation {
  /** `installation_title`, else `Romeka S`. */
  readonly title: string;
  /** `pagination_per_page`, the page size of a browse list, else 25. */
  readonly perPage: number;
  /** `time_zone`, the zone every admin date prints in, else `UTC`. */
  readonly timeZone: string;
  /** `default_to_private_item_sets`: a new item set starts private. */
  readonly defaultPrivateItemSets: boolean;
}

/**
 * A `PATCH /api/settings` body. A list of `null` removes its row, which
 * restores upstream's default list; an empty list refuses every file.
 */
export interface GlobalSettingsWrite {
  readonly locale: string | null;
  readonly mediaAltTextProperty: string | null;
  readonly mediaTypeWhitelist: readonly string[] | null;
  readonly extensionWhitelist: readonly string[] | null;
  readonly disableFileValidation: boolean;
  readonly importAllowPrivateUrls: boolean;
  readonly annotationsPublic: boolean;
  readonly installationTitle: string;
  readonly administratorEmail: string;
  readonly timeZone: string;
  readonly perPage: number;
  readonly defaultPrivateItemSets: boolean;
  readonly versionNotifications: boolean;
  /** Start the full-text index job; never stored. */
  readonly indexFulltextSearch: boolean;
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

/** The items a site change acts on: a stored-shape item query, or picked items and item sets. */
export type SiteItemsSelector =
  | { readonly kind: 'query'; readonly query: JsonValue }
  /** These items with the items of these item sets; both empty names nothing. */
  | {
      readonly kind: 'picked';
      readonly items: readonly ItemId[];
      readonly itemSets: readonly ItemSetId[];
    };

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
