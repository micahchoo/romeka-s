//! The ACL for writes, from `Service\AclFactory`.
//!
//! Upstream checks two levels for one write. `Api\Manager::execute` checks the
//! adapter grant, `userIsAllowed($adapter, $operation)`. `AbstractEntityAdapter`
//! then checks the entity grant through its assertions. The two levels carry
//! different rules per resource and per role, so each lives here as a table
//! instead of a branch chain in the request path.
//!
//! A resource with no row in [`ADAPTER_WRITES`] passes the adapter check and
//! reaches the handler. That is the plugin path: an adapter the port does not
//! compile in has no ACL rule here, and the request falls through to the
//! handler the caller named.

use omeka_db::ids::UserId;
use omeka_db::rows::SitePermissionRow;

use crate::auth::Actor;
use crate::read::{entity_denied, permission_denied};
use crate::routes::Operation;

/// The non-admin roles `AclFactory` grants a write privilege on an adapter.
///
/// Both admin roles hold every privilege, because `AclFactory::addRulesForSiteAdmin`
/// and `::addRulesForGlobalAdmin` call `$acl->allow($role)` with no resource
/// list. A `Grant` therefore never names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grant {
    /// Only the two admin roles.
    Admins,
    /// The listed roles, plus both admin roles.
    Roles(&'static [&'static str]),
    /// Every role, the anonymous request included
    /// (`AclFactory::addRulesForSites` allows `SiteAdapter` and
    /// `SitePageAdapter` with no privilege list).
    AnyRole,
}

impl Grant {
    fn allows(self, role: Option<&str>) -> bool {
        match self {
            Self::Admins => is_admin_role(role),
            Self::Roles(roles) => {
                is_admin_role(role) || role.is_some_and(|role| roles.contains(&role))
            }
            Self::AnyRole => true,
        }
    }
}

/// Adapter-level create, update and delete grants, one row per resource.
///
/// The roles are `AclFactory`'s adapter rules: `author`, `reviewer` and
/// `editor` each carry an explicit list (`addRulesForAuthor` and its siblings),
/// `researcher` only ever holds `UserAdapter` update, and a resource absent
/// from every list falls through to the admin-only default. A resource with no
/// row here has no compiled-in handler either.
#[allow(clippy::type_complexity)]
const ADAPTER_WRITES: &[(&str, Grant, Grant, Grant)] = &[
    ("items", RESOURCE, RESOURCE, RESOURCE),
    ("item_sets", RESOURCE, RESOURCE, RESOURCE),
    ("media", RESOURCE, RESOURCE, RESOURCE),
    ("assets", RESOURCE, RESOURCE, RESOURCE),
    ("resource_templates", TEMPLATES, TEMPLATES, TEMPLATES),
    ("vocabularies", Grant::Admins, Grant::Admins, Grant::Admins),
    (
        "resource_classes",
        Grant::Admins,
        Grant::Admins,
        Grant::Admins,
    ),
    ("properties", Grant::Admins, Grant::Admins, Grant::Admins),
    ("jobs", Grant::Admins, Grant::Admins, Grant::Admins),
    (
        "users",
        Grant::Admins,
        Grant::Roles(USER_WRITE_ROLES),
        Grant::Admins,
    ),
    // `AclFactory::addRulesForSites` grants both adapters to every role, the
    // anonymous request included; the entity assertion is what refuses a
    // write. Both admin roles hold every privilege through
    // `addRulesForSiteAdmin` and `addRulesForGlobalAdmin`.
    ("sites", Grant::AnyRole, Grant::AnyRole, Grant::AnyRole),
    ("site_pages", Grant::AnyRole, Grant::AnyRole, Grant::AnyRole),
];

/// `AclFactory`'s resource entity adapters: `ItemAdapter`, `ItemSetAdapter`,
/// `MediaAdapter` and `AssetAdapter` carry the same three-role write list.
const RESOURCE_ROLES: &[&str] = &["author", "reviewer", "editor"];
/// `ResourceTemplateAdapter` carries no `reviewer` grant (`addRulesForReviewer`
/// omits it).
const TEMPLATE_ROLES: &[&str] = &["author", "editor"];
/// `UserAdapter` update is granted to every authenticated role.
const USER_WRITE_ROLES: &[&str] = &["researcher", "author", "reviewer", "editor"];

/// The grant the four resource entity adapters carry.
const RESOURCE: Grant = Grant::Roles(RESOURCE_ROLES);
/// The grant `ResourceTemplateAdapter` carries.
const TEMPLATES: Grant = Grant::Roles(TEMPLATE_ROLES);

