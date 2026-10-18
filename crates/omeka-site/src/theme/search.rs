//! The bundled theme's search views.
//!
//! Ported from `view/common/search-form.phtml` (the sitewide fulltext form),
//! `view/common/advanced-search.phtml` with the partials it adds, and the
//! results templates: `view/omeka/site/index/search.phtml`,
//! `view/omeka/site/item/search.phtml`, `view/omeka/site/item-set/search.phtml`,
//! `view/omeka/site/page/browse.phtml` and
//! `view/omeka/site/cross-site-search/*.phtml` with
//! `view/common/cross-site-search/*.phtml`.
//!
//! The comments name the upstream view each function ports. Where the port
//! departs:
//!
//! - The advanced form drops the `dataType()->getSelect` input (the site layer
//!   has no data-type registry), the `resource-template-restrict` hidden inputs
//!   (`search_restrict_templates` needs a template list) and the add/remove
//!   value buttons (`advanced-search.js` is not ported). `Omeka.prepareSearchForm`
//!   is still called, because upstream's `global.js` is what the frame loads.
//! - The property select lists the properties the server supplies, not the
//!   whole vocabulary or the site's used members; `vocabulary_scope` needs a
//!   catalog the form does not receive.
//! - A cross-site item or item set prints its title as a link to the resource on
//!   the site being served. Upstream lists one link per site the resource
//!   belongs to (`$item->sites()`); no reader maps a resource to its sites.
//! - The page browse prints each page title in an `h3`, where upstream
//!   `page/browse.phtml` uses an `h4`. Upstream's `h4` sits under the `Pages`
//!   `h2` and skips a level; the port keeps the outline contiguous (axe
//!   `heading-order`).

use crate::html::{Attr, escape_html, hyperlink, open_tag, pagination};
use crate::i18n::{SearchLabels, sprintf1};
use crate::search::{
    AdvancedSearchForm, CrossSiteBody, CrossSitePage, FormOption, ItemSetRow, PropertyRow,
    SearchHit, SearchList, SearchResource, SearchResults, SearchSection,
};
use crate::slug;
use crate::view::RenderContext;

/// `common/search-form.phtml`: the sitewide search form, in the document frame.
///
/// `search_type` picks the target: the site's own search page, or the
/// cross-site results page.
pub(crate) fn search_form(ctx: &RenderContext<'_>, labels: &SearchLabels) -> String {
    let action = ctx.settings.search_type().form_action(&ctx.site.slug);
    let needle = query_value(&ctx.query, "fulltext_search").unwrap_or("");
    format!(
        "<form action=\"{action}\" id=\"search-form\" role=\"search\" aria-label=\"{search}\">\n\
         <label class=\"visually-hidden\" for=\"fulltext-search\">{query}</label>\n\
         <input type=\"text\" id=\"fulltext-search\" name=\"fulltext_search\" value=\"{value}\">\n\
         <button id=\"search-submit\" type=\"submit\"><span class=\"o-icon-search\" aria-hidden=\"true\"></span><span class=\"search-submit-label\">{search}</span></button>\n\
         </form>",
        action = escape_html(&action),
        search = escape_html(labels.search),
        value = escape_html(needle),
        query = escape_html(labels.query),
    )
}

/// `view/omeka/site/index/search.phtml`: the heading, one section per resource
/// the caller searched, and `No result found` when none matched.
pub(crate) fn search_view(results: &SearchResults, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = format!(
        "<h1>{}</h1>",
        escape_html(&sprintf1(labels.search_results, &results.needle))
    );
    if results.is_empty() {
        out.push_str(&format!("<p>{}</p>", escape_html(labels.no_result_found)));
        return out;
    }
    for section in &results.sections {
        out.push_str(&search_section(section, &results.needle, ctx));
    }
    out
}

/// One `<div class="{name} results">` of `index/search.phtml`.
fn search_section(section: &SearchSection, needle: &str, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = format!(
        "<div class=\"{name} results\"><h2>{label}</h2><ul>",
        name = escape_html(section.resource.name()),
        label = escape_html(labels.resource_label(section.resource)),
    );
    for hit in &section.hits {
        out.push_str(&format!("<li>{}</li>", link_pretty(hit)));
    }
    out.push_str("</ul>");
    out.push_str(&hyperlink(
        &sprintf1(labels.view_all_results, &section.total.to_string()),
        Some(&section.resource.site_all_url(&ctx.site.slug, needle)),
        &[],
    ));
    out.push_str("</div>");
    out
}

