//! The admin bar across the top of the public site (`common/user-bar.phtml`,
//! `View/Helper/UserBar.php`).

use omeka_core::permissions::{Action, Adapter, Context, RoleSet, allows};
use omeka_core::resource::ResourceKind;

use crate::html::{escape_html, urlencode};
use crate::i18n::{Messages, Translator, sprintf1};
use crate::view::{RenderContext, ResourceView, View};

/// `common/user-bar.phtml`: the admin bar a signed-in caller sees, or a login
/// link when `show_user_bar` is `1` and nobody is signed in.
///
/// Upstream's `site-links` name the installation, the site and the current
/// resource, with an edit link the caller's permissions gate. The port links
/// the dashboard and, when the caller may edit what the view shows, that
/// thing's admin editor.
///
/// Logout is a form, because ending a session is a write: a `POST /logout`
/// that the server's same-origin check guards. Both Logout and Log in carry
/// the page's path as `redirect`, so the reader comes back to it.
pub(crate) fn render(view: &View<'_>, ctx: &RenderContext<'_>, tr: Translator) -> String {
    // `UserBar.php:35-43`: `-1` never, `1` always, otherwise only when signed in.
    if !ctx.settings.shows_user_bar(ctx.viewer.is_some()) {
        return String::new();
    }
    let messages = tr.messages();
    let label = escape_html(messages.user_bar_label);
    let redirect = urlencode(&ctx.current_path);
    let Some(viewer) = &ctx.viewer else {
        return format!(
            "<nav id=\"user-bar\" aria-label=\"{label}\">\
             <a href=\"/login?redirect={redirect}\" class=\"login\">{login}</a></nav>\n",
            login = escape_html(messages.log_in),
        );
    };
    let edit = match edit_link(view, ctx, &viewer.roles, messages) {
        Some(link) => format!(
            "<a href=\"{}\" class=\"admin-edit\">{}</a>",
            escape_html(&link.href),
            escape_html(link.label)
        ),
        None => String::new(),
    };
    format!(
        "<nav id=\"user-bar\" aria-label=\"{label}\">\
         <div class=\"site-links\">\
         <a href=\"/admin\" class=\"admin-dashboard\">{dashboard}</a>{edit}\
         </div>\
         <div class=\"user-links\">\
         <span class=\"user-id\">{signed_in}</span> \
         <form method=\"post\" action=\"/logout?redirect={redirect}\">\
         <button type=\"submit\" class=\"logout\">{logout}</button></form>\
         </div></nav>\n",
        dashboard = escape_html(messages.admin_dashboard),
        // `user-bar.phtml:22`: sprintf('Signed in as %s', '<strong>name</strong>').
        signed_in = sprintf1(
            &escape_html(messages.signed_in_as),
            &format!("<strong>{}</strong>", escape_html(&viewer.name)),
        ),
        logout = escape_html(messages.log_out),
    )
}

/// An admin editor the caller may open from the bar.
struct EditLink {
    href: String,
    label: &'static str,
}

/// The editor for what the view shows, when the caller may update it.
///
/// `UserBar.php::links` offers it when `$resource->userIsAllowed('edit')`; the
/// port asks for `update`, the privilege the save itself needs, as
/// `AbstractEntityRepresentation::userIsAllowed` does: the adapter grant and
/// the bound entity grant both.
fn edit_link(
    view: &View<'_>,
    ctx: &RenderContext<'_>,
    roles: &RoleSet,
    messages: &Messages,
) -> Option<EditLink> {
    let (adapter, entity, href, label) = match view {
        View::Page => {
            let page = ctx.page?;
            (
                Adapter::SitePage,
                Context::SitePage {
                    site: ctx.site.id,
                    site_owner: ctx.site.owner,
                    site_is_public: ctx.site.is_public,
                    is_public: page.is_public,
                },
                format!("/#/sites/{}/pages/{}", ctx.site.id, page.id),
                messages.edit_page,
            )
        }
        View::Item(resource) | View::ItemSet(resource) | View::Media(resource) => {
            resource_edit(resource, messages)
        }
        _ => return None,
    };
    let may_update = allows(roles, Action::Update, &Context::adapter(adapter))
        && allows(roles, Action::Update, &entity);
    may_update.then_some(EditLink { href, label })
}

/// The adapter, the bound entity, the admin route and the label for one
/// resource (`web/admin/src/pages/{items,item-sets,media}/page.tsx`).
fn resource_edit(
    resource: &ResourceView,
    messages: &Messages,
) -> (Adapter, Context, String, &'static str) {
    let (adapter, entity, route, label) = match resource.kind {
        ResourceKind::Item => (
            Adapter::Item,
            Context::item(resource.owner),
            "items",
            messages.edit_item,
        ),
        ResourceKind::ItemSet => (
            Adapter::ItemSet,
            Context::item_set(resource.owner),
            "item-sets",
            messages.edit_item_set,
        ),
        ResourceKind::Media => (
            Adapter::Media,
            Context::media(resource.owner),
            "media",
            messages.edit_media,
        ),
    };
    (
        adapter,
        entity,
        format!("/#/{route}/{}", resource.id),
        label,
    )
}

#[cfg(test)]
#[path = "user_bar_tests.rs"]
mod tests;