/// `Api\Manager::execute`'s general access check for one write operation.
///
/// A resource with no row here has no ACL rule, as an adapter no role list
/// names has none upstream: both admin roles pass, because
/// `AclFactory::addRulesForSiteAdmin` and `::addRulesForGlobalAdmin` call
/// `allow($role)` with no resource list, and every other role is refused. That
/// is the rule the plugin path gets until a plugin registers its own grants.
pub fn adapter(
    actor: &Actor,
    resource: &str,
    operation: Operation,
) -> Result<(), crate::errors::ApiError> {
    // The read operations have their own check (`Api::allow`).
    if matches!(operation, Operation::Search | Operation::Read) {
        return Ok(());
    }
    let Some((_, create, update, delete)) =
        ADAPTER_WRITES.iter().find(|(name, ..)| *name == resource)
    else {
        return if is_admin_role(actor.role()) {
            Ok(())
        } else {
            Err(permission_denied(resource, operation))
        };
    };
    let grant = match operation {
        Operation::Create => *create,
        Operation::Update | Operation::Patch => *update,
        Operation::Delete => *delete,
        Operation::Search | Operation::Read => return Ok(()),
    };
    if grant.allows(actor.role()) {
        Ok(())
    } else {
        Err(permission_denied(resource, operation))
    }
}

/// The entity-level delete privilege.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityDelete {
    /// Any row, whatever its owner.
    AnyRow,
    /// Only a row the caller owns (`OwnsEntityAssertion`).
    OwnRow,
}

/// `AbstractEntityAdapter::authorize($entity, 'delete')` for items, item sets,
/// media and assets: an editor and the admin roles may remove any row; an
/// author and a reviewer only a row they own.
pub fn resource_entity(
    actor: &Actor,
    class: &str,
    owner: Option<UserId>,
) -> Result<(), crate::errors::ApiError> {
    let privilege = match actor.role() {
        Some("global_admin" | "site_admin" | "editor") => EntityDelete::AnyRow,
        Some("author" | "reviewer") => EntityDelete::OwnRow,
        _ => return Err(entity_denied(class, Operation::Delete)),
    };
    allow(actor, privilege, class, owner)
}

/// `AbstractEntityAdapter::authorize($entity, 'update')` for items, item sets,
/// media, assets and resource templates: a reviewer and up may change any row
/// (`addRulesForReviewer` grants update with no assertion); an author only a
/// row they own.
pub fn resource_entity_update(
    actor: &Actor,
    class: &str,
    owner: Option<UserId>,
) -> Result<(), crate::errors::ApiError> {
    let privilege = match actor.role() {
        Some("global_admin" | "site_admin" | "editor" | "reviewer") => EntityDelete::AnyRow,
        Some("author") => EntityDelete::OwnRow,
        _ => return Err(entity_denied(class, Operation::Update)),
    };
    allow(actor, privilege, class, owner)
}

/// `OwnsEntityAssertion` for the site entities: the admin roles may remove any
/// site or page, every other role only one it owns.
///
/// Upstream also accepts a site-editor or site-admin permission on the site.
/// The port's [`Actor`] carries no site role, so ownership is the whole test.
pub fn owned_entity(
    actor: &Actor,
    class: &str,
    owner: Option<UserId>,
) -> Result<(), crate::errors::ApiError> {
    let privilege = match actor.role() {
        Some("global_admin" | "site_admin") => EntityDelete::AnyRow,
        _ => EntityDelete::OwnRow,
    };
    allow(actor, privilege, class, owner)
}

/// `AbstractEntityAdapter::authorize` for `Omeka\Entity\Vocabulary` on update
/// and delete.
///
/// `AclFactory::addRulesForSiteAdmin` denies both privileges unless the site
/// admin owns the row (`AssertionNegation(OwnsEntityAssertion)`), so a site
/// admin may change a vocabulary they added and not a shared one. A global
/// admin is unrestricted. The adapter grant already refuses every other role,
/// so nothing here repeats it.
pub fn vocabulary_entity(
    actor: &Actor,
    class: &str,
    owner: Option<UserId>,
) -> Result<(), crate::errors::ApiError> {
    let privilege = match actor.role() {
        Some("global_admin") => EntityDelete::AnyRow,
        Some("site_admin") => EntityDelete::OwnRow,
        _ => return Err(entity_denied(class, Operation::Update)),
    };
    allow(actor, privilege, class, owner)
}

