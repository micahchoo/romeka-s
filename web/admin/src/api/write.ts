/**
 * Request-body encoders for the write endpoints, the mirror of `parse.ts`.
 *
 * The shapes are what `crates/omeka-api/src/write/` reads, which follows
 * `ValueHydrator::hydrate` and the per-adapter `hydrate` of omeka-s at 5e413f0.
 * A `PUT` replaces every member the adapter hydrates, so `encodeResource`
 * always writes the class, template and thumbnail members even when the form
 * does not show a control for them.
 */
import type {
  AssetWrite,
  ItemSetId,
  ItemSetWrite,
  ItemWrite,
  JsonValue,
  MediaWrite,
  PageId,
  ResourceTemplateWrite,
  ResourceWrite,
  SiteBlockAttachmentWrite,
  SiteBlockWrite,
  SitePageWrite,
  SitePermissionWrite,
  SiteWrite,
  TemplatePropertyWrite,
  UserId,
  UserWrite,
  ValueWrite,
  VocabularyMemberWrite,
  VocabularyWrite,
} from './types';

/** A JSON object body. The client serialises it with `JSON.stringify`. */
export type WireBody = Readonly<Record<string, unknown>>;

/** `POST` lets the server default the owner; `PUT` writes the member as sent. */
export type WriteMode = 'create' | 'update';

/** A `{"o:id": N}` reference; `{"o:id": null}` clears the member. */
const ref = (id: number | null): Record<string, unknown> => ({ 'o:id': id });

/**
 * `hydrateOwner`'s member: a create with no chosen owner omits it so the server
 * assigns the acting user; a full update must send it, because an absent owner
 * keeps the stored one and only an explicit null clears it.
 */
const ownerMember = (owner: UserId | null, mode: WriteMode): Record<string, unknown> =>
  mode === 'update' || owner !== null ? { 'o:owner': ref(owner) } : {};

/**
 * `Literal::hydrate` reads `@language` and `Uri::hydrate` reads `o:lang`, the
 * member each one's `getJsonLd` writes.
 */
const encodeValue = (value: ValueWrite): Record<string, unknown> => {
  switch (value.kind) {
    case 'literal':
      return {
        property_id: value.propertyId,
        type: value.type,
        '@value': value.text,
        ...(value.lang === null ? {} : { '@language': value.lang }),
        is_public: value.isPublic,
      };
    case 'uri':
      return {
        property_id: value.propertyId,
        type: value.type,
        '@id': value.uri,
        ...(value.label === null ? {} : { 'o:label': value.label }),
        ...(value.lang === null ? {} : { 'o:lang': value.lang }),
        is_public: value.isPublic,
      };
    case 'resource':
      return {
        property_id: value.propertyId,
        type: value.type,
        value_resource_id: value.resourceId,
        is_public: value.isPublic,
      };
  }
};

const encodeValues = (values: readonly ValueWrite[]): Record<string, unknown[]> => {
  const groups: Record<string, unknown[]> = {};
  for (const value of values) {
    const entries = groups[value.term] ?? [];
    entries.push(encodeValue(value));
    groups[value.term] = entries;
  }
  return groups;
};

const encodeResource = (
  write: ResourceWrite,
  mode: WriteMode,
): Record<string, unknown> => ({
  'o:is_public': write.isPublic,
  ...ownerMember(write.owner, mode),
  'o:resource_class': ref(write.resourceClass),
  'o:resource_template': ref(write.resourceTemplate),
  'o:thumbnail': ref(write.thumbnail),
  ...encodeValues(write.values),
});

export const encodeItem = (write: ItemWrite, mode: WriteMode): WireBody => ({
  ...encodeResource(write, mode),
  'o:item_set': write.itemSets.map((id) => ref(id)),
  'o:primary_media': ref(write.primaryMedia),
  ...(write.sites === null ? {} : { 'o:site': write.sites.map((id) => ref(id)) }),
});

export const encodeItemSet = (write: ItemSetWrite, mode: WriteMode): WireBody => ({
  ...encodeResource(write, mode),
  'o:is_open': write.isOpen,
});

