//! The message catalogue the theme, the blocks and the navigation render: one
//! field per upstream message id, one static per locale.

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
    /// `view/omeka/site/item/browse.phtml`: the heading, and the
    /// `linked-resources.phtml` label for linking items.
    pub items: &'static str,
    /// `item-with-metadata.phtml` and the item-set view: the item-set list.
    pub item_sets: &'static str,
    /// `BlockLayout/ItemWithMetadata.php::render`: a block with no attachments.
    pub no_item_selected: &'static str,
    /// `media/show.phtml`'s `sprintf($translate('Part of %s'), ...)` pattern,
    /// over the parent item's link. A pattern, not a prefix, so a language
    /// that puts the item first can.
    pub part_of: &'static str,
    /// `block-layout/page-date-time.phtml`.
    pub created: &'static str,
    /// `block-layout/page-date-time.phtml`.
    pub modified: &'static str,
    /// `common/user-bar.phtml`: the bar's aria label.
    pub user_bar_label: &'static str,
    /// `common/user-bar.phtml`: the `Signed in as %s` pattern, over the user's
    /// name.
    pub signed_in_as: &'static str,
    /// `common/user-bar.phtml`: the logout link.
    pub log_out: &'static str,
    /// `common/user-bar.phtml`: the login link.
    pub log_in: &'static str,
    /// The user bar's link to the admin. Upstream labels it with the
    /// installation title; the port names the place, `Admin dashboard`.
    pub admin_dashboard: &'static str,
    /// `view/layout/layout.phtml`: the skip link.
    pub skip_to_main: &'static str,
    /// The theme header's menu toggle and its `nav` label. Upstream's
    /// application catalogue has no such message id.
    pub menu: &'static str,
    /// `Site/Navigation/Translator.php:60`: the link an empty navigation shows.
    pub home: &'static str,
    /// `Navigation/Link/Browse.php::getName`: an unlabelled browse link.
    pub browse: &'static str,
    /// `Navigation/Link/BrowseItemSets.php::getName`.
    pub browse_item_sets: &'static str,
    /// `Navigation/Link/Url.php::getName`: an unlabelled URL link.
    pub custom_url: &'static str,
    /// `Navigation/Link/Page.php::getLabel` and `Fallback.php::getName`: a
    /// link to a page the site does not hold.
    pub missing_page: &'static str,
    /// `AbstractResourceEntityRepresentation::displayTitle`: a resource with no
    /// title.
    pub untitled: &'static str,
    /// `resource-page-block-layout/site-pages.phtml`: the section label.
    pub site_pages: &'static str,
    /// `common/linked-resources.phtml`: the first column heading.
    pub title: &'static str,
    /// `common/linked-resources.phtml`: the second column heading.
    pub class: &'static str,
    /// `common/linked-resources.phtml`: the caption, `%s with "%s: %s"` over
    /// the resource label, the property label and the resource title.
    pub linked_caption: &'static str,
    /// `view/error/404.phtml`: the heading.
    pub page_not_found: &'static str,
    /// `view/error/404.phtml`: the paragraph.
    pub page_not_found_message: &'static str,
    /// `view/error/403.phtml`: the heading.
    pub forbidden: &'static str,
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
    items: "Items",
    item_sets: "Item sets",
    no_item_selected: "No item selected",
    part_of: "Part of %s",
    created: "Created",
    modified: "Modified",
    user_bar_label: "Omeka User",
    signed_in_as: "Signed in as %s",
    log_out: "Logout",
    log_in: "Log in",
    admin_dashboard: "Admin dashboard",
    skip_to_main: "Skip to main content",
    menu: "Menu",
    home: "Home",
    browse: "Browse",
    browse_item_sets: "Browse item sets",
    custom_url: "Custom URL",
    missing_page: "[Missing Page]",
    untitled: "[Untitled]",
    site_pages: "Site pages",
    title: "Title",
    class: "Class",
    linked_caption: "%s with \"%s: %s\"",
    page_not_found: "Page not found",
    page_not_found_message: "The page you are trying to view does not exist. \
                             Use the back button to return to the last page.",
    forbidden: "Forbidden",
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
    items: "Fichas",
    item_sets: "Conjuntos de fichas",
    no_item_selected: "Ninguna ficha seleccionada",
    part_of: "Parte de %s",
    created: "Creado",
    modified: "Modificado",
    user_bar_label: "Usuario de Omeka",
    signed_in_as: "Sesión iniciada como %s",
    log_out: "Cerrar sesión",
    log_in: "Iniciar sesión",
    admin_dashboard: "Panel de administración",
    skip_to_main: "Saltar al contenido principal",
    // Not in `es.po`; the port's own translation.
    menu: "Menú",
    home: "Inicio",
    browse: "Explorar",
    browse_item_sets: "Explorar conjuntos de fichas",
    custom_url: "URL personalizada",
    // `es.po:447`: upstream says "lost page" ("perdida"), not "missing".
    missing_page: "[Página perdida]",
    untitled: "[Sin título]",
    site_pages: "Páginas del sitio",
    title: "Título",
    class: "Clase",
    linked_caption: "%s con \"%s: %s\"",
    page_not_found: "Página no encontrada",
    // `es.po:1900-1905` rephrases the message rather than translating it word
    // for word.
    page_not_found_message: "La página solicitada no existe. El botón de volver \
                             atrás regresará a la última página.",
    forbidden: "Prohibido",
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback strings this unit moved into the catalogue carry upstream's
    /// English and `es.po`'s Spanish.
    #[test]
    fn fallback_strings_follow_upstream_and_es_po() {
        let pairs = [
            (EN.home, ES.home, "Home", "Inicio"),
            (EN.browse, ES.browse, "Browse", "Explorar"),
            (
                EN.browse_item_sets,
                ES.browse_item_sets,
                "Browse item sets",
                "Explorar conjuntos de fichas",
            ),
            (
                EN.missing_page,
                ES.missing_page,
                "[Missing Page]",
                "[Página perdida]",
            ),
            (EN.untitled, ES.untitled, "[Untitled]", "[Sin título]"),
            (
                EN.page_not_found,
                ES.page_not_found,
                "Page not found",
                "Página no encontrada",
            ),
            (EN.forbidden, ES.forbidden, "Forbidden", "Prohibido"),
        ];
        for (en, es, want_en, want_es) in pairs {
            assert_eq!((en, es), (want_en, want_es));
        }
        assert_eq!(
            EN.page_not_found_message,
            "The page you are trying to view does not exist. Use the back button to return to the last page."
        );
        assert_eq!(
            ES.page_not_found_message,
            "La página solicitada no existe. El botón de volver atrás regresará a la última página."
        );
    }
}