/// `linkPretty('square')` with a title: the square thumbnail when the resource
/// has one, then the title, as one link.
fn link_pretty(hit: &SearchHit) -> String {
    let mut content = String::new();
    if let Some(thumbnail) = &hit.thumbnail {
        content.push_str(&format!(
            "<img src=\"{}\" alt=\"{}\">",
            escape_html(&thumbnail.url),
            escape_html(&thumbnail.alt)
        ));
    }
    content.push_str(&escape_html(&hit.title));
    format!("<a href=\"{}\">{}</a>", escape_html(&hit.url), content)
}

/// `view/omeka/site/cross-site-search/*.phtml`: the page's fulltext form, then
/// the action's body.
pub(crate) fn cross_site_view(page: &CrossSitePage, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = fulltext_form(&page.form_action, &page.needle, labels);
    match &page.body {
        CrossSiteBody::Form => {}
        CrossSiteBody::Results(sections) => {
            out.push_str(&cross_site_results(sections, &page.needle, ctx));
        }
        CrossSiteBody::List(list) => out.push_str(&cross_site_list(list, &page.needle, ctx)),
    }
    out
}

/// `common/cross-site-search/fulltext-form.phtml`.
fn fulltext_form(action: &str, needle: &str, labels: &SearchLabels) -> String {
    format!(
        "<form action=\"{action}\" class=\"sitewide-search-form\" role=\"search\">\n\
         <input type=\"text\" name=\"fulltext_search\" value=\"{value}\" aria-label=\"{query}\">\n\
         <input type=\"submit\" value=\"{search}\">\n\
         </form>",
        action = escape_html(action),
        value = escape_html(needle),
        query = escape_html(labels.query),
        search = escape_html(labels.search),
    )
}

/// `common/cross-site-search/results.phtml`: the heading and one section per
/// configured resource name, or `No result found`.
fn cross_site_results(sections: &[SearchSection], needle: &str, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = format!(
        "<h2>{}</h2>",
        escape_html(&sprintf1(labels.search_results, needle))
    );
    if sections.is_empty() {
        out.push_str(&format!("<p>{}</p>", escape_html(labels.no_result_found)));
        return out;
    }
    for section in sections {
        let mut body = format!(
            "<div class=\"{name} results\"><h3>{label}</h3><ul>",
            name = escape_html(section.resource.name()),
            label = escape_html(labels.resource_label(section.resource)),
        );
        for hit in &section.hits {
            body.push_str(&cross_site_hit(hit));
        }
        body.push_str("</ul>");
        body.push_str(&hyperlink(
            &sprintf1(labels.view_all_results, &section.total.to_string()),
            Some(&section.resource.cross_all_url(&ctx.site.slug, needle)),
            &[],
        ));
        body.push_str("</div>");
        out.push_str(&body);
    }
    out
}

/// One `li` of `common/cross-site-search/results.phtml`. A site page names the
/// site it belongs to; an item or an item set links to the resource on the site
/// being served.
fn cross_site_hit(hit: &SearchHit) -> String {
    let title = hyperlink(
        &hit.title,
        Some(&hit.url),
        &[Attr::text("class", "result-title")],
    );
    let mut out = format!("<li><span class=\"result-title\">{title}</span>");
    if let Some(site) = &hit.site {
        out.push_str(&format!(
            "<span class=\"result-site\">{}</span>",
            hyperlink(&site.title, Some(&site.url), &[])
        ));
    }
    out.push_str("</li>");
    out
}

