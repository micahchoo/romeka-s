//! The public search: the resource names a search covers, the pages it renders
//! and the advanced form's fields.
//!
//! Upstream sources: `src/Controller/Site/IndexController.php::searchAction` and
//! `CrossSiteSearchController.php` (which resources a search covers and the
//! scope each one queries), `view/omeka/site/index/search.phtml` and
//! `view/omeka/site/cross-site-search/*.phtml` (the results pages),
//! `view/omeka/site/page/browse.phtml` (the site-page result list),
//! `view/common/search-form.phtml` (the sitewide fulltext form) and
//! `view/common/advanced-search.phtml` with its partials (the advanced form).
//!
//! The three resource names live in one table ([`REGISTRY`]): the wire name the
//! API and `site_setting.search_resource_names` use, the controller segment a
//! site URL uses and the cross-site action that lists the resources. Upstream
//! spreads the same three names over a `switch`, two arrays in a view and a
//! `$resources` map in a controller; one table keeps the names from drifting
//! apart. The labels a section prints live in the message catalogue
//! (`i18n::SearchLabels`).
//!
//! Everything here is a value or a pure transform. The server reads rows and
//! builds these; the theme renders them.

use crate::html::query_string;
use crate::slug::{self, Slug};
use crate::view::Thumbnail;

/// One of the three resource names a search covers, the values
/// `SiteSettingsForm`'s `search_resource_names` offers and
/// `IndexController::searchAction` iterates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SearchResource {
    /// `site_pages`.
    SitePages,
    /// `items`.
    Items,
    /// `item_sets`.
    ItemSets,
}

/// What one resource name carries: its wire name, the site controller that
/// lists it and the cross-site action that lists it.
struct Entry {
    wire: &'static str,
    controller: &'static str,
    cross_action: &'static str,
    body: &'static str,
}

/// The resource-name table, indexed by [`SearchResource`].
const REGISTRY: [Entry; 3] = [
    Entry {
        wire: "site_pages",
        controller: "page",
        cross_action: "site-pages",
        body: "page",
    },
    Entry {
        wire: "items",
        controller: "item",
        cross_action: "items",
        body: "items",
    },
    Entry {
        wire: "item_sets",
        controller: "item-set",
        cross_action: "item-sets",
        body: "item-sets",
    },
];

impl SearchResource {
    /// Every resource name, in the order `SiteSettingsForm` offers them.
    pub const ALL: [Self; 3] = [Self::SitePages, Self::Items, Self::ItemSets];

    const fn entry(self) -> &'static Entry {
        &REGISTRY[self as usize]
    }

    /// The name a stored `site_setting` holds, the API search uses and the
    /// section's CSS class is built from.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.entry().wire
    }

    /// The site controller segment that lists this resource, as
    /// `$resourceControllers` and `controller_for` spell it.
    #[must_use]
    pub const fn controller(self) -> &'static str {
        self.entry().controller
    }

    /// The cross-site action that lists this resource alone
    /// (`CrossSiteSearchController`).
    #[must_use]
    pub const fn cross_action(self) -> &'static str {
        self.entry().cross_action
    }

    /// The body class the search views append for this resource
    /// (`item/search.phtml`: `items search`, `item-set/search.phtml`:
    /// `item-sets search`, `page/browse.phtml`: `page browse`).
    #[must_use]
    pub const fn body_class(self) -> &'static str {
        self.entry().body
    }

    /// The resource a stored name selects, `None` for a name
    /// `search_resource_names` does not offer.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|entry| entry.name() == name)
    }

    /// The resource a site controller segment names, the inverse of
    /// [`SearchResource::controller`].
    #[must_use]
    pub fn from_controller(controller: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|entry| entry.controller() == controller)
    }

    /// `index/search.phtml`: where a section's `View all results` link points,
    /// the site browse for this resource with the needle in the query.
    #[must_use]
    pub fn site_all_url(self, site: &Slug, needle: &str) -> String {
        slug::resource_browse_url(site, self.controller(), fulltext_query(needle).as_deref())
    }

    /// `common/cross-site-search/results.phtml`: the same link across sites.
    #[must_use]
    pub fn cross_all_url(self, site: &Slug, needle: &str) -> String {
        let action = slug::resource_action_url(site, "cross-site-search", self.cross_action());
        match fulltext_query(needle) {
            Some(query) => format!("{action}?{query}"),
            None => action,
        }
    }
}

