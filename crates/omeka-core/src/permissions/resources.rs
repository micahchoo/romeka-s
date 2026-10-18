/// The `Omeka\Entity\*` kinds an entity privilege is checked against
/// (`Resource`, the joined base, included: `view-all` checks it directly).
use super::Role;
use crate::ids::{SiteId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectKind {
    /// `Omeka\Entity\Item`
    Item,
    /// `Omeka\Entity\ItemSet`
    ItemSet,
    /// `Omeka\Entity\Media`
    Media,
    /// `Omeka\Entity\ResourceTemplate`
    ResourceTemplate,
    /// `Omeka\Entity\Asset`
    Asset,
    /// `Omeka\Entity\ValueAnnotation`
    ValueAnnotation,
    /// `Omeka\Entity\Vocabulary`
    Vocabulary,
    /// `Omeka\Entity\ResourceClass`
    ResourceClass,
    /// `Omeka\Entity\Property`
    Property,
    /// `Omeka\Entity\Resource`, the abstract base used by the `view-all` rule
    Resource,
    /// `Omeka\Entity\Site`
    Site,
    /// `Omeka\Entity\SitePage`
    SitePage,
    /// `Omeka\Entity\User`
    User,
}

/// The `Omeka\Api\Adapter\*` resources, one per registered adapter
/// (`application/config/module.config.php:353-376`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Adapter {
    /// `Omeka\Api\Adapter\ResourceAdapter`
    Resource,
    /// `Omeka\Api\Adapter\ItemSetAdapter`
    ItemSet,
    /// `Omeka\Api\Adapter\ItemAdapter`
    Item,
    /// `Omeka\Api\Adapter\MediaAdapter`
    Media,
    /// `Omeka\Api\Adapter\VocabularyAdapter`
    Vocabulary,
    /// `Omeka\Api\Adapter\ResourceClassAdapter`
    ResourceClass,
    /// `Omeka\Api\Adapter\PropertyAdapter`
    Property,
    /// `Omeka\Api\Adapter\ResourceTemplateAdapter`
    ResourceTemplate,
    /// `Omeka\Api\Adapter\AssetAdapter`
    Asset,
    /// `Omeka\Api\Adapter\SiteAdapter`
    Site,
    /// `Omeka\Api\Adapter\SitePageAdapter`
    SitePage,
    /// `Omeka\Api\Adapter\UserAdapter`
    User,
    /// `Omeka\Api\Adapter\ApiResourceAdapter`
    ApiResource,
    /// `Omeka\Api\Adapter\DataTypeAdapter`
    DataType,
}

/// The `Omeka\Controller\*` resources. The public and site controllers are
/// granted all actions to everyone in one rule (`AclFactory.php:146-160`); the
/// admin controllers carry the per-role action lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Controller {
    /// `Omeka\Controller\Api`
    Api,
    /// `Omeka\Controller\ApiLocal`
    ApiLocal,
    /// `Omeka\Controller\Index`
    Index,
    /// `Omeka\Controller\Login`
    Login,
    /// `Omeka\Controller\Maintenance`
    Maintenance,
    /// `Omeka\Controller\Migrate`
    Migrate,
    /// `Omeka\Controller\Search`
    Search,
    /// `Omeka\Controller\IiifViewer`
    IiifViewer,
    /// `Omeka\Controller\SiteAdmin\Index`
    SiteAdminIndex,
    /// `Omeka\Controller\SiteAdmin\Page`
    SiteAdminPage,
    /// `Omeka\Controller\Site\Index`
    SiteIndex,
    /// `Omeka\Controller\Site\Item`
    SiteItem,
    /// `Omeka\Controller\Site\ItemSet`
    SiteItemSet,
    /// `Omeka\Controller\Site\Media`
    SiteMedia,
    /// `Omeka\Controller\Site\Page`
    SitePage,
    /// `Omeka\Controller\Site\CrossSiteSearch`
    SiteCrossSiteSearch,
    /// `Omeka\Controller\Admin\Index`, the admin dashboard
    AdminIndex,
    /// `Omeka\Controller\Admin\Item`
    AdminItem,
    /// `Omeka\Controller\Admin\ItemSet`
    AdminItemSet,
    /// `Omeka\Controller\Admin\Media`
    AdminMedia,
    /// `Omeka\Controller\Admin\ResourceTemplate`
    AdminResourceTemplate,
    /// `Omeka\Controller\Admin\Vocabulary`
    AdminVocabulary,
    /// `Omeka\Controller\Admin\ResourceClass`
    AdminResourceClass,
    /// `Omeka\Controller\Admin\Property`
    AdminProperty,
    /// `Omeka\Controller\Admin\SystemInfo`
    AdminSystemInfo,
    /// `Omeka\Controller\Admin\Query`
    AdminQuery,
    /// `Omeka\Controller\Admin\Columns`
    AdminColumns,
    /// `Omeka\Controller\Admin\Asset`
    AdminAsset,
    /// `Omeka\Controller\Admin\User`
    AdminUser,
    /// `Omeka\Controller\Admin\Setting`
    AdminSetting,
    /// `Omeka\Controller\Admin\Module`
    AdminModule,
}

