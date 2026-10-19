//! The admin bar a page shows: who is signed in, the way to the admin and the
//! way out, above the site header.

use crate::blocks::test_support::{TestResources, context, page, site};
use crate::theme::DefaultTheme;
use crate::theme::Theme;
use crate::view::View;
use omeka_core::site::PageLayout;

/// The `<nav id="user-bar">` element of a document, or `None`.
fn bar(document: &str) -> Option<&str> {
    let start = document.find("<nav id=\"user-bar\"")?;
    let end = start + document[start..].find("</nav>")? + "</nav>".len();
    Some(&document[start..end])
}

#[test]
fn a_signed_in_viewer_sees_the_bar_with_the_dashboard_and_logout_links() {
    let site = site();
    let resources = TestResources::default();
    let page = page(PageLayout::Normal, serde_json::json!({}), Vec::new());
    let mut ctx = context(&site, &resources);
    ctx.page = Some(&page);
    ctx.signed_in = true;
    ctx.user_name = Some("Ada <Admin>".to_string());

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
        bar.contains("<a href=\"/logout\" class=\"logout\">Logout</a>"),
        "{bar}"
    );
    let bar_at = document.find("id=\"user-bar\"").expect("the bar");
    let header_at = document.find("<header").expect("the header");
    assert!(bar_at < header_at, "the bar sits above the header");
}

#[test]
fn the_bar_follows_show_user_bar_for_an_anonymous_viewer() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    assert_eq!(bar(&DefaultTheme.render(&View::Index, &ctx)), None);

    ctx.settings.show_user_bar = Some(1);
    let document = DefaultTheme.render(&View::Index, &ctx);
    let bar = bar(&document).expect("a login bar");
    assert!(
        bar.contains("<a href=\"/login\" class=\"login\">Log in</a>"),
        "{bar}"
    );
    assert!(!bar.contains("/admin"), "{bar}");
}

#[test]
fn the_bar_speaks_the_site_locale() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.settings.locale = Some("es_ES".to_string());
    ctx.signed_in = true;
    ctx.user_name = Some("Ada".to_string());

    let document = DefaultTheme.render(&View::Index, &ctx);
    let bar = bar(&document).expect("a user bar");
    assert!(bar.contains(">Panel de administración</a>"), "{bar}");
    assert!(bar.contains(">Cerrar sesión</a>"), "{bar}");
}