/// `AbstractEntityAdapter::authorize` for `Omeka\Entity\User` on delete.
///
/// `AclFactory::addRulesForGlobalAdmin` and `::addRulesForSiteAdmin` deny
/// `Entity\User` `delete` to `IsSelfAssertion`, so an admin may not remove
/// their own account, and a site-admin may not remove another admin
/// (`UserIsAdminAssertion`). The adapter grant already refuses every other
/// role, so nothing here repeats it.
pub fn user_entity(
    actor: &Actor,
    class: &str,
    target: UserId,
    target_role: &str,
) -> Result<(), crate::errors::ApiError> {
    let denied = || entity_denied(class, Operation::Delete);
    if actor.user() == Some(target) {
        return Err(denied());
    }
    if actor.role() == Some("site_admin") && is_admin_role(Some(target_role)) {
        return Err(denied());
    }
    Ok(())
}

/// `Acl::isAdminRole`.
pub fn is_admin_role(role: Option<&str>) -> bool {
    matches!(role, Some("global_admin" | "site_admin"))
}

/// A site permission role, numbered the way [`HasSitePermissionAssertion`]
/// numbers it: `admin` 1, `editor` 2, `viewer` 3. A lower number is the
/// stronger role, so `stored <= required` is "at least this role".
///
/// [`HasSitePermissionAssertion`]: https://github.com/omeka/omeka-s/blob/5e413f009c447f4b587803ab23a9cec7d228f6e8/application/src/Permissions/Assertion/HasSitePermissionAssertion.php
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SiteRole {
    Admin,
    Editor,
    Viewer,
}

impl SiteRole {
    /// The assertion's `getRoleNumber`, whose `default` case is `viewer`.
    fn of(role: &str) -> Self {
        match role {
            "admin" => Self::Admin,
            "editor" => Self::Editor,
            _ => Self::Viewer,
        }
    }
}

/// `HasSitePermissionAssertion`: `user` holds at least `required` on the site
/// the permissions belong to.
///
/// An anonymous caller holds no site permission, which the assertion says
/// explicitly before it reads the collection.
pub fn has_site_permission(
    permissions: &[SitePermissionRow],
    user: UserId,
    required: SiteRole,
) -> bool {
    permissions
        .iter()
        .any(|permission| permission.user_id == user && SiteRole::of(&permission.role) <= required)
}

