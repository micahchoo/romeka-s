//! `BlockLayout/PageDateTime.php` and
//! `view/common/block-layout/page-date-time.phtml`, with the date formatting in
//! `View/Helper/I18n.php::dateFormat` through [`crate::i18n`].
//!
//! `dateFormat` prefers PHP's `intl` when it is loaded and falls back to a
//! fixed English PHP date pattern otherwise (`View/Helper/I18n.php:93-140`).
//! The port has no ICU, so [`crate::i18n::Translator::format_datetime`] carries
//! a per-locale date-name table and the same `date_format`/`time_format`
//! mapping; that module documents the table.
//!
//! The stored value is a naive timestamp (`resource.Timestamp` wraps
//! `NaiveDateTime`), so the port cannot print the timezone abbreviation `T`
//! that `full` asks for, and does not apply the viewer's timezone as upstream
//! does. Both are documented gaps, not silent ones.
//!
//! Core's `SitePage` has no `created`/`modified` yet (wave 2 follow-up), so the
//! dates arrive on [`RenderContext::page_created`] and a missing date renders no
//! row.

use omeka_core::site::{BlockData, PageDateTimeData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `pageDateTime` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct PageDateTime;

impl BlockLayout for PageDateTime {
    fn name(&self) -> &'static str {
        "pageDateTime"
    }

    fn label(&self) -> &'static str {
        "Page date/time"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::PageDateTime(data) = &block.data else {
            return String::new();
        };
        render_block(data, ctx)
    }
}

fn render_block(data: &PageDateTimeData, ctx: &RenderContext<'_>) -> String {
    let tr = ctx.translator();
    let messages = tr.messages();
    let display = data.display.as_deref().unwrap_or("created_modified");
    let date_format = data.date_format.as_deref().unwrap_or("medium");
    let time_format = data.time_format.as_deref().unwrap_or("none");

    let created = ctx
        .page_created
        .filter(|_| matches!(display, "created" | "created_modified"))
        .map(|timestamp| tr.format_datetime(timestamp.as_naive(), date_format, time_format));
    let modified = ctx
        .page_modified
        .filter(|_| matches!(display, "modified" | "created_modified"))
        .map(|timestamp| tr.format_datetime(timestamp.as_naive(), date_format, time_format));

    if created.is_none() && modified.is_none() {
        return String::new();
    }
    let mut out = String::from("<div class=\"page-date-time\">");
    if let Some(created) = created {
        out.push_str(&row("created", messages.created, &created));
    }
    if let Some(modified) = modified {
        out.push_str(&row("modified", messages.modified, &modified));
    }
    out.push_str("</div>");
    out
}

fn row(class: &str, property: &str, value: &str) -> String {
    format!(
        "<div class=\"{class}\"><span class=\"property\">{}</span>: <span class=\"value\">{}</span></div>",
        escape_html(property),
        escape_html(value)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site, timestamp};
    use serde_json::json;

    /// `block-layout/page-date-time.phtml`: created and modified rows, each
    /// labelled and formatted.
    #[test]
    fn page_date_time_matches_upstream_block_layout() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.page_created = Some(timestamp());
        ctx.page_modified = Some(timestamp());
        let stored = block(
            "pageDateTime",
            json!({"display": "created_modified", "date_format": "medium", "time_format": "none"}),
        );
        assert_eq!(
            PageDateTime.render(&stored, &ctx),
            "<div class=\"page-date-time\"><div class=\"created\"><span class=\"property\">Created</span>: <span class=\"value\">Oct 16, 2026</span></div><div class=\"modified\"><span class=\"property\">Modified</span>: <span class=\"value\">Oct 16, 2026</span></div></div>"
        );
    }

    /// A site locale of `es` renders the same block with Spanish labels and
    /// dates, from the same stored block data.
    #[test]
    fn page_date_time_renders_in_the_site_locale() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.settings.locale = Some("es".to_string());
        ctx.page_created = Some(timestamp());
        ctx.page_modified = Some(timestamp());
        let stored = block(
            "pageDateTime",
            json!({"display": "created_modified", "date_format": "long", "time_format": "none"}),
        );
        assert_eq!(
            PageDateTime.render(&stored, &ctx),
            "<div class=\"page-date-time\"><div class=\"created\"><span class=\"property\">Creado</span>: <span class=\"value\">16 de octubre de 2026</span></div><div class=\"modified\"><span class=\"property\">Modificado</span>: <span class=\"value\">16 de octubre de 2026</span></div></div>"
        );
    }

    /// `display` picks the rows: `created` alone hides the modified row.
    #[test]
    fn page_date_time_display_selects_rows() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.page_created = Some(timestamp());
        ctx.page_modified = Some(timestamp());
        let stored = block("pageDateTime", json!({"display": "created"}));
        let out = PageDateTime.render(&stored, &ctx);
        assert!(out.contains("class=\"created\""), "{out}");
        assert!(!out.contains("class=\"modified\""), "{out}");
    }

    /// A page with no stored dates and no modified value renders nothing.
    #[test]
    fn page_date_time_without_dates_renders_nothing() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.page_created = Some(timestamp());
        let stored = block("pageDateTime", json!({"display": "modified"}));
        assert_eq!(PageDateTime.render(&stored, &ctx), "");
    }
}
