//! The public site's locales: the message catalogue the theme and the blocks
//! render, and the locale-aware date formatter.
//!
//! Upstream reads a site's `locale` setting and switches the translator and the
//! default locale for the request (`Mvc\MvcListeners::bootstrapLocale`,
//! `application/src/Mvc/MvcListeners.php:376-381`). The port keeps the same
//! setting key (`site_setting.locale`) and the same effect: the theme and the
//! blocks render through a [`Translator`] built from it.
//!
//! Upstream's catalogues are gettext `.po`/`.mo` files under
//! `application/language/` (30 locales plus `template.pot` at this SHA).
//! DESIGN.md excludes full catalog completeness, so the port ships two: English
//! (the source language) and Spanish, whose terms follow upstream's `es.po` so
//! an overlapping string has its upstream translation.
//!
//! Dates: upstream's `View\Helper\I18n::dateFormat` prefers PHP `intl` (which
//! formats in the locale) and falls back to a fixed English PHP date pattern
//! when `intl` is absent (`application/src/View/Helper/I18n.php:93-140`). The
//! port has no ICU, so it carries a small per-locale date-name table and the
//! same `date_format`/`time_format` mapping. English reproduces the fallback
//! branch; Spanish names and patterns follow ICU's `es` data, which is the
//! branch `intl` would take.

use chrono::{Datelike, NaiveDateTime};

use crate::search::SearchResource;

/// A locale the site layer can render in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    /// English, the source language and the default.
    #[default]
    En,
    /// Spanish, the shipped second locale.
    Es,
}

impl Locale {
    /// The BCP 47 tag, the value `site_setting.locale` stores.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
        }
    }

    /// The locale a stored tag names. A bare language matches its region tags
    /// (`en_US` is English), and an unknown tag falls back to [`Locale::En`],
    /// the installation default.
    #[must_use]
    pub fn parse(tag: &str) -> Self {
        let language = tag.split(['_', '-']).next().unwrap_or("");
        match language {
            "es" => Self::Es,
            "en" => Self::En,
            _ => Self::default(),
        }
    }

    /// The translator for this locale.
    #[must_use]
    pub const fn translator(self) -> Translator {
        Translator { locale: self }
    }

    const fn messages(self) -> &'static Messages {
        match self {
            Self::En => &EN,
            Self::Es => &ES,
        }
    }

    const fn names(self) -> &'static DateNames {
        match self {
            Self::En => &EN_NAMES,
            Self::Es => &ES_NAMES,
        }
    }

    const fn search(self) -> &'static SearchLabels {
        match self {
            Self::En => &EN_SEARCH,
            Self::Es => &ES_SEARCH,
        }
    }
}

/// The user-visible strings the site layer renders. One field per message, so a
/// locale that omits one fails to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Messages {
    /// `view/layout/layout.phtml`: the footer line.
    pub powered_by: &'static str,
    /// `view/omeka/site/index/index.phtml`: a site with no pages.
    pub no_pages: &'static str,
    /// `common/site-page-pagination.phtml`.
    pub prev: &'static str,
    /// `common/site-page-pagination.phtml`.
    pub next: &'static str,
    /// `view/omeka/site/item/browse.phtml`: the search link.
    pub advanced_search: &'static str,
    /// `view/omeka/site/item/show.phtml`: the resource kind heading.
    pub item: &'static str,
    /// `view/omeka/site/item-set/browse.phtml`: the kind heading.
    pub item_set: &'static str,
    /// `view/omeka/site/media/show.phtml`: the kind heading.
    pub media: &'static str,
    /// `item-with-metadata.phtml` and the item-set view: the item-set list.
    pub item_sets: &'static str,
    /// `BlockLayout/ItemWithMetadata.php::render`: a block with no attachments.
    pub no_item_selected: &'static str,
    /// `media/show.phtml`'s `sprintf($translate('Part of %s'), ...)` prefix.
    pub part_of: &'static str,
    /// `block-layout/page-date-time.phtml`.
    pub created: &'static str,
    /// `block-layout/page-date-time.phtml`.
    pub modified: &'static str,
}

/// The English catalogue, the source language.
pub static EN: Messages = Messages {
    powered_by: "Powered by Romeka S",
    no_pages: "This site has no pages.",
    prev: "Prev",
    next: "Next",
    advanced_search: "Advanced search",
    item: "Item",
    item_set: "Item set",
    media: "Media",
    item_sets: "Item sets",
    no_item_selected: "No item selected",
    part_of: "Part of",
    created: "Created",
    modified: "Modified",
};

/// The Spanish catalogue. Terms match `application/language/es.po` where the
/// message id is the same.
pub static ES: Messages = Messages {
    powered_by: "Funciona con Romeka S",
    no_pages: "Este sitio no tiene páginas.",
    prev: "Ant",
    next: "Siguiente",
    advanced_search: "Búsqueda avanzada",
    item: "Ficha",
    item_set: "Conjunto de fichas",
    media: "Medios",
    item_sets: "Conjuntos de fichas",
    no_item_selected: "Ninguna ficha seleccionada",
    part_of: "Parte de",
    created: "Creado",
    modified: "Modificado",
};

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