/// `common/cross-site-search/item-results.phtml` and its item-set and
/// site-page siblings: the advanced link where upstream has one, the heading,
/// the list and the pagination.
fn cross_site_list(list: &SearchList, needle: &str, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = String::new();
    if let Some(action) = advanced_action(list.resource) {
        let url = slug::resource_action_url(&ctx.site.slug, "cross-site-search", action);
        let url = match crate::html::query_string(&ctx.query) {
            Some(query) => format!("{url}?{query}"),
            None => url,
        };
        let text = match list.resource {
            SearchResource::ItemSets => labels.advanced_item_set_search,
            _ => labels.advanced_item_search,
        };
        out.push_str(&hyperlink(
            text,
            Some(&url),
            &[Attr::text("class", "advanced-search")],
        ));
    }
    let pattern = match list.resource {
        SearchResource::SitePages => labels.site_page_results,
        SearchResource::ItemSets => labels.item_set_results,
        SearchResource::Items => labels.item_results,
    };
    out.push_str(&format!(
        "<h2>{}</h2>",
        escape_html(&sprintf1(pattern, needle))
    ));
    out.push_str(&format!(
        "<div class=\"{name} results\">",
        name = escape_html(list.resource.name())
    ));
    out.push_str(&list_body(list, ctx, cross_site_hit));
    out.push_str("</div>");
    out
}

/// The `items-advanced` and `item-sets-advanced` actions, which are the only
/// cross-site actions upstream gives an advanced form.
fn advanced_action(resource: SearchResource) -> Option<&'static str> {
    match resource {
        SearchResource::Items => Some("items-advanced"),
        SearchResource::ItemSets => Some("item-sets-advanced"),
        SearchResource::SitePages => None,
    }
}

/// `view/omeka/site/page/browse.phtml`: the `Pages` heading, the page list and
/// `No pages found` when the query matched none.
pub(crate) fn search_list_view(list: &SearchList, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = format!("<h2>{}</h2>", escape_html(labels.pages));
    if list.hits.is_empty() {
        out.push_str(&format!("<p>{}</p>", escape_html(labels.no_pages_found)));
        return out;
    }
    out.push_str(&list_body(list, ctx, page_hit_item));
    out
}

/// One `li` of `view/omeka/site/page/browse.phtml`. Upstream puts the page title
/// in an `h4`, which skips a level under the `Pages` `h2`; the port uses `h3` so
/// the outline has no gap (axe `heading-order`).
fn page_hit_item(hit: &SearchHit) -> String {
    format!(
        "<li class=\"page resource\"><h3>{}</h3></li>",
        hyperlink(&hit.title, Some(&hit.url), &[])
    )
}

/// The pagination, the `<ul>` and the pagination again, which every list
/// template prints.
fn list_body(
    list: &SearchList,
    ctx: &RenderContext<'_>,
    hit_item: fn(&SearchHit) -> String,
) -> String {
    let mut out = pagination(
        &ctx.current_path,
        &ctx.query,
        list.page,
        list.per_page,
        list.total,
    );
    out.push_str("<ul class=\"resource-list\">");
    for hit in &list.hits {
        out.push_str(&hit_item(hit));
    }
    out.push_str("</ul>");
    out.push_str(&pagination(
        &ctx.current_path,
        &ctx.query,
        list.page,
        list.per_page,
        list.total,
    ));
    out
}

/// `view/omeka/site/item/search.phtml` and `item-set/search.phtml`: the page
/// title, the form, and its submit button.
pub(crate) fn advanced_search_view(form: &AdvancedSearchForm, ctx: &RenderContext<'_>) -> String {
    let labels = ctx.translator().search_labels();
    let mut out = format!("<h2>{}</h2>", escape_html(labels.form_title(form.resource)));
    out.push_str(&format!(
        "<form id=\"advanced-search\" method=\"get\" action=\"{}\">",
        escape_html(&form.action)
    ));
    out.push_str("<div class=\"advanced-search-content\">");
    out.push_str(&sort_field(form, labels));
    out.push_str(&fulltext_field(form, labels));
    out.push_str(&property_field(form, labels));
    out.push_str(&class_field(form, labels));
    if form.resource == SearchResource::Items {
        out.push_str(&item_set_field(form, labels));
        out.push_str(&has_media_field(form, labels));
    }
    out.push_str(&id_field(form, labels));
    out.push_str("</div>");
    out.push_str(&format!(
        "<div id=\"page-actions\"><input type=\"submit\" name=\"submit\" value=\"{}\"></div>",
        escape_html(labels.search)
    ));
    out.push_str("</form>");
    out.push_str("<script>Omeka.prepareSearchForm($('#advanced-search'));</script>");
    out
}

