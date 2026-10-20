//! The admin bar a page shows: who is signed in, the way to the admin, the
//! editor for what the page shows when the caller may edit it, and the way
//! out, above the site header.

use crate::blocks::test_support::{TestResources, context, page, site};
use crate::theme::DefaultTheme;
use crate::theme::Theme;
use crate::view::{RenderContext, ResourceView, View, Viewer};
use omeka_core::ids::{ResourceId, SiteId, UserId};
use omeka_core::permissions::{Role, RoleSet, SitePermissionRole};
use omeka_core::resource::ResourceKind;
use omeka_core::site::PageLayout;

/// The `<nav id="user-bar">` element of a document, or `None`.
fn bar(document: &str) -> Option<&str> {
    let start = document.find("<nav id=\"user-bar\"")?;
    let end = start + document[start..].find("</nav>")? + "</nav>".len();
    Some(&document[start..end])
}

/// User 5 with a global role and, when given, a role on site 1.
fn viewer(global: Role, on_site: Option<SitePermissionRole>) -> Option<Viewer> {
    let mut roles = RoleSet::for_user(UserId::new(5), global);
    if let Some(role) = on_site {
        roles = roles.with_site(SiteId::new(1), role);
    }
    Some(Viewer {
        name: "Ada".to_string(),
        roles,
    })
}

/// The bar of one view rendered for one caller, empty when there is none.
fn bar_for(view: &View<'_>, ctx: &RenderContext<'_>) -> String {
    let document = DefaultTheme.render(view, ctx);
    bar(&document).unwrap_or_default().to_string()
}

/// A resource of one kind, owned by one user.
fn owned(kind: ResourceKind, id: u64, owner: u64) -> ResourceView {
    let mut resource = ResourceView::new(ResourceId::new(id), kind, "/s/main/x", "A thing");
    resource.owner = Some(UserId::new(owner));
    resource
}

#[test]
fn a_signed_in_viewer_sees_the_dashboard_and_a_logout_form_that_returns_here() {
    let site = site();
    let resources = TestResources::default();
    let page = page(PageLayout::Normal, serde_json::json!({}), Vec::new());
    let mut ctx = context(&site, &resources);
    ctx.page = Some(&page);
    ctx.current_path = "/s/main/page/welcome".to_string();
    ctx.viewer = Some(Viewer {
        name: "Ada <Admin>".to_string(),
        roles: RoleSet::for_user(UserId::new(5), Role::Researcher),
    });

    let document = DefaultTheme.render(&View::Page, &ctx);
    let bar = bar(&document).expect("a user bar");
    assert!(bar.contains("aria-label=\"Omeka User\""), "{bar}");
    assert!(
        bar.contains("<a href=\"/admin\" class=\"admin-dashboard\">Admin dashboard</a>"),
        "{bar}"
    );
    assert!(
        bar.contains("Signed in as <strong>Ada &lt;Admin&gt;</strong>"),
        "{bar}"
    );
    assert!(
        bar.contains(
            "<form method=\"post\" action=\"/logout?redirect=%2Fs%2Fmain%2Fpage%2Fwelcome\">\
             <button type=\"submit\" class=\"logout\">Logout</button></form>"
        ),
        "{bar}"
    );
    assert!(!bar.contains("href=\"/logout\""), "{bar}");
    let bar_at = document.find("id=\"user-bar\"").expect("the bar");
    let header_at = document.find("<header").expect("the header");
    assert!(bar_at < header_at, "the bar sits above the header");
}

#[test]
fn the_bar_follows_show_user_bar_and_its_login_link_returns_here() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.current_path = "/s/main/item/7".to_string();
    assert_eq!(bar(&DefaultTheme.render(&View::Index, &ctx)), None);

    ctx.settings.show_user_bar = Some(1);
    let document = DefaultTheme.render(&View::Index, &ctx);
    let bar = bar(&document).expect("a login bar");
    assert!(
        bar.contains(
            "<a href=\"/login?redirect=%2Fs%2Fmain%2Fitem%2F7\" class=\"login\">Log in</a>"
        ),
        "{bar}"
    );
    assert!(!bar.contains("/admin"), "{bar}");
    assert!(!bar.contains("/#/"), "{bar}");
}