fn allow(
    actor: &Actor,
    privilege: EntityDelete,
    class: &str,
    owner: Option<UserId>,
) -> Result<(), crate::errors::ApiError> {
    match privilege {
        EntityDelete::AnyRow => Ok(()),
        EntityDelete::OwnRow => match (actor.user(), owner) {
            (Some(me), Some(owner)) if me == owner => Ok(()),
            _ => Err(entity_denied(class, Operation::Delete)),
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn author(id: u64) -> Actor {
        Actor::new(UserId(id), "author")
    }

    #[test]
    fn anonymous_cannot_delete_an_item() {
        let error = adapter(&Actor::anonymous(), "items", Operation::Delete).expect_err("denied");
        assert_eq!(error.status(), http::StatusCode::FORBIDDEN);
        assert_eq!(
            error.document().errors,
            crate::errors::ErrorPayload::General(crate::errors::GeneralError {
                error: "Permission denied for the current user to delete the Omeka\\Api\\Adapter\\ItemAdapter resource."
                    .to_string(),
            })
        );
    }

    #[test]
    fn a_researcher_cannot_delete_an_item() {
        let actor = Actor::new(UserId(9), "researcher");
        assert!(adapter(&actor, "items", Operation::Delete).is_err());
    }

    #[test]
    fn an_author_may_delete_their_own_item_only() {
        assert!(adapter(&author(7), "items", Operation::Delete).is_ok());
        assert!(resource_entity(&author(7), "Omeka\\Entity\\Item", Some(UserId(7))).is_ok());
        assert!(resource_entity(&author(7), "Omeka\\Entity\\Item", Some(UserId(8))).is_err());
        assert!(resource_entity(&author(7), "Omeka\\Entity\\Item", None).is_err());
    }

    #[test]
    fn an_editor_may_delete_any_resource_entity() {
        let actor = Actor::new(UserId(3), "editor");
        assert!(resource_entity(&actor, "Omeka\\Entity\\Media", Some(UserId(8))).is_ok());
    }

    #[test]
    fn an_editor_may_delete_a_site_only_when_they_own_it() {
        let actor = Actor::new(UserId(3), "editor");
        assert!(owned_entity(&actor, "Omeka\\Entity\\Site", Some(UserId(3))).is_ok());
        assert!(owned_entity(&actor, "Omeka\\Entity\\Site", Some(UserId(4))).is_err());
    }

    #[test]
    fn only_the_admin_roles_may_delete_a_user() {
        assert!(
            adapter(
                &Actor::new(UserId(1), "global_admin"),
                "users",
                Operation::Delete
            )
            .is_ok()
        );
        assert!(
            adapter(
                &Actor::new(UserId(2), "site_admin"),
                "users",
                Operation::Delete
            )
            .is_ok()
        );
        assert!(adapter(&Actor::new(UserId(3), "editor"), "users", Operation::Delete).is_err());
        assert!(
            adapter(
                &Actor::new(UserId(4), "researcher"),
                "users",
                Operation::Delete
            )
            .is_err()
        );
    }

    #[test]
    fn an_admin_may_not_delete_themselves() {
        let actor = Actor::new(UserId(1), "global_admin");
        assert!(user_entity(&actor, "Omeka\\Entity\\User", UserId(1), "global_admin").is_err());
        assert!(user_entity(&actor, "Omeka\\Entity\\User", UserId(2), "author").is_ok());
    }

    #[test]
    fn a_site_admin_may_not_delete_another_admin() {
        let actor = Actor::new(UserId(2), "site_admin");
        assert!(user_entity(&actor, "Omeka\\Entity\\User", UserId(1), "global_admin").is_err());
        assert!(user_entity(&actor, "Omeka\\Entity\\User", UserId(3), "site_admin").is_err());
        assert!(user_entity(&actor, "Omeka\\Entity\\User", UserId(4), "editor").is_ok());
    }

    #[test]
    fn an_anonymous_caller_reaches_a_site_handler_and_is_refused_by_the_entity_rule() {
        assert!(adapter(&Actor::anonymous(), "sites", Operation::Delete).is_ok());
        assert!(owned_entity(&Actor::anonymous(), "Omeka\\Entity\\Site", None).is_err());
    }

    #[test]
    fn a_resource_without_a_table_entry_is_admin_only() {
        assert!(adapter(&Actor::anonymous(), "plugin_things", Operation::Delete).is_err());
        assert!(adapter(&author(7), "plugin_things", Operation::Create).is_err());
        assert!(
            adapter(
                &Actor::new(UserId(1), "site_admin"),
                "plugin_things",
                Operation::Create
            )
            .is_ok()
        );
        // The read operations answer before the table is consulted.
        assert!(adapter(&Actor::anonymous(), "plugin_things", Operation::Search).is_ok());
    }

    #[test]
    fn only_the_admin_roles_may_write_a_vocabulary() {
        for operation in [Operation::Create, Operation::Update, Operation::Delete] {
            assert!(adapter(&author(7), "vocabularies", operation).is_err());
            assert!(adapter(&Actor::new(UserId(3), "editor"), "vocabularies", operation).is_err());
            assert!(
                adapter(
                    &Actor::new(UserId(1), "global_admin"),
                    "vocabularies",
                    operation
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn a_site_admin_may_change_a_vocabulary_only_when_they_own_it() {
        let actor = Actor::new(UserId(2), "site_admin");
        assert!(vocabulary_entity(&actor, "Omeka\\Entity\\Vocabulary", Some(UserId(2))).is_ok());
        assert!(vocabulary_entity(&actor, "Omeka\\Entity\\Vocabulary", Some(UserId(1))).is_err());
        assert!(vocabulary_entity(&actor, "Omeka\\Entity\\Vocabulary", None).is_err());
        let admin = Actor::new(UserId(1), "global_admin");
        assert!(vocabulary_entity(&admin, "Omeka\\Entity\\Vocabulary", Some(UserId(2))).is_ok());
    }

    #[test]
    fn a_reviewer_may_not_write_a_resource_template() {
        let reviewer = Actor::new(UserId(5), "reviewer");
        assert!(adapter(&reviewer, "resource_templates", Operation::Create).is_err());
        assert!(adapter(&author(7), "resource_templates", Operation::Create).is_ok());
    }

    #[test]
    fn a_reviewer_may_update_any_asset_but_delete_only_their_own() {
        let reviewer = Actor::new(UserId(5), "reviewer");
        assert!(adapter(&reviewer, "assets", Operation::Update).is_ok());
        assert!(resource_entity_update(&reviewer, "Omeka\\Entity\\Asset", Some(UserId(6))).is_ok());
        assert!(resource_entity(&reviewer, "Omeka\\Entity\\Asset", Some(UserId(6))).is_err());
        assert!(resource_entity(&reviewer, "Omeka\\Entity\\Asset", Some(UserId(5))).is_ok());
    }

    #[test]
    fn only_the_admin_roles_may_write_a_job() {
        assert!(adapter(&author(7), "jobs", Operation::Delete).is_err());
        assert!(
            adapter(
                &Actor::new(UserId(1), "global_admin"),
                "jobs",
                Operation::Delete
            )
            .is_ok()
        );
    }
}
