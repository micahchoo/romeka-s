use super::*;
use crate::ids::{SiteId, UserId};

fn user() -> UserId {
    UserId::new(1)
}

fn other() -> UserId {
    UserId::new(2)
}

fn site() -> SiteId {
    SiteId::new(10)
}

fn author() -> RoleSet {
    RoleSet::for_user(user(), Role::Author)
}

fn reviewer() -> RoleSet {
    RoleSet::for_user(user(), Role::Reviewer)
}

fn editor() -> RoleSet {
    RoleSet::for_user(user(), Role::Editor)
}

fn researcher() -> RoleSet {
    RoleSet::for_user(user(), Role::Researcher)
}

fn site_admin() -> RoleSet {
    RoleSet::for_user(user(), Role::SiteAdmin)
}

fn global_admin() -> RoleSet {
    RoleSet::for_user(user(), Role::GlobalAdmin)
}

/// AclFactory.php:256-307: search and read are public for the listed adapters
/// and entities, so the public API works logged out; the user adapter and
/// user entity are not on either list.
#[test]
fn permissions_anonymous_can_search_and_read_public_resources() {
    let anonymous = RoleSet::anonymous();
    assert!(allows(
        &anonymous,
        Action::Search,
        &Context::adapter(Adapter::Item)
    ));
    assert!(allows(
        &anonymous,
        Action::Read,
        &Context::adapter(Adapter::Vocabulary)
    ));
    assert!(allows(
        &anonymous,
        Action::Search,
        &Context::adapter(Adapter::DataType)
    ));
    assert!(allows(
        &anonymous,
        Action::Read,
        &Context::entity(SubjectKind::Item)
    ));
    assert!(!allows(
        &anonymous,
        Action::Create,
        &Context::adapter(Adapter::Item)
    ));
    assert!(!allows(
        &anonymous,
        Action::Read,
        &Context::adapter(Adapter::User)
    ));
    assert!(!allows(
        &anonymous,
        Action::Read,
        &Context::entity(SubjectKind::User)
    ));
}

/// AclFactory.php:146-165: the public and site controllers and the site
/// adapters are allowed to everyone, without assertions.
#[test]
fn permissions_public_controllers_and_site_adapters_are_open_to_anonymous() {
    let anonymous = RoleSet::anonymous();
    assert!(allows(
        &anonymous,
        Action::AdminBrowse,
        &Context::controller(Controller::Login)
    ));
    assert!(allows(
        &anonymous,
        Action::AdminShow,
        &Context::controller(Controller::SiteItem)
    ));
    assert!(allows(
        &anonymous,
        Action::Read,
        &Context::adapter(Adapter::Site)
    ));
    assert!(allows(
        &anonymous,
        Action::Search,
        &Context::adapter(Adapter::SitePage)
    ));
}

/// AclFactory.php:497-510 and :524-552 with OwnsEntityAssertion: the
/// adapter check is role-only, the bound entity check needs ownership; the
/// value annotation has no ownership assertion.
#[test]
fn permissions_author_adapter_crud_and_owned_entity_writes() {
    assert!(allows(
        &author(),
        Action::Create,
        &Context::adapter(Adapter::Item)
    ));
    assert!(allows(
        &author(),
        Action::Delete,
        &Context::adapter(Adapter::Item)
    ));
    assert!(allows(
        &author(),
        Action::Update,
        &Context::item(Some(user()))
    ));
    assert!(!allows(
        &author(),
        Action::Update,
        &Context::item(Some(other()))
    ));
    assert!(!allows(&author(), Action::Update, &Context::item(None)));
    assert!(allows(
        &author(),
        Action::Update,
        &Context::value_annotation(Some(other()))
    ));
}