/// `['fulltext_search' => $needle]` as `http_build_query` writes it. An absent
/// needle is null upstream and drops out of the query entirely.
fn fulltext_query(needle: &str) -> Option<String> {
    if needle.is_empty() {
        return None;
    }
    query_string(&[("fulltext_search".to_string(), needle.to_string())])
}

/// A site a cross-site result belongs to, the link `results.phtml` prints under
/// a site-page result (`$sitePage->site()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteRef {
    /// The site title.
    pub title: String,
    /// The site URL.
    pub url: String,
}

impl SiteRef {
    /// A site link.
    #[must_use]
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
        }
    }
}

/// One result entry: the title, the URL, the thumbnail a result list shows when
/// the resource has one, and the site the entry belongs to when the search spans
/// sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// `displayTitle`.
    pub title: String,
    /// The resource's site URL.
    pub url: String,
    /// The square thumbnail `linkPretty('square')` prints, `None` when the
    /// resource has no thumbnail.
    pub thumbnail: Option<Thumbnail>,
    /// The site a cross-site result belongs to, `None` on a site search.
    pub site: Option<SiteRef>,
}

impl SearchHit {
    /// A hit with a title and a URL.
    #[must_use]
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            thumbnail: None,
            site: None,
        }
    }
}

/// One resource section of a results page: the hits and the total the query
/// matched before the limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSection {
    /// Which resource this section lists.
    pub resource: SearchResource,
    /// The hits on this page.
    pub hits: Vec<SearchHit>,
    /// `getTotalResults()`, the number the section's link prints.
    pub total: usize,
}

impl SearchSection {
    /// A section of one resource kind.
    #[must_use]
    pub fn new(resource: SearchResource, hits: Vec<SearchHit>, total: usize) -> Self {
        Self {
            resource,
            hits,
            total,
        }
    }
}

/// The combined results page: one section per configured resource name
/// (`index/search.phtml`, `common/cross-site-search/results.phtml`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchResults {
    /// `fulltext_search`, empty when the request carried none.
    pub needle: String,
    /// The sections, in the order the site's `search_resource_names` holds. A
    /// resource with no match is not a section, which is what upstream's
    /// `if (!$totalResults) continue;` leaves.
    pub sections: Vec<SearchSection>,
}

impl SearchResults {
    /// The results of a needle with no sections yet.
    #[must_use]
    pub fn new(needle: impl Into<String>) -> Self {
        Self {
            needle: needle.into(),
            sections: Vec::new(),
        }
    }

    /// Whether every section is absent, the `empty($results)` test.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

/// A single-resource result list with pagination: the cross-site `items`,
/// `item-sets` and `site-pages` actions, and the site page browse
/// (`common/cross-site-search/item-results.phtml`, `page/browse.phtml`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchList {
    /// Which resource this list shows.
    pub resource: SearchResource,
    /// The hits on this page.
    pub hits: Vec<SearchHit>,
    /// The total the query matched.
    pub total: usize,
    /// The one-based page number.
    pub page: usize,
    /// `per_page`, for the page count.
    pub per_page: usize,
}

/// The cross-site search page (`CrossSiteSearchController`), whose actions
/// differ only in their body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossSitePage {
    /// `fulltext_search`, empty when the request carried none.
    pub needle: String,
    /// Where the page's fulltext form posts, the `results` action.
    pub form_action: String,
    /// What the action renders.
    pub body: CrossSiteBody,
}