/// `advanced-search/sort.phtml`. A field whose `sort_by_default` is set holds
/// the browse default, not a visitor's choice, so its select keeps the empty
/// option.
fn sort_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    let sort_by = selected_value(form, "sort_by", "sort_by_default");
    let sort_order = selected_value(form, "sort_order", "sort_order_default");
    let by = select(
        "sort_by",
        "",
        labels.sort,
        labels.select_sort_by,
        &owned_options(&labels.sort_fields),
        &sort_by,
    );
    let order = select(
        "sort_order",
        "",
        labels.sort,
        labels.select_sort_order,
        &owned_options(&[("asc", labels.ascending), ("desc", labels.descending)]),
        &sort_order,
    );
    format!(
        "<div class=\"field\" id=\"sort_field\" role=\"group\">\
         <div class=\"field-meta\"><label>{}</label></div>\
         <div class=\"inputs\">{by}{order}</div></div>",
        escape_html(labels.sort)
    )
}

/// The value a sort select keeps: the query's value unless the browse default
/// set it.
fn selected_value(form: &AdvancedSearchForm, name: &str, default_name: &str) -> String {
    if form.value(default_name).is_some() {
        return String::new();
    }
    form.value(name).unwrap_or("").to_string()
}

/// `advanced-search/fulltext.phtml`.
fn fulltext_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    format!(
        "<div class=\"field\" id=\"fulltext_search_field\">\
         <div class=\"field-meta\"><label for=\"fulltext_search\">{label}</label></div>\
         <div class=\"inputs\"><input type=\"text\" name=\"fulltext_search\" id=\"fulltext_search\" value=\"{value}\"></div>\
         </div>",
        label = escape_html(labels.search_full_text),
        value = escape_html(form.value("fulltext_search").unwrap_or("")),
    )
}

/// `advanced-search/properties.phtml`: one row per query row, each naming
/// `property[i][joiner]`, `property[i][property]`, `property[i][type]` and
/// `property[i][text]`.
fn property_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    let mut out = format!(
        "<div id=\"property-queries\" class=\"field multi-value\" role=\"group\">\
         <div class=\"field-meta\"><span class=\"label\">{}</span></div>\
         <div class=\"inputs\">",
        escape_html(labels.search_by_value)
    );
    for row in form.property_rows() {
        out.push_str(&property_row(&row, form, labels));
    }
    out.push_str("</div></div>");
    out
}

fn property_row(row: &PropertyRow, form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    let stem = format!("property[{}]", row.index);
    let joiner = select(
        &format!("{stem}[joiner]"),
        "joiner",
        labels.joiner,
        "",
        &owned_options(&[("and", labels.and), ("or", labels.or)]),
        &row.joiner,
    );
    let property = select(
        &format!("{stem}[property]"),
        "query-property",
        labels.property,
        labels.any_property,
        &form.properties,
        &row.property,
    );
    let kind = select(
        &format!("{stem}[type]"),
        "query-type",
        labels.query_type,
        "",
        &labels
            .comparisons
            .iter()
            .map(|(value, label)| FormOption::new(*value, *label))
            .collect::<Vec<_>>(),
        &row.kind,
    );
    let text = format!(
        "<input type=\"text\" class=\"query-text\" name=\"{stem}[text]\" value=\"{}\" aria-label=\"{}\">",
        escape_html(&row.text),
        escape_html(labels.query_text),
    );
    format!("<div class=\"value\">{joiner}{property}{kind}{text}</div>")
}

/// `advanced-search/resource-class.phtml`: one `resource_class_id[]` select per
/// class the query names, or one empty select.
fn class_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    let ids = form.class_ids();
    let ids: Vec<String> = if ids.is_empty() {
        vec![String::new()]
    } else {
        ids
    };
    let mut out = format!(
        "<div id=\"resource-class\" class=\"field multi-value\" role=\"group\">\
         <div class=\"field-meta\"><span class=\"label\">{}</span></div>\
         <div class=\"inputs\">",
        escape_html(labels.search_by_class)
    );
    for id in &ids {
        out.push_str(&format!(
            "<div class=\"value\">{}</div>",
            select(
                "resource_class_id[]",
                "",
                labels.search_by_class,
                "",
                &form.classes,
                id
            )
        ));
    }
    out.push_str("</div></div>");
    out
}