/// AclFactory.php:675-727: a reviewer may update any item (create and update
/// carry no assertion) but may delete only owned ones; the adapter delete
/// has no assertion.
#[test]
fn permissions_reviewer_updates_any_but_entity_delete_needs_ownership() {
    assert!(allows(
        &reviewer(),
        Action::Update,
        &Context::item(Some(other()))
    ));
    assert!(allows(
        &reviewer(),
        Action::Delete,
        &Context::adapter(Adapter::Item)
    ));
    assert!(!allows(
        &reviewer(),
        Action::Delete,
        &Context::item(Some(other()))
    ));
    assert!(allows(
        &reviewer(),
        Action::Delete,
        &Context::item(Some(user()))
    ));
    assert!(!allows(
        &reviewer(),
        Action::Delete,
        &Context::item_set(None)
    ));
}

/// AclFactory.php:858-885: an editor creates, updates and deletes any of the
/// listed adapter and entity resources; no ownership assertion.
#[test]
fn permissions_editor_entity_crud_covers_any_owner() {
    assert!(allows(
        &editor(),
        Action::Delete,
        &Context::item(Some(other()))
    ));
    assert!(allows(
        &editor(),
        Action::Update,
        &Context::value_annotation(Some(other()))
    ));
    assert!(allows(
        &editor(),
        Action::Delete,
        &Context::adapter(Adapter::ResourceTemplate)
    ));
    // Vocabulary is not on the editor's entity list (AclFactory.php:873).
    assert!(!allows(
        &editor(),
        Action::Create,
        &Context::entity(SubjectKind::Vocabulary)
    ));
}

/// AclFactory.php:729-733 and :915-919: only reviewer and up may see
/// non-public resources through `Omeka\Entity\Resource` `view-all`.
#[test]
fn permissions_view_all_matches_upstream_resource_entity_rule() {
    let resource = Context::entity(SubjectKind::Resource);
    assert!(!allows(&RoleSet::anonymous(), Action::ViewAll, &resource));
    assert!(!allows(&author(), Action::ViewAll, &resource));
    assert!(!allows(&researcher(), Action::ViewAll, &resource));
    assert!(allows(&reviewer(), Action::ViewAll, &resource));
    assert!(allows(&editor(), Action::ViewAll, &resource));
}

/// AclFactory.php:219-226 with SiteIsPublicAssertion: a public site reads
/// for anyone; a private one only for its owner or a viewer.
#[test]
fn permissions_site_read_matches_upstream_site_is_public_assertion() {
    let public = Context::Site {
        id: site(),
        owner: Some(other()),
        is_public: true,
    };
    let private = Context::Site {
        id: site(),
        owner: Some(other()),
        is_public: false,
    };
    let private_owned = Context::Site {
        id: site(),
        owner: Some(user()),
        is_public: false,
    };
    assert!(allows(&RoleSet::anonymous(), Action::Read, &public));
    assert!(!allows(&RoleSet::anonymous(), Action::Read, &private));
    assert!(!allows(&author(), Action::Read, &private));
    assert!(allows(&author(), Action::Read, &private_owned));
    assert!(allows(
        &author().with_site(site(), SitePermissionRole::Viewer),
        Action::Read,
        &private
    ));
}

/// AclFactory.php:231-238 with SitePageIsPublicAssertion: a page reads when
/// both the page and its site are public, or for owner and viewers.
#[test]
fn permissions_site_page_read_matches_upstream_page_and_site_public_assertion() {
    let page =
        |site_owner: Option<UserId>, site_is_public: bool, is_public: bool| Context::SitePage {
            site: site(),
            site_owner,
            site_is_public,
            is_public,
        };
    assert!(allows(
        &RoleSet::anonymous(),
        Action::Read,
        &page(Some(other()), true, true)
    ));
    assert!(!allows(
        &RoleSet::anonymous(),
        Action::Read,
        &page(Some(other()), true, false)
    ));
    assert!(!allows(
        &RoleSet::anonymous(),
        Action::Read,
        &page(Some(other()), false, true)
    ));
    assert!(allows(
        &author(),
        Action::Read,
        &page(Some(user()), false, false)
    ));
    assert!(!allows(
        &author(),
        Action::Read,
        &page(Some(other()), false, false)
    ));
    assert!(allows(
        &author().with_site(site(), SitePermissionRole::Viewer),
        Action::Read,
        &page(Some(other()), false, false)
    ));
}