/// What is being acted on, tagged with the upstream resource family.
///
/// Build the bound variants with the constructors so the kind and the facts
/// stay consistent; `Context::Entity` is the unbound string check upstream makes
/// with a class name (for example `'Omeka\Entity\User'`, `'change-role'` before
/// the row exists). An API write is the conjunction of the adapter check and the
/// bound entity check (`AbstractEntityRepresentation::userIsAllowed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// An unbound entity check (`userIsAllowed('Omeka\Entity\Item', $privilege)`).
    Entity(SubjectKind),
    /// An adapter operation (`Api\Manager::execute`).
    Adapter(Adapter),
    /// A controller action (`MvcListeners`).
    Controller(Controller),
    /// `Omeka\Module\Manager` (`Module\Manager::isAllowed`).
    Module,
    /// A bound entity that carries `resource.owner_id`.
    Owned {
        /// Which entity.
        kind: SubjectKind,
        /// `resource.owner_id`, null after the owner is deleted
        /// (`ON DELETE SET NULL`).
        owner: Option<UserId>,
    },
    /// A site row.
    Site {
        /// `site.id`.
        id: SiteId,
        /// `site.owner_id`.
        owner: Option<UserId>,
        /// `site.is_public`.
        is_public: bool,
    },
    /// A site page row. Ownership follows the page's site
    /// (`SitePage::getOwner` delegates to the site).
    SitePage {
        /// The page's site.
        site: SiteId,
        /// The site's owner.
        site_owner: Option<UserId>,
        /// `site.is_public`.
        site_is_public: bool,
        /// `site_page.is_public`.
        is_public: bool,
    },
    /// A user row.
    User {
        /// `user.id`.
        id: UserId,
        /// `user.role`.
        role: Role,
    },
}

impl Context {
    /// An unbound entity check by class name.
    #[must_use]
    pub const fn entity(kind: SubjectKind) -> Self {
        Self::Entity(kind)
    }

    /// An adapter operation check.
    #[must_use]
    pub const fn adapter(adapter: Adapter) -> Self {
        Self::Adapter(adapter)
    }

    /// A controller action check.
    #[must_use]
    pub const fn controller(controller: Controller) -> Self {
        Self::Controller(controller)
    }

    /// The module manager check.
    #[must_use]
    pub const fn module() -> Self {
        Self::Module
    }

    /// A bound entity of a caller-chosen kind.
    #[must_use]
    pub const fn owned(kind: SubjectKind, owner: Option<UserId>) -> Self {
        Self::Owned { kind, owner }
    }

