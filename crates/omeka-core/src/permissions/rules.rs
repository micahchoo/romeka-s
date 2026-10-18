/// Whether a matching rule allows or denies.
use super::{Action, Adapter, Context, Controller, Role, RoleSet, SitePermissionRole, SubjectKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Allow,
    Deny,
}

/// The action set a rule covers.
#[derive(Debug, Clone, Copy)]
enum Actions {
    Any,
    One(Action),
    Some(&'static [Action]),
}

impl Actions {
    fn contains(self, action: Action) -> bool {
        match self {
            Self::Any => true,
            Self::One(one) => one == action,
            Self::Some(list) => list.contains(&action),
        }
    }
}

/// The upstream resources a rule was added to. The families never mix: a rule
/// on `Omeka\Entity\Item` cannot answer a check on `Omeka\Api\Adapter\ItemAdapter`.
#[derive(Debug, Clone, Copy)]
enum Targets {
    /// No resource constraint: `$acl->allow('site_admin')`.
    Any,
    /// `Omeka\Entity\*`, including the bound variants of [`Context`].
    Entities(&'static [SubjectKind]),
    /// `Omeka\Api\Adapter\*`.
    Adapters(&'static [Adapter]),
    /// `Omeka\Controller\*`.
    Controllers(&'static [Controller]),
    /// `Omeka\Module\Manager`.
    Module,
}

impl Targets {
    fn accepts(self, context: &Context) -> bool {
        match (self, context) {
            (Self::Any, _) => true,
            (Self::Entities(list), _) => context
                .entity_kind()
                .is_some_and(|kind| list.contains(&kind)),
            (Self::Adapters(list), Context::Adapter(adapter)) => list.contains(adapter),
            (Self::Controllers(list), Context::Controller(controller)) => list.contains(controller),
            (Self::Module, Context::Module) => true,
            _ => false,
        }
    }
}

/// The assertions from `application/src/Permissions/Assertion/*`, pre-composed
/// exactly as `AclFactory::aggregate` composes them. `Any` is
/// `AssertionAggregate::MODE_AT_LEAST_ONE`.
#[derive(Debug, Clone, Copy)]
enum Assertion {
    /// No assertion: the rule applies to any resource.
    None,
    /// `OwnsEntityAssertion`.
    Owns,
    /// `AssertionNegation(OwnsEntityAssertion)`.
    NotOwns,
    /// `Any[SiteIsPublicAssertion, OwnsEntityAssertion, HasSitePermissionAssertion('viewer')]`.
    SitePublicOrOwnsOrViewer,
    /// `Any[SitePageIsPublicAssertion, OwnsEntityAssertion, HasSitePermissionAssertion('viewer')]`.
    PageAndSitePublicOrOwnsOrViewer,
    /// `Any[OwnsEntityAssertion, HasSitePermissionAssertion('admin')]`.
    OwnsOrSiteAdmin,
    /// `Any[OwnsEntityAssertion, HasSitePermissionAssertion('editor')]`.
    OwnsOrSiteEditor,
    /// `Any[OwnsEntityAssertion, HasSitePermissionAssertion('admin'), HasSitePermissionAssertion('editor')]`.
    OwnsOrSiteAdminOrSiteEditor,
    /// `IsSelfAssertion`: the role object is the resource.
    IsSelf,
    /// `All[UserIsAdminAssertion, AssertionNegation(IsSelfAssertion)]`: the
    /// target user has an admin role and is not the actor.
    TargetIsAdminAndNotSelf,
}

/// One ACL rule.
#[derive(Debug, Clone, Copy)]
struct Rule {
    /// `None` is upstream's `null` role: every actor, authenticated or not.
    role: Option<Role>,
    actions: Actions,
    targets: Targets,
    assertion: Assertion,
    decision: Decision,
}

impl Rule {
    const fn new(
        role: Option<Role>,
        actions: Actions,
        targets: Targets,
        assertion: Assertion,
        decision: Decision,
    ) -> Self {
        Self {
            role,
            actions,
            targets,
            assertion,
            decision,
        }
    }
}

use Actions::{Any as AnyAction, One as OneAction, Some as SomeActions};
use Assertion::{
    IsSelf, None as NoAssertion, NotOwns, Owns, OwnsOrSiteAdmin, OwnsOrSiteAdminOrSiteEditor,
    OwnsOrSiteEditor, PageAndSitePublicOrOwnsOrViewer, SitePublicOrOwnsOrViewer,
    TargetIsAdminAndNotSelf,
};
use Decision::{Allow, Deny};
use Targets::{Adapters, Any as AnyTarget, Controllers, Entities, Module as ModuleTarget};

const ROLE_RESEARCHER: Option<Role> = Some(Role::Researcher);
const ROLE_AUTHOR: Option<Role> = Some(Role::Author);
const ROLE_REVIEWER: Option<Role> = Some(Role::Reviewer);
const ROLE_EDITOR: Option<Role> = Some(Role::Editor);
const ROLE_SITE_ADMIN: Option<Role> = Some(Role::SiteAdmin);
const ROLE_GLOBAL_ADMIN: Option<Role> = Some(Role::GlobalAdmin);

/// The controllers every actor reaches, authenticated or not
/// (`AclFactory.php:146-158`).
const PUBLIC_CONTROLLERS: &[Controller] = &[
    Controller::Api,
    Controller::ApiLocal,
    Controller::Index,
    Controller::Login,
    Controller::Maintenance,
    Controller::Migrate,
    Controller::Search,
    Controller::IiifViewer,
    Controller::SiteAdminIndex,
    Controller::SiteAdminPage,
    Controller::SiteIndex,
    Controller::SiteItem,
    Controller::SiteItemSet,
    Controller::SiteMedia,
    Controller::SitePage,
    Controller::SiteCrossSiteSearch,
];

/// The adapters whose `search` and `read` are public
/// (`AclFactory.php:271-287`).
const PUBLIC_ADAPTERS: &[Adapter] = &[
    Adapter::Resource,
    Adapter::ItemSet,
    Adapter::Item,
    Adapter::Media,
    Adapter::Vocabulary,
    Adapter::ResourceClass,
    Adapter::Property,
    Adapter::ResourceTemplate,
    Adapter::Asset,
    Adapter::ApiResource,
    Adapter::DataType,
];

/// Entities whose `read` is public (`AclFactory.php:291-302`).
const PUBLIC_READ_ENTITIES: &[SubjectKind] = &[
    SubjectKind::ItemSet,
    SubjectKind::Item,
    SubjectKind::Media,
    SubjectKind::Vocabulary,
    SubjectKind::ResourceClass,
    SubjectKind::Property,
    SubjectKind::ResourceTemplate,
    SubjectKind::Asset,
];

/// The entities that make up a resource in the common sense: the five nameable
/// ones plus the value annotation (`AclFactory.php:524-536, 702-714, 873-885`).
const OWNED_RESOURCES: &[SubjectKind] = &[
    SubjectKind::Item,
    SubjectKind::ItemSet,
    SubjectKind::Media,
    SubjectKind::ResourceTemplate,
    SubjectKind::Asset,
];

/// Resource controllers: item, item set and media (sidebar lists and batches).
const RESOURCE_CONTROLLERS: &[Controller] = &[
    Controller::AdminItem,
    Controller::AdminItemSet,
    Controller::AdminMedia,
];

/// Item and item set controllers (researcher's sidebar).
const ITEM_CONTROLLERS: &[Controller] = &[Controller::AdminItem, Controller::AdminItemSet];

/// The `read` controllers a researcher and up may browse and show
/// (`AclFactory.php:320-336, 408-424, 591-607, 767-783`).
const READ_ADMIN_CONTROLLERS: &[Controller] = &[
    Controller::AdminItem,
    Controller::AdminItemSet,
    Controller::AdminMedia,
    Controller::AdminResourceTemplate,
    Controller::AdminVocabulary,
    Controller::AdminResourceClass,
    Controller::AdminProperty,
    Controller::AdminSystemInfo,
];

/// The controllers an author, reviewer and editor may add, edit and delete
/// through (`AclFactory.php:469-483, 647-660, 828-842`).
const ADD_EDIT_CONTROLLERS: &[Controller] = &[
    Controller::AdminItem,
    Controller::AdminItemSet,
    Controller::AdminMedia,
    Controller::AdminResourceTemplate,
    Controller::AdminAsset,
];

/// The reviewer's version of the above: no resource template.
const ADD_EDIT_CONTROLLERS_REVIEWER: &[Controller] = &[
    Controller::AdminItem,
    Controller::AdminItemSet,
    Controller::AdminMedia,
    Controller::AdminAsset,
];

/// The adapters an author and an editor may create, update and delete through
/// (`AclFactory.php:497-510, 858-871`).
const CRUD_ADAPTERS: &[Adapter] = &[
    Adapter::Item,
    Adapter::ItemSet,
    Adapter::Media,
    Adapter::ResourceTemplate,
    Adapter::Asset,
];

/// The reviewer's adapters: no resource template (`AclFactory.php:675-687`).
const CRUD_ADAPTERS_REVIEWER: &[Adapter] = &[
    Adapter::Item,
    Adapter::ItemSet,
    Adapter::Media,
    Adapter::Asset,
];

/// The item, item set and media adapters (batch operations).
const RESOURCE_ADAPTERS: &[Adapter] = &[Adapter::Item, Adapter::ItemSet, Adapter::Media];

/// Entities an author may create (`AclFactory.php:524-536`).
const AUTHOR_CREATE_ENTITIES: &[SubjectKind] = &[
    SubjectKind::Item,
    SubjectKind::ItemSet,
    SubjectKind::Media,
    SubjectKind::ResourceTemplate,
    SubjectKind::Asset,
    SubjectKind::ValueAnnotation,
];

/// Entities a reviewer may create and update (`AclFactory.php:702-714`).
const REVIEWER_CREATE_ENTITIES: &[SubjectKind] = &[
    SubjectKind::Item,
    SubjectKind::ItemSet,
    SubjectKind::Media,
    SubjectKind::Asset,
    SubjectKind::ValueAnnotation,
];

/// Entities a reviewer may delete only when owned (`AclFactory.php:716-727`).
const REVIEWER_OWNED_ENTITIES: &[SubjectKind] = &[
    SubjectKind::Item,
    SubjectKind::ItemSet,
    SubjectKind::Media,
    SubjectKind::Asset,
];

/// Entities an editor may create, update and delete (`AclFactory.php:873-885`).
const EDITOR_CRUD_ENTITIES: &[SubjectKind] = &[
    SubjectKind::Item,
    SubjectKind::ItemSet,
    SubjectKind::Media,
    SubjectKind::ResourceTemplate,
    SubjectKind::Asset,
    SubjectKind::ValueAnnotation,
];

const SITE: &[SubjectKind] = &[SubjectKind::Site];
const SITE_PAGE: &[SubjectKind] = &[SubjectKind::SitePage];
const VOCABULARY: &[SubjectKind] = &[SubjectKind::Vocabulary];
const USER: &[SubjectKind] = &[SubjectKind::User];
const RESOURCE: &[SubjectKind] = &[SubjectKind::Resource];
const SITE_ADAPTERS: &[Adapter] = &[Adapter::Site, Adapter::SitePage];
const USER_ADAPTER: &[Adapter] = &[Adapter::User];
const MODULE_CONTROLLERS: &[Controller] = &[Controller::AdminModule];
const SETTING_CONTROLLERS: &[Controller] = &[Controller::AdminSetting];
const RESOURCE_TEMPLATE_CONTROLLERS: &[Controller] = &[Controller::AdminResourceTemplate];
const USER_CONTROLLERS: &[Controller] = &[Controller::AdminUser];
const QUERY_CONTROLLERS: &[Controller] = &[Controller::AdminQuery];
const COLUMN_CONTROLLERS: &[Controller] = &[Controller::AdminColumns];
const ASSET_CONTROLLERS: &[Controller] = &[Controller::AdminAsset];
const VOCABULARY_CONTROLLERS: &[Controller] = &[Controller::AdminVocabulary];

/// Read the controller lists, item sidebars and search. `index` is part of the
/// common read list (`AclFactory.php:320-338` and repeats).
const READ_ADMIN_ACTIONS: &[Action] = &[
    Action::AdminIndex,
    Action::AdminBrowse,
    Action::AdminShow,
    Action::AdminShowDetails,
];
const SEARCH_ACTIONS: &[Action] = &[Action::AdminSidebarSelect, Action::AdminSearch];
const QUERY_ACTIONS: &[Action] = &[
    Action::AdminSidebarEdit,
    Action::AdminSidebarPreview,
    Action::AdminSearchFilters,
];
const COLUMN_ACTIONS: &[Action] = &[
    Action::AdminColumnList,
    Action::AdminColumnRow,
    Action::AdminColumnEditSidebar,
];
const ASSET_ADMIN_ACTIONS: &[Action] = &[
    Action::AdminSidebarSelect,
    Action::AdminBrowse,
    Action::AdminShowDetails,
];
const CLASS_ACTIONS: &[Action] = &[Action::AdminClasses, Action::AdminProperties];
const ADD_EDIT_ACTIONS: &[Action] = &[
    Action::AdminAdd,
    Action::AdminEdit,
    Action::AdminDelete,
    Action::AdminDeleteConfirm,
];
const BATCH_ACTIONS: &[Action] = &[Action::AdminBatchEdit, Action::AdminBatchDelete];
const BATCH_ALL_ACTIONS: &[Action] = &[
    Action::AdminBatchEdit,
    Action::AdminBatchEditAll,
    Action::AdminBatchDelete,
    Action::AdminBatchDeleteAll,
];
const CRUD_ADAPTER_ACTIONS: &[Action] = &[Action::Create, Action::Update, Action::Delete];
const BATCH_ADAPTER_ACTIONS: &[Action] = &[Action::BatchUpdate, Action::BatchDelete];
const BATCH_ADAPTER_ACTIONS_REVIEWER: &[Action] = &[
    Action::BatchUpdate,
    Action::BatchUpdateAll,
    Action::BatchDelete,
];
const BATCH_ADAPTER_ACTIONS_EDITOR: &[Action] = &[
    Action::BatchUpdate,
    Action::BatchUpdateAll,
    Action::BatchDelete,
    Action::BatchDeleteAll,
];
const USER_SELF_ACTIONS: &[Action] = &[Action::Update, Action::ChangePassword, Action::EditKeys];
const USER_ADAPTER_ACTIONS: &[Action] = &[Action::Read, Action::Update, Action::Search];
const MODULE_ACTIONS: &[Action] = &[
    Action::ModuleActivate,
    Action::ModuleDeactivate,
    Action::ModuleInstall,
    Action::ModuleUninstall,
    Action::ModuleUpgrade,
    Action::ModuleConfigure,
];

/// Every rule, in the order `AclFactory::addRules` adds them (`:124-137`). The
/// last matching rule wins, which is how upstream's Laminas ACL evaluates rules.
static RULES: &[Rule] = &[
    // addRulesForSites (AclFactory.php:144-249).
    Rule::new(
        None,
        AnyAction,
        Controllers(PUBLIC_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :146
    Rule::new(None, AnyAction, Adapters(SITE_ADAPTERS), NoAssertion, Allow), // :159
    Rule::new(
        ROLE_EDITOR,
        OneAction(Action::Create),
        Entities(SITE),
        NoAssertion,
        Allow,
    ), // :167
    Rule::new(
        None,
        OneAction(Action::Create),
        Entities(SITE_PAGE),
        NoAssertion,
        Allow,
    ), // :173
    Rule::new(None, OneAction(Action::Delete), Entities(SITE), Owns, Allow), // :179
    Rule::new(
        None,
        OneAction(Action::Update),
        Entities(SITE),
        OwnsOrSiteAdmin,
        Allow,
    ), // :190
    Rule::new(
        None,
        OneAction(Action::AddPage),
        Entities(SITE),
        OwnsOrSiteEditor,
        Allow,
    ), // :201
    Rule::new(
        None,
        SomeActions(&[Action::Update, Action::Delete]),
        Entities(SITE_PAGE),
        OwnsOrSiteEditor,
        Allow,
    ), // :207
    Rule::new(
        None,
        OneAction(Action::Read),
        Entities(SITE),
        SitePublicOrOwnsOrViewer,
        Allow,
    ), // :219
    Rule::new(
        None,
        OneAction(Action::Read),
        Entities(SITE_PAGE),
        PageAndSitePublicOrOwnsOrViewer,
        Allow,
    ), // :231
    Rule::new(
        None,
        OneAction(Action::CanAssignItems),
        Entities(SITE),
        OwnsOrSiteAdminOrSiteEditor,
        Allow,
    ), // :243
    // addRulesForAllRoles (AclFactory.php:256-307).
    Rule::new(
        None,
        SomeActions(&[Action::Search, Action::Read]),
        Adapters(PUBLIC_ADAPTERS),
        NoAssertion,
        Allow,
    ), // :271
    Rule::new(
        None,
        OneAction(Action::Read),
        Entities(PUBLIC_READ_ENTITIES),
        NoAssertion,
        Allow,
    ), // :291
    // addRulesForResearcher (AclFactory.php:314-395).
    Rule::new(
        ROLE_RESEARCHER,
        AnyAction,
        Controllers(&[Controller::AdminIndex]),
        NoAssertion,
        Allow,
    ), // :316
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(READ_ADMIN_ACTIONS),
        Controllers(READ_ADMIN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :320
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(QUERY_ACTIONS),
        Controllers(QUERY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :339
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(COLUMN_ACTIONS),
        Controllers(COLUMN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :348
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(SEARCH_ACTIONS),
        Controllers(ITEM_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :357
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(ASSET_ADMIN_ACTIONS),
        Controllers(ASSET_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :365
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(CLASS_ACTIONS),
        Controllers(VOCABULARY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :370
    Rule::new(
        ROLE_RESEARCHER,
        AnyAction,
        Controllers(USER_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :375
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(USER_ADAPTER_ACTIONS),
        Adapters(USER_ADAPTER),
        NoAssertion,
        Allow,
    ), // :379
    Rule::new(
        ROLE_RESEARCHER,
        OneAction(Action::Read),
        Entities(USER),
        NoAssertion,
        Allow,
    ), // :384
    Rule::new(
        ROLE_RESEARCHER,
        SomeActions(USER_SELF_ACTIONS),
        Entities(USER),
        IsSelf,
        Allow,
    ), // :389
    // addRulesForAuthor (AclFactory.php:402-578).
    Rule::new(
        ROLE_AUTHOR,
        AnyAction,
        Controllers(&[Controller::AdminIndex]),
        NoAssertion,
        Allow,
    ), // :404
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(READ_ADMIN_ACTIONS),
        Controllers(READ_ADMIN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :408
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(QUERY_ACTIONS),
        Controllers(QUERY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :427
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(COLUMN_ACTIONS),
        Controllers(COLUMN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :436
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(&[Action::AdminAddNewPropertyRow, Action::AdminImport]),
        Controllers(RESOURCE_TEMPLATE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :445
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(SEARCH_ACTIONS),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :450
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(ASSET_ADMIN_ACTIONS),
        Controllers(ASSET_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :459
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(CLASS_ACTIONS),
        Controllers(VOCABULARY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :464
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(ADD_EDIT_ACTIONS),
        Controllers(ADD_EDIT_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :469
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(BATCH_ACTIONS),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :485
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(CRUD_ADAPTER_ACTIONS),
        Adapters(CRUD_ADAPTERS),
        NoAssertion,
        Allow,
    ), // :497
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(BATCH_ADAPTER_ACTIONS),
        Adapters(RESOURCE_ADAPTERS),
        NoAssertion,
        Allow,
    ), // :512
    Rule::new(
        ROLE_AUTHOR,
        OneAction(Action::Create),
        Entities(AUTHOR_CREATE_ENTITIES),
        NoAssertion,
        Allow,
    ), // :524
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(&[Action::Update, Action::Delete]),
        Entities(OWNED_RESOURCES),
        Owns,
        Allow,
    ), // :538
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(&[Action::Update, Action::Delete]),
        Entities(&[SubjectKind::ValueAnnotation]),
        NoAssertion,
        Allow,
    ), // :553
    Rule::new(
        ROLE_AUTHOR,
        AnyAction,
        Controllers(USER_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :558
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(USER_ADAPTER_ACTIONS),
        Adapters(USER_ADAPTER),
        NoAssertion,
        Allow,
    ), // :562
    Rule::new(
        ROLE_AUTHOR,
        OneAction(Action::Read),
        Entities(USER),
        NoAssertion,
        Allow,
    ), // :567
    Rule::new(
        ROLE_AUTHOR,
        SomeActions(USER_SELF_ACTIONS),
        Entities(USER),
        IsSelf,
        Allow,
    ), // :572
    // addRulesForReviewer (AclFactory.php:585-754).
    Rule::new(
        ROLE_REVIEWER,
        AnyAction,
        Controllers(&[Controller::AdminIndex]),
        NoAssertion,
        Allow,
    ), // :587
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(READ_ADMIN_ACTIONS),
        Controllers(READ_ADMIN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :591
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(QUERY_ACTIONS),
        Controllers(QUERY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :610
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(COLUMN_ACTIONS),
        Controllers(COLUMN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :619
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(SEARCH_ACTIONS),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :628
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(ASSET_ADMIN_ACTIONS),
        Controllers(ASSET_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :637
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(CLASS_ACTIONS),
        Controllers(VOCABULARY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :642
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(ADD_EDIT_ACTIONS),
        Controllers(ADD_EDIT_CONTROLLERS_REVIEWER),
        NoAssertion,
        Allow,
    ), // :647
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(&[
            Action::AdminBatchEdit,
            Action::AdminBatchEditAll,
            Action::AdminBatchDelete,
        ]),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :662
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(CRUD_ADAPTER_ACTIONS),
        Adapters(CRUD_ADAPTERS_REVIEWER),
        NoAssertion,
        Allow,
    ), // :675
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(BATCH_ADAPTER_ACTIONS_REVIEWER),
        Adapters(RESOURCE_ADAPTERS),
        NoAssertion,
        Allow,
    ), // :689
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(&[Action::Create, Action::Update]),
        Entities(REVIEWER_CREATE_ENTITIES),
        NoAssertion,
        Allow,
    ), // :702
    Rule::new(
        ROLE_REVIEWER,
        OneAction(Action::Delete),
        Entities(REVIEWER_OWNED_ENTITIES),
        Owns,
        Allow,
    ), // :716
    Rule::new(
        ROLE_REVIEWER,
        OneAction(Action::ViewAll),
        Entities(RESOURCE),
        NoAssertion,
        Allow,
    ), // :729
    Rule::new(
        ROLE_REVIEWER,
        AnyAction,
        Controllers(USER_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :734
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(USER_ADAPTER_ACTIONS),
        Adapters(USER_ADAPTER),
        NoAssertion,
        Allow,
    ), // :738
    Rule::new(
        ROLE_REVIEWER,
        OneAction(Action::Read),
        Entities(USER),
        NoAssertion,
        Allow,
    ), // :743
    Rule::new(
        ROLE_REVIEWER,
        SomeActions(USER_SELF_ACTIONS),
        Entities(USER),
        IsSelf,
        Allow,
    ), // :748
    // addRulesForEditor (AclFactory.php:761-940).
    Rule::new(
        ROLE_EDITOR,
        AnyAction,
        Controllers(&[Controller::AdminIndex]),
        NoAssertion,
        Allow,
    ), // :763
    Rule::new(
        ROLE_EDITOR,
        SomeActions(READ_ADMIN_ACTIONS),
        Controllers(READ_ADMIN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :767
    Rule::new(
        ROLE_EDITOR,
        SomeActions(QUERY_ACTIONS),
        Controllers(QUERY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :786
    Rule::new(
        ROLE_EDITOR,
        SomeActions(COLUMN_ACTIONS),
        Controllers(COLUMN_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :795
    Rule::new(
        ROLE_EDITOR,
        SomeActions(&[Action::AdminAddNewPropertyRow, Action::AdminImport]),
        Controllers(RESOURCE_TEMPLATE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :804
    Rule::new(
        ROLE_EDITOR,
        SomeActions(SEARCH_ACTIONS),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :809
    Rule::new(
        ROLE_EDITOR,
        SomeActions(ASSET_ADMIN_ACTIONS),
        Controllers(ASSET_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :818
    Rule::new(
        ROLE_EDITOR,
        SomeActions(CLASS_ACTIONS),
        Controllers(VOCABULARY_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :823
    Rule::new(
        ROLE_EDITOR,
        SomeActions(ADD_EDIT_ACTIONS),
        Controllers(ADD_EDIT_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :828
    Rule::new(
        ROLE_EDITOR,
        SomeActions(BATCH_ALL_ACTIONS),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :844
    Rule::new(
        ROLE_EDITOR,
        SomeActions(CRUD_ADAPTER_ACTIONS),
        Adapters(CRUD_ADAPTERS),
        NoAssertion,
        Allow,
    ), // :858
    Rule::new(
        ROLE_EDITOR,
        SomeActions(CRUD_ADAPTER_ACTIONS),
        Entities(EDITOR_CRUD_ENTITIES),
        NoAssertion,
        Allow,
    ), // :873
    Rule::new(
        ROLE_EDITOR,
        SomeActions(BATCH_ADAPTER_ACTIONS_EDITOR),
        Adapters(RESOURCE_ADAPTERS),
        NoAssertion,
        Allow,
    ), // :889
    Rule::new(
        ROLE_EDITOR,
        SomeActions(BATCH_ACTIONS),
        Controllers(RESOURCE_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :903 (repeats :844)
    Rule::new(
        ROLE_EDITOR,
        OneAction(Action::ViewAll),
        Entities(RESOURCE),
        NoAssertion,
        Allow,
    ), // :915
    Rule::new(
        ROLE_EDITOR,
        AnyAction,
        Controllers(USER_CONTROLLERS),
        NoAssertion,
        Allow,
    ), // :920
    Rule::new(
        ROLE_EDITOR,
        SomeActions(USER_ADAPTER_ACTIONS),
        Adapters(USER_ADAPTER),
        NoAssertion,
        Allow,
    ), // :924
    Rule::new(
        ROLE_EDITOR,
        OneAction(Action::Read),
        Entities(USER),
        NoAssertion,
        Allow,
    ), // :929
    Rule::new(
        ROLE_EDITOR,
        SomeActions(USER_SELF_ACTIONS),
        Entities(USER),
        IsSelf,
        Allow,
    ), // :934
    // addRulesForSiteAdmin (AclFactory.php:947-991).
    Rule::new(ROLE_SITE_ADMIN, AnyAction, AnyTarget, NoAssertion, Allow), // :949
    Rule::new(
        ROLE_SITE_ADMIN,
        SomeActions(MODULE_ACTIONS),
        ModuleTarget,
        NoAssertion,
        Deny,
    ), // :950
    Rule::new(
        ROLE_SITE_ADMIN,
        SomeActions(MODULE_ACTIONS),
        Controllers(MODULE_CONTROLLERS),
        NoAssertion,
        Deny,
    ), // :950
    Rule::new(
        ROLE_SITE_ADMIN,
        AnyAction,
        Controllers(SETTING_CONTROLLERS),
        NoAssertion,
        Deny,
    ), // :955
    Rule::new(
        ROLE_SITE_ADMIN,
        SomeActions(&[Action::Update, Action::Delete]),
        Entities(VOCABULARY),
        NotOwns,
        Deny,
    ), // :959
    Rule::new(
        ROLE_SITE_ADMIN,
        OneAction(Action::ChangeRoleAdmin),
        Entities(USER),
        NoAssertion,
        Deny,
    ), // :966
    Rule::new(
        ROLE_SITE_ADMIN,
        SomeActions(&[Action::ChangeRole, Action::ActivateUser, Action::Delete]),
        Entities(USER),
        IsSelf,
        Deny,
    ), // :971
    Rule::new(
        ROLE_SITE_ADMIN,
        SomeActions(&[
            Action::Update,
            Action::Delete,
            Action::ChangePassword,
            Action::EditKeys,
        ]),
        Entities(USER),
        TargetIsAdminAndNotSelf,
        Deny,
    ), // :985
    // addRulesForGlobalAdmin (AclFactory.php:998-1007).
    Rule::new(ROLE_GLOBAL_ADMIN, AnyAction, AnyTarget, NoAssertion, Allow), // :1000
    Rule::new(
        ROLE_GLOBAL_ADMIN,
        SomeActions(&[Action::ChangeRole, Action::ActivateUser, Action::Delete]),
        Entities(USER),
        IsSelf,
        Deny,
    ), // :1001
];

/// Whether the actor may perform the action on the context.
///
/// Evaluate this the way upstream does: a bound entity check passes the row's
/// context (`Context::item(Some(owner))`), an API operation passes the adapter
/// (`Context::adapter(Adapter::Item)`), a controller action passes the
/// controller. An API write is allowed when both the adapter check and the
/// bound entity check pass (`AbstractEntityRepresentation::userIsAllowed`).
/// The result is the last matching rule's verdict, matching Laminas ACL.
#[must_use]
pub fn allows(actor: &RoleSet, action: Action, context: &Context) -> bool {
    let mut decision = false;
    for rule in RULES {
        if rule.role.is_some() && rule.role != actor.global {
            continue;
        }
        if !rule.actions.contains(action) {
            continue;
        }
        if !rule.targets.accepts(context) {
            continue;
        }
        if !assertion_holds(rule.assertion, actor, context) {
            continue;
        }
        decision = matches!(rule.decision, Allow);
    }
    decision
}

fn assertion_holds(assertion: Assertion, actor: &RoleSet, context: &Context) -> bool {
    match assertion {
        NoAssertion => true,
        Owns => owns(actor, context),
        NotOwns => !owns(actor, context),
        SitePublicOrOwnsOrViewer => {
            site_is_public(context)
                || owns(actor, context)
                || site_permission_at_least(actor, context, SitePermissionRole::Viewer)
        }
        PageAndSitePublicOrOwnsOrViewer => {
            (site_is_public(context) && page_is_public(context))
                || owns(actor, context)
                || site_permission_at_least(actor, context, SitePermissionRole::Viewer)
        }
        OwnsOrSiteAdmin => {
            owns(actor, context)
                || site_permission_at_least(actor, context, SitePermissionRole::Admin)
        }
        OwnsOrSiteEditor => {
            owns(actor, context)
                || site_permission_at_least(actor, context, SitePermissionRole::Editor)
        }
        OwnsOrSiteAdminOrSiteEditor => {
            owns(actor, context)
                || site_permission_at_least(actor, context, SitePermissionRole::Admin)
                || site_permission_at_least(actor, context, SitePermissionRole::Editor)
        }
        IsSelf => match context {
            Context::User { id, .. } => actor.actor() == Some(*id),
            _ => false,
        },
        TargetIsAdminAndNotSelf => match context {
            Context::User { id, role } => role.is_admin() && actor.actor() != Some(*id),
            _ => false,
        },
    }
}

/// `OwnsEntityAssertion`: an authenticated actor whose id equals the resource's
/// owner id (`application/src/Permissions/Assertion/OwnsEntityAssertion.php`).
fn owns(actor: &RoleSet, context: &Context) -> bool {
    match (actor.actor(), context.owner()) {
        (Some(actor), Some(owner)) => actor == owner,
        _ => false,
    }
}

fn site_is_public(context: &Context) -> bool {
    match context {
        Context::Site { is_public, .. } => *is_public,
        Context::SitePage { site_is_public, .. } => *site_is_public,
        _ => false,
    }
}

fn page_is_public(context: &Context) -> bool {
    match context {
        Context::SitePage { is_public, .. } => *is_public,
        _ => false,
    }
}

/// `HasSitePermissionAssertion`: the actor must be authenticated (unauthed
/// users can't have site permissions) and hold a site role at least as strong as
/// the required one (role numbers ascend from admin).
fn site_permission_at_least(
    actor: &RoleSet,
    context: &Context,
    required: SitePermissionRole,
) -> bool {
    if actor.actor().is_none() {
        return false;
    }
    let Some(site) = context.site() else {
        return false;
    };
    let Some(held) = actor.site_permission(site) else {
        return false;
    };
    held.number() <= required.number()
}
