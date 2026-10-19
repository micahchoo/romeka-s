//! The labels the public search views render, one table per hand-written
//! locale. The machine-translated locales parse theirs from `locales/`.

use crate::search::SearchResource;

/// The labels the public search reads: the sitewide search form, the advanced
/// search form and the results pages.
///
/// These are the strings `view/common/search-form.phtml`,
/// `view/common/advanced-search*.phtml` and the search result templates pass to
/// `translate()`. They sit beside [`Messages`] rather than inside it because
/// only the search views read them, and the `property[i][type]` select carries a
/// fixed option table ([`SearchLabels::comparisons`]) instead of one field per
/// comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLabels {
    /// `common/search-form.phtml` and the forms' submit label.
    pub search: &'static str,
    /// `search-form.phtml`: the fulltext input's aria label.
    pub query: &'static str,
    /// `search-form.phtml`: the submit button's label.
    pub submit: &'static str,
    /// `advanced-search/sort.phtml`.
    pub sort: &'static str,
    /// `advanced-search/sort.phtml`: the `sort_by` select's empty option.
    pub select_sort_by: &'static str,
    /// `advanced-search/sort.phtml`: the `sort_order` select's empty option.
    pub select_sort_order: &'static str,
    /// `advanced-search/sort.phtml`: the `asc` option.
    pub ascending: &'static str,
    /// `advanced-search/sort.phtml`: the `desc` option.
    pub descending: &'static str,
    /// `advanced-search/fulltext.phtml`.
    pub search_full_text: &'static str,
    /// `advanced-search/properties.phtml`: the field group's label.
    pub search_by_value: &'static str,
    /// `advanced-search/properties.phtml`: the text input's aria label.
    pub query_text: &'static str,
    /// `advanced-search/properties.phtml`: the property select's aria label.
    pub property: &'static str,
    /// `advanced-search/properties.phtml`: the type select's aria label.
    pub query_type: &'static str,
    /// `advanced-search/properties.phtml`: the joiner select's aria label.
    pub joiner: &'static str,
    /// `advanced-search/properties.phtml`: the `and` joiner option.
    pub and: &'static str,
    /// `advanced-search/properties.phtml`: the `or` joiner option.
    pub or: &'static str,
    /// `advanced-search/properties.phtml`: the property select's empty option.
    pub any_property: &'static str,
    /// `advanced-search/resource-class.phtml`.
    pub search_by_class: &'static str,
    /// `advanced-search/item-sets.phtml`.
    pub search_by_item_set: &'static str,
    /// `advanced-search/item-sets.phtml`: the `in` condition option.
    pub in_set: &'static str,
    /// `advanced-search/item-sets.phtml`: the `not_in` condition option.
    pub not_in_set: &'static str,
    /// `advanced-search/has-media.phtml`.
    pub search_by_media_presence: &'static str,
    /// `advanced-search/has-media.phtml`: the select's empty option.
    pub select_media_presence: &'static str,
    /// `advanced-search/has-media.phtml`: the `1` option.
    pub has_media: &'static str,
    /// `advanced-search/has-media.phtml`: the `0` option.
    pub has_no_media: &'static str,
    /// `advanced-search/ids.phtml`.
    pub search_by_id: &'static str,
    /// `view/omeka/site/index/search.phtml`: the results heading pattern.
    pub search_results: &'static str,
    /// `index/search.phtml` and `common/cross-site-search/results.phtml`.
    pub no_result_found: &'static str,
    /// `index/search.phtml`: the per-section count link pattern.
    pub view_all_results: &'static str,
    /// `common/cross-site-search/item-results.phtml`: the heading pattern.
    pub item_results: &'static str,
    /// `common/cross-site-search/item-set-results.phtml`: the heading pattern.
    pub item_set_results: &'static str,
    /// `common/cross-site-search/site-page-results.phtml`: the heading pattern.
    pub site_page_results: &'static str,
    /// `common/cross-site-search/item-results.phtml`: the advanced form link.
    pub advanced_item_search: &'static str,
    /// `common/cross-site-search/item-set-results.phtml`: the same link.
    pub advanced_item_set_search: &'static str,
    /// `view/omeka/site/item-set/search.phtml`: the page title. Upstream spells
    /// it with a capital S, unlike the cross-site link above.
    pub advanced_item_set_form_title: &'static str,
    /// `view/omeka/site/page/browse.phtml`: the heading.
    pub pages: &'static str,
    /// `page/browse.phtml`: the empty-result message.
    pub no_pages_found: &'static str,
    /// The section label for `site_pages` (`SiteSettingsForm`).
    pub site_pages: &'static str,
    /// The section label for `items`.
    pub items: &'static str,
    /// The section label for `item_sets`.
    pub item_sets: &'static str,
    /// The `property[i][type]` select's options, `(value, label)`, in the order
    /// `advanced-search/properties.phtml` writes them.
    pub comparisons: [(&'static str, &'static str); 14],
    /// The `sort_by` select's options, `(value, label)`. Upstream reads them
    /// from `sort_defaults.public` (`config/module.config.php`); the port offers
    /// the fields its sort understands.
    pub sort_fields: [(&'static str, &'static str); 3],
}