/// AclFactory.php:167-171: only an editor may create a site; :179-184:
/// anyone authenticated who owns a site may delete it.
#[test]
fn permissions_site_create_and_delete_match_upstream_site_rules() {
    assert!(allows(
        &editor(),
        Action::Create,
        &Context::entity(SubjectKind::Site)
    ));
    assert!(!allows(
        &author(),
        Action::Create,
        &Context::entity(SubjectKind::Site)
    ));
    assert!(!allows(
        &RoleSet::anonymous(),
        Action::Create,
        &Context::entity(SubjectKind::Site)
    ));

    let owned = Context::Site {
        id: site(),
        owner: Some(user()),
        is_public: true,
    };
    let foreign = Context::Site {
        id: site(),
        owner: Some(other()),
        is_public: true,
    };
    assert!(allows(&author(), Action::Delete, &owned));
    assert!(!allows(&author(), Action::Delete, &foreign));
    assert!(
        !allows(&editor(), Action::Delete, &foreign),
        "the editor role has no site delete rule of its own; only site_admin and up allow all"
    );
}

/// AclFactory.php:186-212 with HasSitePermissionAssertion: a site admin
/// updates the site and a site editor may add pages, but a viewer may not;
/// the permission must be on the site being checked.
#[test]
fn permissions_site_update_and_add_page_match_upstream_site_permissions() {
    let site_ctx = |id: SiteId| Context::Site {
        id,
        owner: Some(other()),
        is_public: true,
    };
    let admin = researcher().with_site(site(), SitePermissionRole::Admin);
    let site_editor = researcher().with_site(site(), SitePermissionRole::Editor);
    let viewer = researcher().with_site(site(), SitePermissionRole::Viewer);

    assert!(allows(&admin, Action::Update, &site_ctx(site())));
    assert!(!allows(&site_editor, Action::Update, &site_ctx(site())));
    assert!(allows(&site_editor, Action::AddPage, &site_ctx(site())));
    assert!(allows(&admin, Action::AddPage, &site_ctx(site())));
    assert!(!allows(&viewer, Action::AddPage, &site_ctx(site())));
    assert!(allows(
        &author(),
        Action::AddPage,
        &Context::Site {
            id: site(),
            owner: Some(user()),
            is_public: true,
        }
    ));
    assert!(!allows(&admin, Action::Update, &site_ctx(SiteId::new(99))));
}

/// AclFactory.php:207-212: page update and delete follow the proposing role
/// or an editor permission on the page's site; page ownership is the site owner.
#[test]
fn permissions_site_page_update_and_delete_match_upstream_page_rules() {
    let foreign_page = Context::SitePage {
        site: site(),
        site_owner: Some(other()),
        site_is_public: true,
        is_public: true,
    };
    assert!(allows(
        &researcher().with_site(site(), SitePermissionRole::Editor),
        Action::Update,
        &foreign_page
    ));
    assert!(allows(
        &researcher().with_site(site(), SitePermissionRole::Editor),
        Action::Delete,
        &foreign_page
    ));
    assert!(!allows(
        &researcher().with_site(site(), SitePermissionRole::Viewer),
        Action::Delete,
        &foreign_page
    ));
    assert!(!allows(&author(), Action::Delete, &foreign_page));
    let owner_page = Context::SitePage {
        site: site(),
        site_owner: Some(user()),
        site_is_public: false,
        is_public: false,
    };
    assert!(allows(&author(), Action::Delete, &owner_page));
}

/// AclFactory.php:243-249: `can-assign-items` for the site owner, a site
/// admin or a site editor, and nobody else.
#[test]
fn permissions_can_assign_items_matches_upstream_site_rule() {
    let owned_by_actor = Context::Site {
        id: site(),
        owner: Some(user()),
        is_public: false,
    };
    let owned_by_other = Context::Site {
        id: site(),
        owner: Some(other()),
        is_public: false,
    };
    assert!(allows(
        &researcher(),
        Action::CanAssignItems,
        &owned_by_actor
    ));
    assert!(allows(
        &author().with_site(site(), SitePermissionRole::Editor),
        Action::CanAssignItems,
        &owned_by_other
    ));
    assert!(allows(
        &author().with_site(site(), SitePermissionRole::Admin),
        Action::CanAssignItems,
        &owned_by_other
    ));
    assert!(!allows(
        &author().with_site(site(), SitePermissionRole::Viewer),
        Action::CanAssignItems,
        &owned_by_other
    ));
    assert!(!allows(
        &RoleSet::anonymous().with_site(site(), SitePermissionRole::Viewer),
        Action::CanAssignItems,
        &owned_by_other
    ));
}