/// `advanced-search/item-sets.phtml`. A site with no item sets hides the field,
/// which is what the upstream partial returns early for.
fn item_set_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    if form.item_sets.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "<div id=\"item-sets\" class=\"field multi-value\" role=\"group\">\
         <div class=\"field-meta\"><span class=\"label\">{}</span></div>\
         <div class=\"inputs\">",
        escape_html(labels.search_by_item_set)
    );
    for row in form.item_set_rows() {
        out.push_str(&item_set_row(&row, form, labels));
    }
    out.push_str("</div></div>");
    out.push_str(ITEM_SET_SCRIPT);
    out
}

fn item_set_row(row: &ItemSetRow, form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    let name = if row.not_in {
        "not_item_set_id[]"
    } else {
        "item_set_id[]"
    };
    let condition = select(
        "",
        "item-set-select-type",
        labels.search_by_item_set,
        "",
        &owned_options(&[("in", labels.in_set), ("not_in", labels.not_in_set)]),
        if row.not_in { "not_in" } else { "in" },
    );
    let chosen = select(
        name,
        "item-set-select",
        labels.search_by_item_set,
        "",
        &form.item_sets,
        &row.id,
    );
    format!("<div class=\"value\">{condition}{chosen}</div>")
}

/// The inline script `item-sets.phtml` ships: the condition select renames the
/// item-set select it sits beside.
const ITEM_SET_SCRIPT: &str = "<script>\n\
$('#content').on('change', '.item-set-select-type', function() {\n\
    const typeSelect = $(this);\n\
    const itemSetSelect = typeSelect.closest('.value').find('.item-set-select');\n\
    if ('not_in' === typeSelect.val()) {\n\
        itemSetSelect.attr('name', 'not_item_set_id[]');\n\
    } else {\n\
        itemSetSelect.attr('name', 'item_set_id[]');\n\
    }\n\
});\n\
</script>";

/// `advanced-search/has-media.phtml`.
fn has_media_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    let options = owned_options(&[("1", labels.has_media), ("0", labels.has_no_media)]);
    format!(
        "<div class=\"field\" id=\"has_media_field\">\
         <div class=\"field-meta\"><label for=\"has_media\">{}</label></div>\
         <div class=\"inputs\">{}</div></div>",
        escape_html(labels.search_by_media_presence),
        select(
            "has_media",
            "",
            labels.search_by_media_presence,
            labels.select_media_presence,
            &options,
            form.value("has_media").unwrap_or(""),
        )
    )
}

/// `advanced-search/ids.phtml`.
fn id_field(form: &AdvancedSearchForm, labels: &SearchLabels) -> String {
    format!(
        "<div class=\"field\" id=\"id_field\">\
         <div class=\"field-meta\"><label for=\"id\">{}</label></div>\
         <div class=\"inputs\"><input type=\"text\" name=\"id\" id=\"id\" value=\"{}\"></div>\
         </div>",
        escape_html(labels.search_by_id),
        escape_html(&form.comma_joined("id")),
    )
}

/// One `<select>` in the shape `formSelect` renders: the optional empty option,
/// one option per entry, and `selected="selected"` on the matching value. The
/// class is the hook upstream's scripts and stylesheet use.
fn select(
    name: &str,
    class: &str,
    aria: &str,
    empty: &str,
    options: &[FormOption],
    selected: &str,
) -> String {
    let mut attrs = Vec::with_capacity(3);
    if !name.is_empty() {
        attrs.push(Attr::text("name", name));
    }
    if !class.is_empty() {
        attrs.push(Attr::text("class", class));
    }
    attrs.push(Attr::text("aria-label", aria));
    let mut out = open_tag("select", &attrs);
    if !empty.is_empty() {
        out.push_str(&option("", empty, selected.is_empty()));
    }
    for entry in options {
        out.push_str(&option(&entry.value, &entry.label, entry.value == selected));
    }
    out.push_str("</select>");
    out
}

fn option(value: &str, label: &str, selected: bool) -> String {
    format!(
        "<option value=\"{}\"{}>{}</option>",
        escape_html(value),
        if selected {
            " selected=\"selected\""
        } else {
            ""
        },
        escape_html(label)
    )
}

/// Form options from `(value, label)` pairs.
fn owned_options(pairs: &[(&str, &str)]) -> Vec<FormOption> {
    pairs
        .iter()
        .map(|(value, label)| FormOption::new(*value, *label))
        .collect()
}

fn query_value<'a>(query: &'a [(String, String)], name: &str) -> Option<&'a str> {
    query
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