/// The three shapes a cross-site search action renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossSiteBody {
    /// `index.phtml`: the fulltext form alone.
    Form,
    /// `results.phtml`: one section per configured resource name.
    Results(Vec<SearchSection>),
    /// `items.phtml` and its siblings: one paginated list.
    List(Box<SearchList>),
}

/// Which of the two places a site's search form sends the visitor
/// (`site_setting.search_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchType {
    /// `sitewide`, the default: this site's own search page.
    #[default]
    Sitewide,
    /// `cross-site`: the cross-site results page.
    CrossSite,
}

impl SearchType {
    /// The type a stored setting names. Anything but `cross-site` is the
    /// default, which is what `search-form.phtml`'s `switch` does.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "cross-site" => Self::CrossSite,
            _ => Self::Sitewide,
        }
    }

    /// `common/search-form.phtml`: where the sitewide form posts.
    #[must_use]
    pub fn form_action(self, site: &Slug) -> String {
        match self {
            // `url('site/resource', ['controller' => 'index', 'action' => 'search'])`.
            Self::Sitewide => slug::resource_action_url(site, "index", "search"),
            Self::CrossSite => cross_site_form_action(site),
        }
    }
}

/// Where the cross-site fulltext form posts
/// (`CrossSiteSearchController::resultsAction`).
#[must_use]
pub fn cross_site_form_action(site: &Slug) -> String {
    slug::resource_action_url(site, "cross-site-search", "results")
}

/// One option of a form select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormOption {
    /// The option's value.
    pub value: String,
    /// The option's label.
    pub label: String,
}

impl FormOption {
    /// An option.
    #[must_use]
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// One `property[i][...]` row of the advanced form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyRow {
    /// The index in the posted name (`property[0][text]`).
    pub index: usize,
    /// `joiner`, `and` or `or`.
    pub joiner: String,
    /// `property`, the property id as text, empty for any property.
    pub property: String,
    /// `type`, the comparison the row carries.
    pub kind: String,
    /// `text`, the value the row compares.
    pub text: String,
}

/// One `item_set_id[]` row: the set and whether the item must be in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSetRow {
    /// The item-set id as text, empty for an unset select.
    pub id: String,
    /// `not_in` when the id arrived as `not_item_set_id[]`.
    pub not_in: bool,
}

/// The advanced search form (`common/advanced-search.phtml`): the query being
/// edited, where the form posts, and the options its selects offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvancedSearchForm {
    /// Which resource the form searches, which decides its fields.
    pub resource: SearchResource,
    /// Where the form posts (GET).
    pub action: String,
    /// The request query, so every field keeps its value.
    pub query: Vec<(String, String)>,
    /// The property select's options, `(id, label)`.
    pub properties: Vec<FormOption>,
    /// The resource-class select's options, `(id, label)`.
    pub classes: Vec<FormOption>,
    /// The item-set select's options, `(id, label)`. Empty hides the field,
    /// which is what `item-sets.phtml` does for a site with no item sets.
    pub item_sets: Vec<FormOption>,
}

impl AdvancedSearchForm {
    /// A form over a query with no select options.
    #[must_use]
    pub fn new(
        resource: SearchResource,
        action: impl Into<String>,
        query: Vec<(String, String)>,
    ) -> Self {
        Self {
            resource,
            action: action.into(),
            query,
            properties: Vec::new(),
            classes: Vec::new(),
            item_sets: Vec::new(),
        }
    }

    /// `$query[$name]`, the value a single-valued field keeps.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// `$query[$name]` as `advanced-search/ids.phtml` writes it: a repeated
    /// name or a list becomes a comma-separated string.
    #[must_use]
    pub fn comma_joined(&self, name: &str) -> String {
        list_values(&self.query, name).join(",")
    }