/// `sprintf($pattern, $value)` for the one-argument patterns the search views
/// print, e.g. `Search results for “%s”`.
#[must_use]
pub fn sprintf1(pattern: &str, value: &str) -> String {
    pattern.replacen("%s", value, 1)
}

/// One locale's date names and patterns. Indexed by month (one-based) and
/// weekday (Sunday is zero).
struct DateNames {
    months: [&'static str; 12],
    months_short: [&'static str; 12],
    weekdays: [&'static str; 7],
    short: &'static str,
    medium: &'static str,
    long: &'static str,
    full: &'static str,
}

/// The English fallback branch: `I18n::dateFormat`'s patterns and the language's
/// own names.
static EN_NAMES: DateNames = DateNames {
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    months_short: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    weekdays: [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ],
    short: "n/j/y",
    medium: "M j, Y",
    long: "F j, Y",
    full: "l, F j, Y",
};

/// The Spanish names and ICU `es` patterns, the branch `intl` would format in.
static ES_NAMES: DateNames = DateNames {
    months: [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
    months_short: [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
    weekdays: [
        "domingo",
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
    ],
    short: "j/n/y",
    medium: "j M Y",
    long: "j \\d\\e F \\d\\e Y",
    full: "l, j \\d\\e F \\d\\e Y",
};

/// A locale and its catalogue, the value the theme and the blocks render with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Translator {
    locale: Locale,
}

impl Translator {
    /// A translator for one locale.
    #[must_use]
    pub const fn new(locale: Locale) -> Self {
        Self { locale }
    }

    /// The locale in force.
    #[must_use]
    pub const fn locale(self) -> Locale {
        self.locale
    }

    /// The catalogue for this locale.
    #[must_use]
    pub const fn messages(self) -> &'static Messages {
        self.locale.messages()
    }

    /// The search labels for this locale.
    #[must_use]
    pub const fn search_labels(self) -> &'static SearchLabels {
        self.locale.search()
    }

    /// `I18n::dateFormat` for this locale: the `date_format` and `time_format`
    /// a `pageDateTime` block stores, applied to a naive timestamp.
    ///
    /// `full` asks for a timezone abbreviation (`T`) the naive timestamp cannot
    /// supply, so the port prints nothing there, as the block module documents.
    #[must_use]
    pub fn format_datetime(
        self,
        value: NaiveDateTime,
        date_format: &str,
        time_format: &str,
    ) -> String {
        let names = self.locale.names();
        let date = match date_format {
            "none" => "",
            "short" => names.short,
            "long" => names.long,
            "full" => names.full,
            _ => names.medium,
        };
        let time = match time_format {
            "full" => "g:i:sa T",
            "long" => "g:i:sa",
            "medium" | "short" => "g:ia",
            _ => "",
        };
        let pattern = format!("{date} {time}");
        format_php_date(value, pattern.trim(), names)
    }
}

/// Apply the PHP date characters the formatter uses: `l F j Y n y M g i s a T`,
/// with a backslash escaping the next character. `T` has no value here and
/// prints nothing.
fn format_php_date(value: NaiveDateTime, pattern: &str, names: &DateNames) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(literal) = chars.next() {
                    out.push(literal);
                }
            }
            'l' => out.push_str(weekday_name(value, names)),
            'F' => out.push_str(month_name(value, names)),
            'M' => out.push_str(month_short_name(value, names)),
            'j' => out.push_str(trim_zero(&value.format("%d").to_string())),
            'Y' => out.push_str(&value.format("%Y").to_string()),
            'n' => out.push_str(trim_zero(&value.format("%m").to_string())),
            'y' => out.push_str(&value.format("%y").to_string()),
            'g' => out.push_str(trim_zero(&value.format("%I").to_string())),
            'i' => out.push_str(&value.format("%M").to_string()),
            's' => out.push_str(&value.format("%S").to_string()),
            'a' => out.push_str(&value.format("%p").to_string().to_ascii_lowercase()),
            'T' => {}
            other => out.push(other),
        }
    }
    out
}

fn weekday_name(value: NaiveDateTime, names: &DateNames) -> &'static str {
    let index = usize::try_from(value.weekday().num_days_from_sunday()).unwrap_or(0);
    names.weekdays.get(index).copied().unwrap_or("")
}

fn month_name(value: NaiveDateTime, names: &DateNames) -> &'static str {
    let index = usize::try_from(value.month0()).unwrap_or(0);
    names.months.get(index).copied().unwrap_or("")
}

fn month_short_name(value: NaiveDateTime, names: &DateNames) -> &'static str {
    let index = usize::try_from(value.month0()).unwrap_or(0);
    names.months_short.get(index).copied().unwrap_or("")
}