/// AclFactory.php:949-958: a site admin may do everything except manage
/// modules and use the settings controller.
#[test]
fn permissions_site_admin_denied_module_and_setting_management() {
    assert!(!allows(
        &site_admin(),
        Action::ModuleActivate,
        &Context::module()
    ));
    assert!(!allows(
        &site_admin(),
        Action::ModuleConfigure,
        &Context::controller(Controller::AdminModule)
    ));
    assert!(allows(
        &global_admin(),
        Action::ModuleActivate,
        &Context::module()
    ));
    assert!(allows(
        &global_admin(),
        Action::ModuleInstall,
        &Context::controller(Controller::AdminModule)
    ));
    assert!(!allows(
        &site_admin(),
        Action::AdminBrowse,
        &Context::controller(Controller::AdminSetting)
    ));
    assert!(allows(
        &site_admin(),
        Action::Create,
        &Context::entity(SubjectKind::Vocabulary)
    ));
    assert!(allows(
        &site_admin(),
        Action::Update,
        &Context::item(Some(other()))
    ));
}

/// AclFactory.php:959-964: a site admin may update or delete a vocabulary
/// only when they own it.
#[test]
fn permissions_site_admin_vocabulary_change_requires_ownership() {
    assert!(!allows(
        &site_admin(),
        Action::Update,
        &Context::vocabulary(Some(other()))
    ));
    assert!(!allows(
        &site_admin(),
        Action::Delete,
        &Context::vocabulary(None)
    ));
    assert!(allows(
        &site_admin(),
        Action::Update,
        &Context::vocabulary(Some(user()))
    ));
    assert!(allows(
        &site_admin(),
        Action::Delete,
        &Context::vocabulary(Some(user()))
    ));
}

/// AclFactory.php:966-976: a site admin may not touch admin roles and may not
/// change, activate or delete their own account.
#[test]
fn permissions_site_admin_user_rules_match_upstream_denies() {
    let self_user = Context::User {
        id: user(),
        role: Role::SiteAdmin,
    };
    let other_user = Context::User {
        id: other(),
        role: Role::Author,
    };
    assert!(!allows(&site_admin(), Action::ChangeRoleAdmin, &other_user));
    assert!(!allows(
        &site_admin(),
        Action::ChangeRoleAdmin,
        &Context::entity(SubjectKind::User)
    ));
    assert!(!allows(&site_admin(), Action::ChangeRole, &self_user));
    assert!(!allows(&site_admin(), Action::ActivateUser, &self_user));
    assert!(!allows(&site_admin(), Action::Delete, &self_user));
    assert!(allows(&site_admin(), Action::ChangeRole, &other_user));
    assert!(allows(&site_admin(), Action::ActivateUser, &other_user));
    assert!(allows(&site_admin(), Action::Delete, &other_user));
}

/// AclFactory.php:978-990 with UserIsAdminAssertion: a site admin may not
/// edit, password, key-edit or delete another admin, but may do so to a
/// non-admin; only `delete` is also denied on their own account (:971).
#[test]
fn permissions_site_admin_cannot_edit_other_admin_users() {
    let other_admin = Context::User {
        id: other(),
        role: Role::GlobalAdmin,
    };
    let self_account = Context::User {
        id: user(),
        role: Role::SiteAdmin,
    };
    let other_author = Context::User {
        id: other(),
        role: Role::Author,
    };
    for action in [
        Action::Update,
        Action::ChangePassword,
        Action::EditKeys,
        Action::Delete,
    ] {
        assert!(
            !allows(&site_admin(), action, &other_admin),
            "{action:?} on another admin must be denied"
        );
    }
    for action in [Action::Update, Action::ChangePassword, Action::EditKeys] {
        assert!(
            allows(&site_admin(), action, &self_account),
            "{action:?} on self must be allowed"
        );
    }
    assert!(!allows(&site_admin(), Action::Delete, &self_account));
    assert!(allows(&site_admin(), Action::Update, &other_author));
    assert!(allows(&site_admin(), Action::Delete, &other_author));
}