export const encodeUser = (write: UserWrite): WireBody => ({
  'o:name': write.name,
  'o:email': write.email,
  'o:role': write.role,
  'o:is_active': write.isActive,
});

export const encodeMedia = (write: MediaWrite): WireBody => ({
  ...encodeResource(write, 'update'),
  'o:lang': write.lang,
  'o:alt_text': write.altText,
});

/**
 * `SiteAdapter::hydrate`. `o:owner` is read on create only; when the form has
 * no owner the member is omitted so the server assigns the acting user. An
 * update ignores it, so omitting it is also the faithful update body.
 */
export const encodeSite = (write: SiteWrite): WireBody => ({
  'o:slug': write.slug,
  'o:title': write.title,
  'o:theme': write.theme,
  'o:summary': write.summary,
  'o:navigation': write.navigation,
  'o:homepage': ref(write.homepage),
  // The column holds an array; the create default and a cleared pool are `[]`.
  'o:item_pool': write.itemPool ?? [],
  'o:is_public': write.isPublic,
  'o:assign_new_items': write.assignNewItems,
  ...(write.owner === null ? {} : { 'o:owner': ref(write.owner) }),
});

/**
 * The partial site bodies the site-admin sub-pages send.
 *
 * `PATCH /api/sites/{id}` writes only the members the body names, so each form
 * sends its own member and leaves the rest of the site alone. The shapes follow
 * `SiteAdapter::hydrate`; `o:theme_settings` is the port's addition.
 */
export const encodeSiteNavigation = (navigation: readonly JsonValue[]): WireBody => ({
  'o:navigation': navigation,
});

/** What the Settings form edits of a site: the members `encodeSiteInfo` writes. */
export interface SiteInfoWrite {
  readonly title: string;
  readonly slug: string;
  readonly theme: string;
  readonly summary: string;
  readonly owner: UserId | null;
  readonly isPublic: boolean;
  readonly assignNewItems: boolean;
}

/**
 * The partial update body of the Settings form (`SiteAdmin/IndexController::
 * editAction`, `isPartial => true`). A full `PUT` would also reset `o:page`,
 * `o:navigation`, `o:item_pool` and `o:site_item_set`. An unset owner is left
 * as stored.
 */
export const encodeSiteInfo = (info: SiteInfoWrite): WireBody => {
  const summary = info.summary.trim();
  return {
    'o:title': info.title.trim(),
    'o:slug': info.slug.trim(),
    'o:theme': info.theme,
    'o:summary': summary === '' ? null : summary,
    'o:is_public': info.isPublic,
    'o:assign_new_items': info.assignNewItems,
    ...(info.owner === null ? {} : { 'o:owner': ref(info.owner) }),
  };
};

/** The homepage member alone; `null` hands the choice back to the first menu page. */
export const encodeSiteHomepage = (page: PageId | null): WireBody => ({
  'o:homepage': page === null ? null : ref(page),
});

export const encodeSiteResources = (
  itemPool: JsonValue | null,
  itemSets: readonly ItemSetId[],
): WireBody => ({
  'o:item_pool': itemPool ?? [],
  'o:site_item_set': itemSets.map((id) => ({ 'o:item_set': ref(id) })),
});

export const encodeSitePermissions = (
  permissions: readonly SitePermissionWrite[],
): WireBody => ({
  'o:site_permission': permissions.map((permission) => ({
    'o:user': ref(permission.userId),
    'o:role': permission.role,
  })),
});

export const encodeSiteThemeSettings = (
  settings: Readonly<Record<string, JsonValue>>,
): WireBody => ({ 'o:theme_settings': settings });

/** The site settings map, a port addition (`o:site_settings`). */
export const encodeSiteSettings = (
  settings: Readonly<Record<string, JsonValue>>,
): WireBody => ({ 'o:site_settings': settings });