#[test]
fn the_bar_speaks_the_site_locale() {
    let site = site();
    let resources = TestResources::default();
    let page = page(PageLayout::Normal, serde_json::json!({}), Vec::new());
    let mut ctx = context(&site, &resources);
    ctx.page = Some(&page);
    ctx.settings.locale = Some("es_ES".to_string());
    ctx.viewer = viewer(Role::GlobalAdmin, None);

    let bar = bar_for(&View::Page, &ctx);
    assert!(bar.contains(">Panel de administración</a>"), "{bar}");
    assert!(bar.contains(">Editar esta página</a>"), "{bar}");
    assert!(bar.contains(">Cerrar sesión</button>"), "{bar}");
}

#[test]
fn edit_this_page_shows_only_to_callers_who_may_edit_the_page() {
    let site = site();
    let resources = TestResources::default();
    let page = page(PageLayout::Normal, serde_json::json!({}), Vec::new());
    let mut ctx = context(&site, &resources);
    ctx.page = Some(&page);
    let link = "<a href=\"/#/sites/1/pages/2\" class=\"admin-edit\">Edit this page</a>";

    for (who, caller) in [
        ("a global admin", viewer(Role::GlobalAdmin, None)),
        (
            "a site editor",
            viewer(Role::Researcher, Some(SitePermissionRole::Editor)),
        ),
        (
            "a site admin",
            viewer(Role::Author, Some(SitePermissionRole::Admin)),
        ),
    ] {
        ctx.viewer = caller;
        let bar = bar_for(&View::Page, &ctx);
        assert!(bar.contains(link), "{who}: {bar}");
    }

    for (who, caller) in [
        (
            "a site viewer",
            viewer(Role::Researcher, Some(SitePermissionRole::Viewer)),
        ),
        (
            "a user with no role on the site",
            viewer(Role::Author, None),
        ),
    ] {
        ctx.viewer = caller;
        let bar = bar_for(&View::Page, &ctx);
        assert!(bar.contains("Admin dashboard"), "{who}: {bar}");
        assert!(!bar.contains("admin-edit"), "{who}: {bar}");
    }

    ctx.viewer = None;
    ctx.settings.show_user_bar = Some(1);
    let bar = bar_for(&View::Page, &ctx);
    assert!(bar.contains("Log in"), "anonymous: {bar}");
    assert!(!bar.contains("admin-edit"), "anonymous: {bar}");
}

#[test]
fn the_site_owner_may_edit_the_page_without_a_site_role() {
    let mut site = site();
    site.owner = Some(UserId::new(5));
    let resources = TestResources::default();
    let page = page(PageLayout::Normal, serde_json::json!({}), Vec::new());
    let mut ctx = context(&site, &resources);
    ctx.page = Some(&page);
    ctx.viewer = viewer(Role::Researcher, None);

    let bar = bar_for(&View::Page, &ctx);
    assert!(bar.contains(">Edit this page</a>"), "{bar}");
}

#[test]
fn a_resource_page_links_its_admin_page_for_callers_who_may_edit_it() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);

    // An author edits their own rows only (`OwnsEntityAssertion`).
    ctx.viewer = viewer(Role::Author, None);
    let mine = owned(ResourceKind::Item, 7, 5);
    let bar = bar_for(&View::Item(&mine), &ctx);
    assert!(
        bar.contains("<a href=\"/#/items/7\" class=\"admin-edit\">Edit item</a>"),
        "{bar}"
    );
    let theirs = owned(ResourceKind::Item, 8, 6);
    let bar = bar_for(&View::Item(&theirs), &ctx);
    assert!(!bar.contains("admin-edit"), "{bar}");

    // An editor edits any row.
    ctx.viewer = viewer(Role::Editor, None);
    let set = owned(ResourceKind::ItemSet, 9, 6);
    let bar = bar_for(&View::ItemSet(&set), &ctx);
    assert!(
        bar.contains("<a href=\"/#/item-sets/9\" class=\"admin-edit\">Edit item set</a>"),
        "{bar}"
    );
    let media = owned(ResourceKind::Media, 10, 6);
    let bar = bar_for(&View::Media(&media), &ctx);
    assert!(
        bar.contains("<a href=\"/#/media/10\" class=\"admin-edit\">Edit media</a>"),
        "{bar}"
    );

    // A researcher edits no resource, even one they own.
    ctx.viewer = viewer(Role::Researcher, Some(SitePermissionRole::Admin));
    let bar = bar_for(&View::Item(&mine), &ctx);
    assert!(!bar.contains("admin-edit"), "{bar}");
}

#[test]
fn a_view_with_nothing_to_edit_offers_no_edit_link() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.viewer = viewer(Role::GlobalAdmin, None);
    let bar = bar_for(&View::Index, &ctx);
    assert!(bar.contains("Admin dashboard"), "{bar}");
    assert!(!bar.contains("admin-edit"), "{bar}");
}