/// AclFactory.php:1001-1006: a global admin may do everything except change,
/// activate or delete their own account.
#[test]
fn permissions_global_admin_denied_self_account_changes() {
    let self_user = Context::User {
        id: user(),
        role: Role::GlobalAdmin,
    };
    let other_admin = Context::User {
        id: other(),
        role: Role::GlobalAdmin,
    };
    assert!(!allows(&global_admin(), Action::Delete, &self_user));
    assert!(!allows(&global_admin(), Action::ChangeRole, &self_user));
    assert!(!allows(&global_admin(), Action::ActivateUser, &self_user));
    assert!(allows(&global_admin(), Action::Delete, &other_admin));
    assert!(allows(&global_admin(), Action::Update, &self_user));
}

/// AclFactory.php:389-394 and the roles that repeat it: a non-admin user may
/// update, change the password of and edit the keys of their own account
/// only, may read any user, and owns no other user privilege.
#[test]
fn permissions_non_admin_user_is_self_rules_are_entity_only() {
    let self_user = Context::User {
        id: user(),
        role: Role::Author,
    };
    let other_user = Context::User {
        id: other(),
        role: Role::Author,
    };
    for actor in [researcher(), author(), reviewer(), editor()] {
        assert!(allows(&actor, Action::Update, &self_user));
        assert!(allows(&actor, Action::ChangePassword, &self_user));
        assert!(allows(&actor, Action::EditKeys, &self_user));
        assert!(!allows(&actor, Action::Update, &other_user));
        assert!(!allows(&actor, Action::ChangePassword, &other_user));
        assert!(allows(&actor, Action::Read, &other_user));
        assert!(!allows(&actor, Action::ChangeRole, &self_user));
    }
}

/// The three surfaces stay separate: an author reaches every `Admin\User`
/// controller action and the user adapter's `read`/`update`/`search`, but
/// the entity privileges still run the self assertion.
#[test]
fn permissions_user_controller_actions_do_not_grant_entity_privileges() {
    assert!(allows(
        &author(),
        Action::AdminBrowse,
        &Context::controller(Controller::AdminUser)
    ));
    assert!(allows(
        &author(),
        Action::Update,
        &Context::adapter(Adapter::User)
    ));
    let other_user = Context::User {
        id: other(),
        role: Role::Author,
    };
    assert!(!allows(&author(), Action::Update, &other_user));
    assert!(!allows(&author(), Action::ChangeRole, &other_user));
    assert!(!allows(
        &author(),
        Action::Update,
        &Context::item(Some(other()))
    ));
}

/// AclFactory.php:173-177: `Omeka\Entity\SitePage` `create` is allowed with
/// a null role, so even an unauthenticated request passes the ACL. Upstream
/// guards the action elsewhere; the matrix records the rule as written.
#[test]
fn permissions_site_page_create_matches_upstream_null_role_allow() {
    assert!(allows(
        &RoleSet::anonymous(),
        Action::Create,
        &Context::entity(SubjectKind::SitePage)
    ));
    assert!(allows(
        &author(),
        Action::Create,
        &Context::entity(SubjectKind::SitePage)
    ));
}

/// AclFactory.php:485-523 and :662-701: batch privileges grow with the role.
#[test]
fn permissions_batch_privileges_grow_with_role_like_upstream() {
    let item = Context::adapter(Adapter::Item);
    assert!(allows(&author(), Action::BatchUpdate, &item));
    assert!(!allows(&author(), Action::BatchUpdateAll, &item));
    assert!(!allows(&author(), Action::BatchDeleteAll, &item));
    assert!(allows(&reviewer(), Action::BatchUpdateAll, &item));
    assert!(!allows(&reviewer(), Action::BatchDeleteAll, &item));
    assert!(allows(&editor(), Action::BatchDeleteAll, &item));
    assert!(!allows(&researcher(), Action::BatchUpdate, &item));
    assert!(allows(&site_admin(), Action::BatchDeleteAll, &item));
}

