//! Read endpoints: `omeka-db` rows to `wire` JSON-LD documents.
//!
//! One [`Api`] value answers one HTTP request. It carries the pool, the base
//! URL every `@id` is built from, the authenticated actor for visibility, and a
//! [`Catalog`] of term and label lookups loaded once. The dispatch in
//! [`Api::search`] and [`Api::read_member`] names one builder per resource; an
//! adapter whose registered operations do not contain the request answers the
//! upstream 500, and one the port has not built answers a 501.
//!
//! Upstream routes every `/api` request through `ApiController` and
//! `Api\Manager`; the per-resource shapes come from
//! `application/src/Api/Representation/*.php`, named on each module.

pub mod catalog;
pub mod export;
pub mod item_history;
pub mod params;
pub mod place_counts;
pub mod query;
pub mod response;

mod meta;
mod order;
mod paging;
mod php;
#[cfg(test)]
mod probe;
mod relations;
mod resources;
#[cfg(test)]
mod taken_tests;
pub(crate) use resources::query_rows;
mod resources_generic;
mod system;
mod term_lists;
mod values;
mod vocab;
mod window;

use std::sync::Arc;

use chrono::NaiveDateTime;
use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids::{SiteId, UserId};
use omeka_db::with_db;
use omeka_media::store::{FileUrls, Store};
use omeka_plugin::{AdapterReply, AdapterRequest, Registry};

use crate::adapters;
use crate::auth::Actor;
use crate::errors::{ApiError, NotImplemented};
use crate::jsonld::{self, BaseUrl};
use crate::messages;
use crate::routes::Operation;
use crate::wire::{JsonLdDate, ResourceRef, ThumbnailDisplayUrls};
use crate::write::access::{self, SiteRole};

pub use paging::Window;
pub use params::{Params, Sort, SortField};
pub(crate) use resources::listed_media;
pub use response::{ApiResponse, JSONLD_CONTENT_TYPE, OMEKA_VERSION};

use catalog::Catalog;

pub(crate) use meta::{
    page_snapshot, site_document, site_page_document, site_page_visible, user_document,
};
pub use meta::{session_user_response, user_response};
pub(crate) use resources::{item_document, item_set_document, matching_item_ids, media_document};
pub(crate) use response::serialize;

/// One request's read surface.
pub struct Api<'a> {
    pool: &'a DbPool,
    base: BaseUrl,
    actor: Actor,
    catalog: Catalog,
    store: Option<Arc<dyn Store>>,
    files: FileUrls,
    plugins: Arc<Registry>,
}

impl<'a> Api<'a> {
    /// Load the lookups a read needs and bind one request's actor.
    pub async fn load(pool: &'a DbPool, base: BaseUrl, actor: Actor) -> Result<Self, ApiError> {
        Self::load_with_store(pool, base, actor, None).await
    }

    /// Load the read surface with the file store a media delete reaches.
    pub async fn load_with_store(
        pool: &'a DbPool,
        base: BaseUrl,
        actor: Actor,
        store: Option<Arc<dyn Store>>,
    ) -> Result<Self, ApiError> {
        Self::load_with_registry(pool, base, actor, store, adapters::core_registry()).await
    }

    /// Load the read surface with a plugin registry, so a request for a
    /// resource a plugin registered is served by that plugin's adapter.
    pub async fn load_with_plugins(
        pool: &'a DbPool,
        base: BaseUrl,
        actor: Actor,
        plugins: Arc<Registry>,
    ) -> Result<Self, ApiError> {
        Self::load_with_registry(pool, base, actor, None, plugins).await
    }

    /// Load the read surface with both a file store and a plugin registry.
    pub async fn load_with_registry(
        pool: &'a DbPool,
        base: BaseUrl,
        actor: Actor,
        store: Option<Arc<dyn Store>>,
        plugins: Arc<Registry>,
    ) -> Result<Self, ApiError> {
        let files = match &store {
            Some(store) => FileUrls::new(store.clone(), base.as_str()),
            None => FileUrls::local(base.as_str()),
        };
        Ok(Self {
            pool,
            base,
            actor,
            catalog: Catalog::load_for_read(pool).await?,
            store,
            files,
            plugins,
        })
    }

    /// Load the whole term table into the catalog. A write names properties by
    /// term and prints the rows it just created; a read that parses
    /// `property[i][property]` resolves a term before it reads the page. Both
    /// need terms the page's rows do not name, and nothing else does.
    pub(crate) async fn fill_catalog(&self) -> Result<(), ApiError> {
        self.catalog.fill_all(self.pool).await
    }