const encodeAttachment = (
  attachment: SiteBlockAttachmentWrite,
): Record<string, unknown> => ({
  'o:item': attachment.item === null ? null : ref(attachment.item),
  'o:media': attachment.media === null ? null : ref(attachment.media),
  'o:caption': attachment.caption,
});

const encodeBlock = (block: SiteBlockWrite): Record<string, unknown> => ({
  'o:layout': block.layout,
  'o:data': block.data,
  // A present null would store a null; the update treats an absent member as
  // the block's absent layout data.
  ...(block.layoutData === null ? {} : { 'o:layout_data': block.layoutData }),
  // A full PUT that omits the list clears the block's attachments.
  ...(block.attachments === undefined
    ? {}
    : { 'o:attachment': block.attachments.map(encodeAttachment) }),
});

/** `SitePageAdapter::hydrate`. A `PUT` replaces `o:block` with the list sent. */
export const encodeSitePage = (write: SitePageWrite): WireBody => ({
  'o:site': ref(write.site),
  'o:title': write.title,
  'o:slug': write.slug,
  'o:is_public': write.isPublic,
  'o:layout': write.layout,
  'o:layout_data': write.layoutData,
  'o:block': write.blocks.map(encodeBlock),
});

/**
 * `VocabularyAdapter::hydrate`'s scalar members.
 *
 * The `o:class` and `o:property` lists are not sent. A full `PUT` that omits
 * one removes every member of that kind (`AbstractEntityAdapter`'s
 * `orphanRemoval`), so the vocabulary edit is a `PATCH` and each member owns
 * its own form. `o:prefix` follows `VocabularyForm::init`, which adds the field
 * on create only.
 */
export const encodeVocabulary = (write: VocabularyWrite, mode: WriteMode): WireBody => ({
  'o:namespace_uri': write.namespaceUri,
  ...(mode === 'create' ? { 'o:prefix': write.prefix } : {}),
  'o:label': write.label,
  'o:comment': write.comment,
  ...ownerMember(write.owner, mode),
});

/** `ResourceClassAdapter::hydrate` and `PropertyAdapter::hydrate`. */
export const encodeVocabularyMember = (
  write: VocabularyMemberWrite,
  mode: WriteMode,
): WireBody => ({
  'o:local_name': write.localName,
  'o:label': write.label,
  'o:comment': write.comment,
  // `AbstractVocabularyMemberRepresentation::getJsonLd` carries no `o:owner`,
  // so a member form cannot show the stored owner. Omitting the member keeps it
  // on both a create and an update; only a named owner writes one.
  ...(write.owner === null ? {} : { 'o:owner': ref(write.owner) }),
  // Read on create only (`vocab_member::hydrate`); an update keeps the stored one.
  ...(mode === 'create' ? { 'o:vocabulary': ref(write.vocabulary) } : {}),
});

const encodeTemplateProperty = (
  entry: TemplatePropertyWrite,
): Record<string, unknown> => ({
  'o:property': ref(entry.propertyId),
  'o:alternate_label': entry.alternateLabel,
  'o:alternate_comment': entry.alternateComment,
  'o:data_type': entry.dataTypes,
  'o:is_required': entry.isRequired,
  'o:is_private': entry.isPrivate,
  'o:default_lang': entry.defaultLang,
});

/**
 * `ResourceTemplateAdapter::hydrate`. The property list carries the row set the
 * API accepts and its order is the stored `position`; the server replaces the
 * stored rows with it.
 */
export const encodeResourceTemplate = (
  write: ResourceTemplateWrite,
  mode: WriteMode,
): WireBody => ({
  'o:label': write.label,
  ...ownerMember(write.owner, mode),
  'o:resource_class': ref(write.resourceClass),
  'o:title_property': ref(write.titleProperty),
  'o:description_property': ref(write.descriptionProperty),
  'o:resource_template_property': write.properties.map(encodeTemplateProperty),
});

/** `AssetAdapter::hydrate` on update: the name and the alt text. */
export const encodeAsset = (write: AssetWrite): WireBody => ({
  'o:name': write.name,
  'o:alt_text': write.altText,
});