    /// The `property[i][...]` rows, filtered the way `properties.phtml` filters
    /// them: a row with a blank `text` is dropped, the rest are renumbered from
    /// zero, and no row leaves one empty row for the template to print.
    #[must_use]
    pub fn property_rows(&self) -> Vec<PropertyRow> {
        let mut rows: Vec<PropertyRow> = Vec::new();
        for fields in bracket_rows(&self.query, "property") {
            let text = field(&fields, "text");
            if !keeps_row(&text) {
                continue;
            }
            rows.push(PropertyRow {
                index: rows.len(),
                joiner: field(&fields, "joiner"),
                property: field(&fields, "property"),
                kind: field(&fields, "type"),
                text,
            });
        }
        if rows.is_empty() {
            rows.push(PropertyRow {
                index: 0,
                joiner: "and".to_string(),
                ..PropertyRow::default()
            });
        }
        rows
    }

    /// The `resource_class_id[]` values.
    #[must_use]
    pub fn class_ids(&self) -> Vec<String> {
        list_values(&self.query, "resource_class_id")
    }

    /// The item-set rows: every `item_set_id[]` then every `not_item_set_id[]`,
    /// as `item-sets.phtml` builds them. No id leaves one unset row.
    #[must_use]
    pub fn item_set_rows(&self) -> Vec<ItemSetRow> {
        let mut rows: Vec<ItemSetRow> = list_values(&self.query, "item_set_id")
            .into_iter()
            .map(|id| ItemSetRow { id, not_in: false })
            .chain(
                list_values(&self.query, "not_item_set_id")
                    .into_iter()
                    .map(|id| ItemSetRow { id, not_in: true }),
            )
            .collect();
        if rows.is_empty() {
            rows.push(ItemSetRow {
                id: String::new(),
                not_in: false,
            });
        }
        rows
    }
}

/// `array_filter` over a row's `text`: a row is kept when it carries no text
/// key or its text is not blank.
fn keeps_row(text: &str) -> bool {
    text.is_empty() || !text.trim().is_empty()
}

/// One `name[index][field]` group, in the order the indexes first appear.
fn bracket_rows(query: &[(String, String)], name: &str) -> Vec<Vec<(String, String)>> {
    let mut groups: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<(String, String)>> = Vec::new();
    for (key, value) in query {
        let Some((index, field)) = split_bracket_key(key, name) else {
            continue;
        };
        let position = match groups.iter().position(|seen| *seen == index) {
            Some(position) => position,
            None => {
                groups.push(index);
                rows.push(Vec::new());
                rows.len() - 1
            }
        };
        rows[position].push((field, value.clone()));
    }
    rows
}

/// The index and field of a `name[index][field]` key.
fn split_bracket_key(key: &str, name: &str) -> Option<(String, String)> {
    let rest = key.strip_prefix(name)?.strip_prefix('[')?;
    let (index, tail) = rest.split_once(']')?;
    let field = tail.strip_prefix('[')?.strip_suffix(']')?;
    if index.is_empty() || field.is_empty() || field.contains(['[', ']']) {
        return None;
    }
    Some((index.to_string(), field.to_string()))
}

fn field(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

/// The values of a parameter written as a repeated name, a `name[]` list or a
/// `name[0]` list. PHP `parse_str` turns all three into an array.
fn list_values(query: &[(String, String)], name: &str) -> Vec<String> {
    let bracketed = format!("{name}[]");
    query
        .iter()
        .filter(|(key, _)| {
            *key == name || *key == bracketed || split_index_key(key, name).is_some()
        })
        .map(|(_, value)| value.clone())
        .collect()
}

/// The index of a `name[index]` key.
fn split_index_key(key: &str, name: &str) -> Option<String> {
    let rest = key.strip_prefix(name)?.strip_suffix(']')?;
    let index = rest.strip_prefix('[')?;
    if index.is_empty() || index.contains(['[', ']']) {
        return None;
    }
    Some(index.to_string())
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