impl SearchLabels {
    /// The label a results section prints for one resource
    /// (`index/search.phtml`'s `$resourceLabels`).
    #[must_use]
    pub const fn resource_label(&self, resource: SearchResource) -> &'static str {
        match resource {
            SearchResource::SitePages => self.site_pages,
            SearchResource::Items => self.items,
            SearchResource::ItemSets => self.item_sets,
        }
    }

    /// `pageTitle(...)` of an advanced form view: `item/search.phtml` for items,
    /// `item-set/search.phtml` for item sets.
    #[must_use]
    pub const fn form_title(&self, resource: SearchResource) -> &'static str {
        match resource {
            SearchResource::ItemSets => self.advanced_item_set_form_title,
            _ => self.advanced_item_search,
        }
    }
}

/// The English search labels, the source language.
pub static EN_SEARCH: SearchLabels = SearchLabels {
    search: "Search",
    query: "Query",
    submit: "Submit",
    sort: "Sort",
    select_sort_by: "Select sort by…",
    select_sort_order: "Select sort order…",
    ascending: "Ascending",
    descending: "Descending",
    search_full_text: "Search full-text",
    search_by_value: "Search by value",
    query_text: "Query text",
    property: "Property",
    query_type: "Query type",
    joiner: "Joiner",
    and: "AND",
    or: "OR",
    any_property: "[Any Property]",
    search_by_class: "Search by class",
    search_by_item_set: "Search by item set",
    in_set: "In",
    not_in_set: "Not in",
    search_by_media_presence: "Search by media presence",
    select_media_presence: "Select media presence…",
    has_media: "Has media",
    has_no_media: "Has no media",
    search_by_id: "Search by ID",
    search_results: "Search results for “%s”",
    no_result_found: "No result found",
    view_all_results: "View all results (%s total)",
    item_results: "Item results for \"%s\"",
    item_set_results: "Item set results for \"%s\"",
    site_page_results: "Site page results for \"%s\"",
    advanced_item_search: "Advanced item search",
    advanced_item_set_search: "Advanced item set search",
    advanced_item_set_form_title: "Advanced item set Search",
    pages: "Pages",
    no_pages_found: "No pages found",
    site_pages: "Site pages",
    items: "Items",
    item_sets: "Item sets",
    comparisons: [
        ("eq", "is exactly"),
        ("neq", "is not exactly"),
        ("in", "contains"),
        ("nin", "does not contain"),
        ("sw", "starts with"),
        ("nsw", "does not start with"),
        ("ew", "ends with"),
        ("new", "does not end with"),
        ("res", "is resource with ID"),
        ("nres", "is not resource with ID"),
        ("ex", "has any value"),
        ("nex", "has no values"),
        ("dt", "has data type"),
        ("ndt", "does not have data type"),
    ],
    sort_fields: [("title", "Title"), ("created", "Created"), ("id", "ID")],
};

/// The Spanish search labels. Terms match `application/language/es.po` where the
/// message id is the same; a string that catalogue leaves untranslated keeps the
/// English text, which is what gettext falls back to.
pub static ES_SEARCH: SearchLabels = SearchLabels {
    search: "Búsqueda",
    query: "Consulta",
    submit: "Enviar",
    sort: "Ordenar",
    select_sort_by: "Seleccionar ordenar por...",
    select_sort_order: "Seleccionar orden de clasificación...",
    ascending: "Ascendente",
    descending: "Descendente",
    search_full_text: "Buscar en cualquier campo",
    search_by_value: "Buscar por valor",
    query_text: "Consulta de texto",
    property: "Propiedad",
    query_type: "Tipo de consulta",
    joiner: "Unión (joiner)",
    and: "Y",
    or: "O",
    any_property: "[Any Property]",
    search_by_class: "Buscar por clase",
    search_by_item_set: "Buscar por conjunto de fichas",
    in_set: "En",
    not_in_set: "No en",
    search_by_media_presence: "Buscar por presencia de medios",
    select_media_presence: "Seleccionar presencia de medios...",
    has_media: "Tiene medios",
    has_no_media: "No tiene medios",
    search_by_id: "Buscar por ID",
    search_results: "Resultados de búsqueda para \"%s\"",
    no_result_found: "No se encontraron resultados",
    view_all_results: "Ver todos los resultados (%s total)",
    item_results: "Item results for \"%s\"",
    item_set_results: "Item set results for \"%s\"",
    site_page_results: "Site page results for \"%s\"",
    advanced_item_search: "Búsqueda avanzada de fichas",
    advanced_item_set_search: "Búsqueda avanzada de conjuntos de fichas",
    advanced_item_set_form_title: "Búsqueda avanzada de conjunto de fichas",
    pages: "Páginas",
    no_pages_found: "No se encontraron páginas",
    site_pages: "Páginas del sitio",
    items: "Fichas",
    item_sets: "Conjuntos de fichas",
    comparisons: [
        ("eq", "es exactamente"),
        ("neq", "no es exactamente"),
        ("in", "contiene"),
        ("nin", "no contiene"),
        ("sw", "comienza con"),
        ("nsw", "no comienza con"),
        ("ew", "termina con"),
        ("new", "no termina con"),
        ("res", "es un recurso con ID"),
        ("nres", "no es un recurso con ID"),
        ("ex", "contiene información"),
        ("nex", "está vacío"),
        ("dt", "has data type"),
        ("ndt", "does not have data type"),
    ],
    sort_fields: [("title", "Título"), ("created", "Creado"), ("id", "ID")],
};