    /// `ApiController::getList` for one resource.
    pub async fn search(&self, resource: &str, params: &Params) -> Result<ApiResponse, ApiError> {
        self.allow(resource, Operation::Search)?;
        if let Some(reply) = self.plugin_read(resource, Operation::Search, None, params) {
            return plugin_response(reply);
        }
        match resource {
            "items" => resources::search_items(self, params).await,
            "item_sets" => resources::search_item_sets(self, params).await,
            "media" => resources::search_media(self, params).await,
            "value_annotations" => resources::search_value_annotations(self, params).await,
            "assets" => system::search_assets(self, params).await,
            "jobs" => system::search_jobs(self, params).await,
            "modules" => system::search_modules(self, params).await,
            "vocabularies" => vocab::search_vocabularies(self, params).await,
            "resource_classes" => vocab::search_classes(self, params).await,
            "properties" => vocab::search_properties(self, params).await,
            "resource_templates" => vocab::search_templates(self, params).await,
            "users" => meta::search_users(self, params).await,
            "sites" => meta::search_sites(self, params).await,
            "site_pages" => meta::search_site_pages(self, params).await,
            "api_resources" => Ok(meta::search_api_resources(self, params)),
            "data_types" => meta::search_data_types(self, params).await,
            "custom_vocabs" => term_lists::search_custom_vocabs(self, params).await,
            "resources" => resources_generic::search_resources(self, params).await,
            other => Err(unimplemented_operation(other, Operation::Search)),
        }
    }

    /// `ApiController::get` for one member of one resource.
    pub async fn read_member(&self, resource: &str, id: &str) -> Result<ApiResponse, ApiError> {
        self.allow(resource, Operation::Read)?;
        if let Some(reply) =
            self.plugin_read(resource, Operation::Read, Some(id), &Params::default())
        {
            return plugin_response(reply);
        }
        match resource {
            "items" => resources::read_item(self, id).await,
            "item_sets" => resources::read_item_set(self, id).await,
            "media" => resources::read_media(self, id).await,
            "value_annotations" => resources::read_value_annotation(self, id).await,
            "assets" => system::read_asset(self, id).await,
            "jobs" => system::read_job(self, id).await,
            "modules" => system::read_module(self, id).await,
            "vocabularies" => vocab::read_vocabulary(self, id).await,
            "resource_classes" => vocab::read_class(self, id).await,
            "properties" => vocab::read_property(self, id).await,
            "resource_templates" => vocab::read_template(self, id).await,
            "users" => meta::read_user(self, id).await,
            "sites" => meta::read_site(self, id).await,
            "site_pages" => meta::read_site_page(self, id).await,
            "api_resources" => Ok(meta::read_api_resource(self, id)),
            "data_types" => meta::read_data_type(self, id).await,
            "custom_vocabs" => term_lists::read_custom_vocab(self, id).await,
            "resources" => resources_generic::read_resource(self, id).await,
            other => Err(unimplemented_operation(other, Operation::Read)),
        }
    }

    /// `GET /api`: the resource listing (`ApiController::getList`, no resource).
    pub async fn api_root(&self) -> Result<ApiResponse, ApiError> {
        meta::api_root(self)
    }

    /// `GET /api-context` (`ApiController::contextAction`).
    pub async fn context(&self) -> Result<ApiResponse, ApiError> {
        meta::context(self).await
    }

    /// `ApiController::create`: parse the body, check access, and let the
    /// resource handler write the row.
    ///
    /// `content_type` selects the JSON or multipart path, so a multipart
    /// `POST /api/assets` reaches the file the adapter needs
    /// (`ApiController::processPostData`).
    pub async fn create(
        &self,
        resource: &str,
        bytes: &[u8],
        content_type: Option<&str>,
    ) -> Result<ApiResponse, ApiError> {
        let body = crate::write::body::RequestBody::parse(content_type, bytes)?;
        self.allow_write(resource, Operation::Create)?;
        self.fill_catalog().await?;
        match body {
            crate::write::body::RequestBody::Json(payload) => {
                crate::write::create(self, resource, &payload, None).await
            }
            crate::write::body::RequestBody::Multipart { payload, file } => {
                crate::write::create(self, resource, &payload, file).await
            }
        }
    }