    /// An item row.
    #[must_use]
    pub const fn item(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::Item, owner)
    }

    /// An item set row.
    #[must_use]
    pub const fn item_set(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::ItemSet, owner)
    }

    /// A media row.
    #[must_use]
    pub const fn media(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::Media, owner)
    }

    /// A resource template row.
    #[must_use]
    pub const fn resource_template(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::ResourceTemplate, owner)
    }

    /// An asset row.
    #[must_use]
    pub const fn asset(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::Asset, owner)
    }

    /// A value annotation row, carrying the owner the checked row has.
    #[must_use]
    pub const fn value_annotation(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::ValueAnnotation, owner)
    }

    /// A vocabulary row.
    #[must_use]
    pub const fn vocabulary(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::Vocabulary, owner)
    }

    /// A resource class row.
    #[must_use]
    pub const fn resource_class(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::ResourceClass, owner)
    }

    /// A property row.
    #[must_use]
    pub const fn property(owner: Option<UserId>) -> Self {
        Self::owned(SubjectKind::Property, owner)
    }

    /// The entity kind this context binds, for the entity-target rules.
    pub(super) fn entity_kind(&self) -> Option<SubjectKind> {
        match self {
            Self::Entity(kind) | Self::Owned { kind, .. } => Some(*kind),
            Self::Site { .. } => Some(SubjectKind::Site),
            Self::SitePage { .. } => Some(SubjectKind::SitePage),
            Self::User { .. } => Some(SubjectKind::User),
            Self::Adapter(_) | Self::Controller(_) | Self::Module => None,
        }
    }

    /// The owner for the ownership assertion. A site page's owner is its site's
    /// owner (`SitePage::getOwner`).
    pub(super) fn owner(&self) -> Option<UserId> {
        match self {
            Self::Owned { owner, .. } => *owner,
            Self::Site { owner, .. }
            | Self::SitePage {
                site_owner: owner, ..
            } => *owner,
            Self::Entity(_) | Self::Adapter(_) | Self::Controller(_) | Self::Module => None,
            Self::User { .. } => None,
        }
    }

    /// The site a site permission is looked up on, for sites and site pages.
    pub(super) fn site(&self) -> Option<SiteId> {
        match self {
            Self::Site { id, .. } => Some(*id),
            Self::SitePage { site, .. } => Some(*site),
            _ => None,
        }
    }
}

/// The privileges the API and admin expose. Adapter operations and entity
/// privileges share the names upstream shares (`create`, `update`, `search`,
/// `batch_update`, ...); controller actions carry the `Admin` prefix and the
/// module manager privileges their own names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    // Adapter operations and entity privileges.
    /// `search`
    Search,
    /// `read`
    Read,
    /// `create`
    Create,
    /// `update`
    Update,
    /// `delete`
    Delete,
    /// `batch_update`
    BatchUpdate,
    /// `batch_update_all`
    BatchUpdateAll,
    /// `batch_delete`
    BatchDelete,
    /// `batch_delete_all`
    BatchDeleteAll,
    /// `view-all` on `Omeka\Entity\Resource`
    ViewAll,
    /// `add-page` on a site
    AddPage,
    /// `can-assign-items` on a site
    CanAssignItems,
    // User entity privileges (`UserAdapter::hydrate`, `UserController`).
    /// `change-password`
    ChangePassword,
    /// `edit-keys`
    EditKeys,
    /// `change-role`
    ChangeRole,
    /// `change-role-admin`
    ChangeRoleAdmin,
    /// `activate-user`
    ActivateUser,
    // Module privileges (`Omeka\Module\Manager`).
    /// `activate`
    ModuleActivate,
    /// `deactivate`
    ModuleDeactivate,
    /// `install`
    ModuleInstall,
    /// `uninstall`
    ModuleUninstall,
    /// `upgrade`
    ModuleUpgrade,
    /// `configure`
    ModuleConfigure,
    // Admin controller actions.
    /// `index`, on the dashboard and the listed controllers
    AdminIndex,
    /// `browse`
    AdminBrowse,
    /// `show`
    AdminShow,
    /// `show-details`
    AdminShowDetails,
    /// `add`
    AdminAdd,
    /// `edit`
    AdminEdit,
    /// `delete`
    AdminDelete,
    /// `delete-confirm`
    AdminDeleteConfirm,
    /// `batch-edit`
    AdminBatchEdit,
    /// `batch-edit-all`
    AdminBatchEditAll,
    /// `batch-delete`
    AdminBatchDelete,
    /// `batch-delete-all`
    AdminBatchDeleteAll,
    /// `sidebar-select`
    AdminSidebarSelect,
    /// `search` on a controller
    AdminSearch,
    /// `sidebar-edit`
    AdminSidebarEdit,
    /// `sidebar-preview`
    AdminSidebarPreview,
    /// `search-filters`
    AdminSearchFilters,
    /// `column-list`
    AdminColumnList,
    /// `column-row`
    AdminColumnRow,
    /// `column-edit-sidebar`
    AdminColumnEditSidebar,
    /// `classes`
    AdminClasses,
    /// `properties`
    AdminProperties,
    /// `add-new-property-row`
    AdminAddNewPropertyRow,
    /// `import`
    AdminImport,
}
