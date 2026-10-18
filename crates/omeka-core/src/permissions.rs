//! The permission matrix, mirroring `application/src/Service/AclFactory.php`,
//! `application/src/Permissions/Acl.php` and `application/src/Permissions/Assertion/*`.
//!
//! Upstream builds a Laminas ACL at runtime: roles, resources and rules added in
//! order, with the last matching rule winning, and assertions that read the
//! bound resource. The port keeps the same rules as a static table evaluated in
//! the same order, so an allow/deny outcome matches upstream for the same actor,
//! privilege and resource. Every rule cites the upstream line it comes from.
//!
//! Upstream names three families of resources, and a check is only ever made
//! against one family at a time:
//!
//! - `Omeka\Entity\*` entities, checked with a privilege (`read`, `update`,
//!   `view-all`, ...) against a string name or a bound row.
//! - `Omeka\Api\Adapter\*` adapters, checked by `Api\Manager::execute` with the
//!   request operation (`search`, `create`, `batch_update`, ...).
//! - `Omeka\Controller\*` controllers, checked by `MvcListeners` with the
//!   matched action (`browse`, `add`, `sidebar-select`, ...).
//!
//! [`Context`] names the family, so `Context::adapter(Adapter::Item)` and
//! `Context::item(Some(owner))` cannot collide even though both concern items.
//! An API write pairs the two checks the way `AbstractEntityRepresentation::userIsAllowed`
//! does: `allows(actor, op, &Context::adapter(..)) && allows(actor, op, &bound_context)`.
//!
//! Three things upstream settles that DESIGN.md does not say:
//! - Upstream has six roles, not five. `reviewer` sits between `author` and
//!   `editor` (`Acl.php:10-15`).
//! - Site permissions (`admin`/`editor`/`viewer`, `Entity/SitePermission.php`)
//!   are a different enum from the global roles; the actor's site rows live in
//!   [`RoleSet::sites`] keyed by [`SiteId`].
//! - Owning a row and having a site permission both need the actor's
//!   `UserId`, so [`RoleSet`] carries it.
//!
//! Unauthenticated actors are first-class here, exactly as upstream's
//! `$acl->allow(null, ...)` rules make them: the public controllers and the
//! public API reads work logged out. Whether a *non-public* resource is
//! returned is a query filter, not an ACL decision.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{SiteId, UserId};

/// The global roles upstream registers, in ascending privilege order
/// (`AclFactory::addRoles`, `Acl.php:10-15`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Role {
    /// `researcher`
    Researcher,
    /// `author`
    Author,
    /// `reviewer`
    Reviewer,
    /// `editor`
    Editor,
    /// `site_admin`; the UI label is "Supervisor" (`Acl.php:22`)
    SiteAdmin,
    /// `global_admin`
    GlobalAdmin,
}

impl Role {
    /// The string stored in `user.role`.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Researcher => "researcher",
            Self::Author => "author",
            Self::Reviewer => "reviewer",
            Self::Editor => "editor",
            Self::SiteAdmin => "site_admin",
            Self::GlobalAdmin => "global_admin",
        }
    }

    /// Parse a `user.role` column value. Unknown values become `None`, which the
    /// DB boundary must reject rather than guess (`UserAdapter::validateEntity`).
    #[must_use]
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "researcher" => Some(Self::Researcher),
            "author" => Some(Self::Author),
            "reviewer" => Some(Self::Reviewer),
            "editor" => Some(Self::Editor),
            "site_admin" => Some(Self::SiteAdmin),
            "global_admin" => Some(Self::GlobalAdmin),
            _ => None,
        }
    }

    /// Role labels, from `Acl::$roleLabels`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlobalAdmin => "Global Administrator",
            Self::SiteAdmin => "Supervisor",
            Self::Editor => "Editor",
            Self::Reviewer => "Reviewer",
            Self::Author => "Author",
            Self::Researcher => "Researcher",
        }
    }

    /// `Acl::isAdminRole`: global_admin and site_admin are the roles the
    /// site-admin denies single out (`Acl.php:34-37`).
    #[must_use]
    pub const fn is_admin(self) -> bool {
        matches!(self, Self::SiteAdmin | Self::GlobalAdmin)
    }

    /// Rank, ascending. `Role`'s `Ord` is the rank.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Researcher => 0,
            Self::Author => 1,
            Self::Reviewer => 2,
            Self::Editor => 3,
            Self::SiteAdmin => 4,
            Self::GlobalAdmin => 5,
        }
    }
}

/// A per-site role (`application/src/Entity/SitePermission.php:16-18`). This is
/// not a global [`Role`]: it names what a user may do inside one site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SitePermissionRole {
    /// The site administrator; role number 1.
    Admin,
    /// Role number 2.
    Editor,
    /// Role number 3, the default for an unknown stored value.
    Viewer,
}

impl SitePermissionRole {
    /// The string stored in `site_permission.role`.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    /// Parse a `site_permission.role` column value.
    #[must_use]
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "admin" => Some(Self::Admin),
            "editor" => Some(Self::Editor),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }

    /// `HasSitePermissionAssertion::getRoleNumber`, which numbers the roles
    /// starting at admin and falls back to viewer for anything else.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Admin => 1,
            Self::Editor => 2,
            Self::Viewer => 3,
        }
    }
}

/// Who is asking. `global` is the `user.role` column; `sites` is the user's rows
/// in `site_permission`; `user` is the identity the ownership and self checks
/// need. `global: None` is the unauthenticated actor (`Acl::userIsAllowed`
/// passes a null role when nothing is authenticated).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoleSet {
    /// The actor's `user.id`, absent when unauthenticated.
    pub user: Option<UserId>,
    /// The actor's global role.
    pub global: Option<Role>,
    /// The actor's site permissions, keyed by site.
    pub sites: BTreeMap<SiteId, SitePermissionRole>,
}

impl RoleSet {
    /// Nobody is logged in.
    #[must_use]
    pub fn anonymous() -> Self {
        Self::default()
    }

    /// A user with their global role.
    #[must_use]
    pub fn for_user(user: UserId, role: Role) -> Self {
        Self {
            user: Some(user),
            global: Some(role),
            sites: BTreeMap::new(),
        }
    }

    /// Add or replace a site permission.
    #[must_use]
    pub fn with_site(mut self, site: SiteId, role: SitePermissionRole) -> Self {
        self.sites.insert(site, role);
        self
    }

    /// The actor's permission on a site, if any.
    #[must_use]
    pub fn site_permission(&self, site: SiteId) -> Option<SitePermissionRole> {
        self.sites.get(&site).copied()
    }

    /// The user id, when authenticated. Convenience for the assertions.
    pub(super) fn actor(&self) -> Option<UserId> {
        self.user
    }
}

mod resources;
mod rules;
#[cfg(test)]
mod tests;

pub use resources::{Action, Adapter, Context, Controller, SubjectKind};
pub use rules::allows;