    /// `ApiController::update` and `::patch`: parse the body, check access, and
    /// let the resource handler write the row. `partial` is `PATCH`.
    pub async fn update(
        &self,
        resource: &str,
        id: &str,
        bytes: &[u8],
        partial: bool,
    ) -> Result<ApiResponse, ApiError> {
        let payload = crate::write::body::Payload::parse(bytes)?;
        self.allow_write(resource, Operation::Update)?;
        self.fill_catalog().await?;
        crate::write::update(self, resource, id, &payload, partial).await
    }

    /// `ApiController::delete`: check adapter access and let the resource
    /// handler find, authorize and remove the row. The answer is the removed
    /// row's representation, which is what `Api\Manager::finalize` renders.
    pub async fn delete(&self, resource: &str, id: &str) -> Result<ApiResponse, ApiError> {
        crate::write::access::adapter(self.actor(), resource, Operation::Delete)?;
        crate::write::delete(self, resource, id).await
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The URLs of stored files: the store's, or the local `/files` ones when
    /// the read has no store.
    pub(crate) fn files(&self) -> &FileUrls {
        &self.files
    }

    /// The file store a media delete reaches, `None` when the process has none.
    pub(crate) fn store(&self) -> Option<&Arc<dyn Store>> {
        self.store.as_ref()
    }

    /// Serve a read from a plugin adapter, when one is registered for the
    /// resource, declares the operation, and answers.
    ///
    /// A compiled-in adapter carries no handler, so this returns `None` for
    /// every resource the port itself registers. The access check runs before
    /// this, so a plugin resource is gated by the rule `allow` gives an
    /// unregistered resource, which is the public read the core read adapters
    /// grant.
    fn plugin_read(
        &self,
        resource: &str,
        operation: Operation,
        id: Option<&str>,
        params: &Params,
    ) -> Option<AdapterReply> {
        let handler = self.plugins.adapter(resource)?.handler()?;
        if !handler.spec().operations.contains(&operation) {
            return None;
        }
        handler.serve(&AdapterRequest {
            resource: resource.to_string(),
            operation,
            id: id.map(str::to_string),
            query: params.decoded_pairs(),
        })
    }

    /// `Api\Manager::execute` general access check for a write operation,
    /// following `Service\AclFactory`. The grants live in
    /// [`crate::write::access`] as one table per operation; this method is the
    /// read layer's name for the create and update check.
    pub(crate) fn allow_write(&self, resource: &str, operation: Operation) -> Result<(), ApiError> {
        crate::write::access::adapter(self.actor(), resource, operation)
    }

    /// `AbstractEntityAdapter::authorize` for a resource entity: editors and up
    /// may change any row, an author only their own. Delete carries a different
    /// entity rule (`write::access::resource_entity`).
    pub(crate) fn authorize_resource(
        &self,
        class: &str,
        owner: Option<UserId>,
    ) -> Result<(), ApiError> {
        match self.actor.role() {
            Some("global_admin" | "site_admin" | "editor" | "reviewer") => Ok(()),
            _ => match (self.actor.user(), owner) {
                (Some(me), Some(owner)) if me == owner => Ok(()),
                _ => Err(entity_denied(class, Operation::Update)),
            },
        }
    }

    /// `UserAdapter` entity access: admins may change any user, everyone else
    /// only their own account (`IsSelfAssertion`).
    pub(crate) fn authorize_user_update(&self, target: UserId) -> Result<(), ApiError> {
        match self.actor.role() {
            Some("global_admin" | "site_admin") => Ok(()),
            _ => match self.actor.user() {
                Some(me) if me == target => Ok(()),
                _ => Err(entity_denied("Omeka\\Entity\\User", Operation::Update)),
            },
        }
    }

    /// `Site` entity access for a create: `AclFactory::addRulesForSites`
    /// grants `create` to `editor` only, and the two admin roles hold it
    /// through their blanket allow.
    pub(crate) fn authorize_site_create(&self) -> Result<(), ApiError> {
        match self.actor.role() {
            Some("global_admin" | "site_admin" | "editor") => Ok(()),
            _ => Err(entity_denied("Omeka\\Entity\\Site", Operation::Create)),
        }
    }

    /// `Site` entity access for an update: `OwnsEntityAssertion` or
    /// `HasSitePermissionAssertion('admin')` (`AclFactory::addRulesForSites`).
    pub(crate) async fn authorize_site_update(
        &self,
        site: SiteId,
        owner: Option<UserId>,
    ) -> Result<(), ApiError> {
        if self
            .site_owner_or_role(site, owner, SiteRole::Admin)
            .await?
        {
            Ok(())
        } else {
            Err(entity_denied("Omeka\\Entity\\Site", Operation::Update))
        }
    }

    /// `SitePageAdapter::hydrate` on create: `authorize($site, 'add-page')`,
    /// whose message names the privilege rather than a request operation.
    pub(crate) async fn authorize_site_add_page(
        &self,
        site: SiteId,
        owner: Option<UserId>,
    ) -> Result<(), ApiError> {
        if self
            .site_owner_or_role(site, owner, SiteRole::Editor)
            .await?
        {
            Ok(())
        } else {
            Err(ApiError::PermissionDenied {
                message: messages::permission_denied("add-page", "Omeka\\Entity\\Site"),
            })
        }
    }

    /// `SitePage` entity access for update and delete: `OwnsEntityAssertion` or
    /// a site role of editor or above (`HasSitePermissionAssertion('editor')`).
    pub(crate) async fn authorize_site_page(
        &self,
        operation: Operation,
        site: SiteId,
        owner: Option<UserId>,
    ) -> Result<(), ApiError> {
        if self
            .site_owner_or_role(site, owner, SiteRole::Editor)
            .await?
        {
            Ok(())
        } else {
            Err(entity_denied("Omeka\\Entity\\SitePage", operation))
        }
    }

    /// `OwnsEntityAssertion` OR `HasSitePermissionAssertion(required)`, the
    /// aggregate `AclFactory::addRulesForSites` attaches to a site write.
    ///
    /// The admin roles pass before the site table is read, because
    /// `addRulesForSiteAdmin` and `addRulesForGlobalAdmin` allow them
    /// everything. The site's owner passes without a `site_permission` row.
    async fn site_owner_or_role(
        &self,
        site: SiteId,
        owner: Option<UserId>,
        required: SiteRole,
    ) -> Result<bool, ApiError> {
        if access::is_admin_role(self.actor.role()) {
            return Ok(true);
        }
        let Some(user) = self.actor.user() else {
            return Ok(false);
        };
        if owner == Some(user) {
            return Ok(true);
        }
        let permissions = with_db!(self.pool, |db| omeka_db::repos::SiteRepo::new(db)
            .list_permissions(site)
            .await)
        .map_err(db_error)?;
        Ok(access::has_site_permission(&permissions, user, required))
    }

    pub(crate) fn pool(&self) -> &DbPool {
        self.pool
    }

    pub(crate) fn base(&self) -> &BaseUrl {
        &self.base
    }

    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Whether the actor may see a resource: public, owned, or a viewer with
    /// `view-all` (`AbstractResourceEntityAdapter::getSubjectValuesQueryBuilder`
    /// applies the same test to reverse values).
    pub(crate) fn sees(&self, is_public: bool, owner: Option<UserId>) -> bool {
        self.actor.sees_resource(is_public, owner)
    }

    /// `Api\Manager::execute` general access check against the ACL rules in
    /// `Service\AclFactory::addRulesForAllRoles` and the per-role grants. It
    /// covers `search` and `read`; the write operations have their own checks
    /// (`write::access::adapter` for every write, entity rules per resource).
    ///
    /// The generic `resources` path calls this for the kind a row holds, so a
    /// value annotation read stays a privileged one.
    pub(crate) fn allow(&self, resource: &str, operation: Operation) -> Result<(), ApiError> {
        let privileged = self.actor.is_privileged();
        let allowed = match resource {
            // `UserAdapter` is granted to every authenticated role, never to
            // anonymous requests.
            "users" => self.actor.user().is_some(),
            // `ValueAnnotationAdapter`, `JobAdapter` and `ModuleAdapter` have
            // no rule for the unauthenticated role.
            "value_annotations" | "jobs" | "modules" => privileged,
            // The remaining adapters grant `search` and `read` to everyone.
            _ => true,
        };
        if allowed {
            Ok(())
        } else {
            Err(permission_denied(resource, operation))
        }
    }
}

/// Wrap a plugin adapter's document in the answer a read returns.
///
/// The plugin owns its wire shape; the port serializes it with the same key
/// order the read layer uses and attaches the JSON-LD headers.
fn plugin_response(reply: AdapterReply) -> Result<ApiResponse, ApiError> {
    let body = jsonld::to_string(&reply.document).map_err(ApiError::internal)?;
    Ok(ApiResponse::document(body))
}

/// The answer for an adapter this unit has not built a handler for.
///
/// Two cases hide behind one fallthrough. An adapter whose `operations` list
/// does not contain the operation refuses it upstream too: `AbstractAdapter`
/// throws `OperationNotImplementedException`, which `ApiJsonStrategy` maps to
/// 500. An adapter that does implement the operation but has no builder here
/// is work the port has not done, which stays a 501 so a caller can tell the
/// two apart.
fn unimplemented_operation(resource: &str, operation: Operation) -> ApiError {
    let body = NotImplemented::adapter(resource, operation);
    match crate::adapters::find(resource) {
        Some(spec) if !spec.operations.contains(&operation) => ApiError::Internal {
            message: body.errors.error,
        },
        _ => ApiError::NotImplemented(body),
    }
}

/// `Api\Manager::execute` permission message, keyed by the adapter's PHP class.
pub(crate) fn permission_denied(resource: &str, operation: Operation) -> ApiError {
    ApiError::PermissionDenied {
        message: messages::permission_denied(
            operation.adapter_name(),
            crate::adapters::php_class(resource),
        ),
    }
}

/// `AbstractEntityAdapter::authorize` permission message, keyed by the entity
/// class (`EntityInterface::getResourceId`).
pub(crate) fn entity_denied(class: &str, operation: Operation) -> ApiError {
    ApiError::PermissionDenied {
        message: messages::permission_denied(operation.adapter_name(), class),
    }
}

/// Map a storage failure to a 500.
pub(crate) fn db_error(error: DbError) -> ApiError {
    ApiError::internal(error)
}

/// [`db_error`], except that a unique-constraint violation is the validation error the write's
/// own check gives, under `key`: a concurrent write took the value between that check and this
/// write, and the caller sees the same 422 it would have seen a moment later.
pub(crate) fn taken_or_db_error(
    key: &'static str,
    message: impl Into<String>,
) -> impl FnOnce(DbError) -> ApiError {
    let message = message.into();
    move |error| {
        if error.is_unique_violation() {
            ApiError::Validation(crate::errors::ValidationErrors::from([(
                key.to_string(),
                vec![message],
            )]))
        } else {
            db_error(error)
        }
    }
}

/// `ResourceReference::jsonSerialize` for one id.
pub(crate) fn resource_ref(base: &BaseUrl, resource: &str, id: u64) -> ResourceRef {
    base.resource_ref(resource, id)
}

/// A reference, or `null` when the id is absent.
pub(crate) fn resource_ref_opt(
    base: &BaseUrl,
    resource: &str,
    id: Option<u64>,
) -> Option<ResourceRef> {
    id.map(|id| resource_ref(base, resource, id))
}

/// `AbstractEntityAdapter::findEntity`: a missing row is a 404 whose message
/// carries the entity class and the criteria upstream `json_encode`s.
///
/// The id reaches the adapter as the route parameter, a string, so the criteria
/// value is always a quoted JSON string even when it looks numeric. Upstream
/// `findEntity` only casts a non-array criteria to `['id' => $criteria]`; it
/// never casts the value.
pub(crate) fn not_found(class: &str, raw_id: &str) -> ApiError {
    let criteria = serde_json::to_string(raw_id).unwrap_or_else(|_| format!("\"{raw_id}\""));
    ApiError::not_found(messages::entity_not_found(
        class,
        format!("{{\"id\":{criteria}}}"),
    ))
}

/// A numeric route id, or the same 404 a missing row gets.
pub(crate) fn parse_id(class: &str, raw_id: &str) -> Result<u64, ApiError> {
    raw_id.parse().map_err(|_| not_found(class, raw_id))
}

/// `DateTime::format('c')`, the shape `Stdlib\DateTime::jsonSerialize` emits.
pub(crate) fn iso8601(at: NaiveDateTime) -> String {
    at.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()
}

/// The `{"@value", "@type"}` wrapper for a resource date.
pub(crate) fn jsonld_date(at: NaiveDateTime) -> JsonLdDate {
    JsonLdDate::new(iso8601(at))
}

/// The three thumbnail types from `thumbnails.types` in `module.config.php`.
pub(crate) const THUMBNAIL_TYPES: [&str; 3] = ["large", "medium", "square"];

/// Upstream `AbstractRepresentation::thumbnailDisplayUrls`: one entry per
/// thumbnail type, `null` when there is nothing to display.
pub(crate) fn empty_thumbnail_urls() -> ThumbnailDisplayUrls {
    ThumbnailDisplayUrls {
        large: None,
        medium: None,
        square: None,
    }
}

/// `AbstractResourceEntityRepresentation::displayTitle` fallback.
pub(crate) const UNTITLED: &str = "[Untitled]";