/// AclFactory.php:445-449 and :804-808: only author and editor (and up) may
/// add a property row or import a template; a reviewer may not.
#[test]
fn permissions_template_import_matches_upstream_controller_rules() {
    let template = Context::controller(Controller::AdminResourceTemplate);
    assert!(allows(&author(), Action::AdminAddNewPropertyRow, &template));
    assert!(allows(&author(), Action::AdminImport, &template));
    assert!(allows(&editor(), Action::AdminAddNewPropertyRow, &template));
    assert!(!allows(&reviewer(), Action::AdminImport, &template));
    assert!(!allows(&researcher(), Action::AdminImport, &template));
}

/// AclFactory.php:344-356 and :844-857: batch-edit-all and batch-delete-all
/// appear at reviewer and editor respectively, and only for items and sets.
#[test]
fn permissions_admin_batch_actions_match_upstream_controller_rules() {
    let item = Context::controller(Controller::AdminItem);
    assert!(!allows(&author(), Action::AdminBatchEditAll, &item));
    assert!(allows(&reviewer(), Action::AdminBatchEditAll, &item));
    assert!(!allows(&reviewer(), Action::AdminBatchDeleteAll, &item));
    assert!(allows(&editor(), Action::AdminBatchDeleteAll, &item));

    // Reviewer controller rules cover item, item set and media; the
    // ResourceTemplate controller is add-less for the reviewer.
    assert!(allows(
        &reviewer(),
        Action::AdminEdit,
        &Context::controller(Controller::AdminMedia)
    ));
    assert!(!allows(
        &reviewer(),
        Action::AdminAdd,
        &Context::controller(Controller::AdminResourceTemplate)
    ));
    assert!(allows(
        &author(),
        Action::AdminAdd,
        &Context::controller(Controller::AdminResourceTemplate)
    ));
}

/// AclFactory.php:375-378 vs :873-885: an editor may not create a
/// vocabulary, only the administrator roles may.
#[test]
fn permissions_vocabulary_crud_matches_upstream_rules() {
    let vocabulary = Context::entity(SubjectKind::Vocabulary);
    assert!(!allows(&editor(), Action::Create, &vocabulary));
    assert!(allows(&site_admin(), Action::Create, &vocabulary));
    assert!(allows(&global_admin(), Action::Create, &vocabulary));
}

/// The roles upstream registers (`AclFactory::addRoles`) and the labels from
/// `Acl::$roleLabels`, including Reviewer, which DESIGN.md's list omits.
#[test]
fn permissions_roles_match_upstream_acl_roles() {
    let roles = [
        Role::Researcher,
        Role::Author,
        Role::Reviewer,
        Role::Editor,
        Role::SiteAdmin,
        Role::GlobalAdmin,
    ];
    let mut sorted = roles;
    sorted.sort();
    assert_eq!(sorted, roles, "Role::Ord is the upstream privilege order");
    assert_eq!(Role::GlobalAdmin.label(), "Global Administrator");
    assert_eq!(Role::SiteAdmin.label(), "Supervisor");
    assert!(Role::SiteAdmin.is_admin());
    assert!(Role::GlobalAdmin.is_admin());
    assert!(!Role::Editor.is_admin());
    for role in roles {
        assert_eq!(Role::from_wire_name(role.wire_name()), Some(role));
    }
    assert_eq!(Role::from_wire_name("guest"), None);
    assert_eq!(SitePermissionRole::Admin.number(), 1);
    assert_eq!(SitePermissionRole::Editor.number(), 2);
    assert_eq!(SitePermissionRole::Viewer.number(), 3);
    assert_eq!(
        SitePermissionRole::from_wire_name("admin"),
        Some(SitePermissionRole::Admin)
    );
    assert_eq!(SitePermissionRole::from_wire_name("nope"), None);
}
