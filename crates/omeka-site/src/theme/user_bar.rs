//! The admin bar across the top of the public site (`common/user-bar.phtml`,
//! `View/Helper/UserBar.php`).

use crate::html::escape_html;
use crate::i18n::{Translator, sprintf1};
use crate::view::RenderContext;

/// `common/user-bar.phtml`: the admin bar a signed-in caller sees, or a login
/// link when `show_user_bar` is `1` and nobody is signed in. Upstream's
/// `site-links` also name the site and the current resource, with an edit
/// link the caller's permissions gate; the context carries no permissions, so
/// the port links the dashboard only.
pub(crate) fn render(ctx: &RenderContext<'_>, tr: Translator) -> String {
    // `UserBar.php:35-43`: `-1` never, `1` always, otherwise only when signed in.
    if !ctx.settings.shows_user_bar(ctx.signed_in) {
        return String::new();
    }
    let messages = tr.messages();
    if ctx.signed_in {
        let name = ctx.user_name.as_deref().unwrap_or("");
        format!(
            "<nav id=\"user-bar\" aria-label=\"{label}\">\
             <div class=\"site-links\">\
             <a href=\"/admin\" class=\"admin-dashboard\">{dashboard}</a>\
             </div>\
             <div class=\"user-links\">\
             <span class=\"user-id\">{signed_in}</span> \
             <a href=\"/logout\" class=\"logout\">{logout}</a>\
             </div></nav>\n",
            label = escape_html(messages.user_bar_label),
            dashboard = escape_html(messages.admin_dashboard),
            // `user-bar.phtml:22`: sprintf('Signed in as %s', '<strong>name</strong>').
            signed_in = sprintf1(
                &escape_html(messages.signed_in_as),
                &format!("<strong>{}</strong>", escape_html(name)),
            ),
            logout = escape_html(messages.log_out),
        )
    } else {
        format!(
            "<nav id=\"user-bar\" aria-label=\"{label}\">\
             <a href=\"/login\" class=\"login\">{login}</a></nav>\n",
            label = escape_html(messages.user_bar_label),
            login = escape_html(messages.log_in),
        )
    }
}

#[cfg(test)]
#[path = "user_bar_tests.rs"]
mod tests;