fn trim_zero(value: &str) -> &str {
    value.strip_prefix('0').unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-10-16 09:30:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    /// A stored tag narrows to a locale, and a region tag to its language.
    #[test]
    fn locale_parse_accepts_a_language_and_a_region() {
        assert_eq!(Locale::parse("en_US"), Locale::En);
        assert_eq!(Locale::parse("es"), Locale::Es);
        assert_eq!(Locale::parse("es_419"), Locale::Es);
        assert_eq!(Locale::parse("de_DE"), Locale::En);
        assert_eq!(Locale::parse(""), Locale::En);
        assert_eq!(Locale::default(), Locale::En);
    }

    /// The tag round-trips through parse.
    #[test]
    fn locale_tags_round_trip() {
        for locale in [Locale::En, Locale::Es] {
            assert_eq!(Locale::parse(locale.tag()), locale);
        }
    }

    /// English reproduces `I18n::dateFormat`'s fallback patterns.
    #[test]
    fn translator_formats_dates_like_upstream_i18n() {
        let tr = Locale::En.translator();
        let value = stamp();
        assert_eq!(tr.format_datetime(value, "short", "none"), "10/16/26");
        assert_eq!(tr.format_datetime(value, "medium", "none"), "Oct 16, 2026");
        assert_eq!(
            tr.format_datetime(value, "long", "none"),
            "October 16, 2026"
        );
        assert_eq!(
            tr.format_datetime(value, "full", "none"),
            "Friday, October 16, 2026"
        );
        assert_eq!(tr.format_datetime(value, "none", "short"), "9:30am");
        assert_eq!(tr.format_datetime(value, "none", "long"), "9:30:00am");
        assert_eq!(tr.format_datetime(value, "none", "none"), "");
    }

    /// Spanish renders the same timestamp with Spanish names and ICU `es`
    /// patterns: `16 de octubre de 2026`.
    #[test]
    fn translator_formats_dates_in_the_locale() {
        let tr = Locale::Es.translator();
        let value = stamp();
        assert_eq!(tr.format_datetime(value, "short", "none"), "16/10/26");
        assert_eq!(tr.format_datetime(value, "medium", "none"), "16 oct 2026");
        assert_eq!(
            tr.format_datetime(value, "long", "none"),
            "16 de octubre de 2026"
        );
        assert_eq!(
            tr.format_datetime(value, "full", "none"),
            "viernes, 16 de octubre de 2026"
        );
        assert_eq!(tr.format_datetime(value, "none", "short"), "9:30am");
    }

    /// Every catalogue names every message, and the two differ.
    #[test]
    fn messages_are_present_in_both_locales() {
        assert_eq!(
            Locale::En.translator().messages().powered_by,
            "Powered by Romeka S"
        );
        assert_eq!(
            Locale::Es.translator().messages().powered_by,
            "Funciona con Romeka S"
        );
        assert_ne!(
            Locale::En.translator().messages().item_sets,
            Locale::Es.translator().messages().item_sets
        );
        assert_eq!(Locale::Es.translator().locale(), Locale::Es);
    }

    /// The search catalogue differs per locale where `es.po` has the term, and
    /// every comparison carries a value and a label.
    #[test]
    fn search_labels_follow_the_spanish_catalogue() {
        let en = Locale::En.translator().search_labels();
        let es = Locale::Es.translator().search_labels();
        assert_eq!(en.search, "Search");
        assert_eq!(es.search, "Búsqueda");
        assert_eq!(en.search_full_text, "Search full-text");
        assert_eq!(es.search_full_text, "Buscar en cualquier campo");
        // The `type` select's values are the wire names `buildPropertyQuery`
        // reads, so they stay English in both catalogues.
        assert_eq!(en.comparisons[0], ("eq", "is exactly"));
        assert_eq!(es.comparisons[0], ("eq", "es exactamente"));
        assert_eq!(en.comparisons[13], ("ndt", "does not have data type"));
        assert_eq!(es.comparisons[13], ("ndt", "does not have data type"));
        for (value, label) in en.comparisons {
            assert!(!value.is_empty() && !label.is_empty(), "{value}");
        }
        assert_eq!(en.sort_fields[0], ("title", "Title"));
        assert_eq!(es.sort_fields[0], ("title", "Título"));
        assert_eq!(es.sort_fields[1], ("created", "Creado"));
        assert_eq!(
            es.form_title(crate::search::SearchResource::ItemSets),
            "Búsqueda avanzada de conjunto de fichas"
        );
        assert_eq!(
            es.form_title(crate::search::SearchResource::Items),
            "Búsqueda avanzada de fichas"
        );
    }

    /// `sprintf($pattern, $value)` substitutes one argument.
    #[test]
    fn sprintf1_fills_the_first_placeholder() {
        assert_eq!(
            sprintf1("Search results for “%s”", "cat"),
            "Search results for “cat”"
        );
        assert_eq!(
            sprintf1("View all results (%s total)", "4"),
            "View all results (4 total)"
        );
        assert_eq!(sprintf1("No placeholder", "cat"), "No placeholder");
    }
}
