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
    /// The user bar's link to the page editor. Upstream labels every edit
    /// link `Edit`; the port names what opens.
    pub edit_page: &'static str,
    /// The user bar's link to the item's admin page.
    pub edit_item: &'static str,
    /// The user bar's link to the item set's admin page.
    pub edit_item_set: &'static str,
    /// The user bar's link to the media's admin page.
    pub edit_media: &'static str,
    /// `resource-page-block-layout/resource-class.phtml`: the label.
    pub resource_class: &'static str,
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
    /// The browse pager's summary, `%s results · page %s of %s` over the
    /// total, the page and the page count. Upstream's pager has no summary.
    pub pager_count: &'static str,
    /// The note under a resource heading whose title is a fallback.
    pub no_title_recorded: &'static str,
    /// The panel an item page shows when the item has no media.
    pub no_media_attached: &'static str,
    /// The footer's link list, as its `nav` label.
    pub footer_links: &'static str,
    /// The footer's licence line when the site keeps every right.
    pub all_rights_reserved: &'static str,
    /// The header search field's placeholder.
    pub search_placeholder: &'static str,
    /// A IIIF block in the editor's script-free preview: the `IIIF viewer: %s.
    /// It opens on the public page.` pattern, over the block's title. A Romeka
    /// string; upstream's preview runs the viewer.
    pub iiif_poster: &'static str,
    /// The item page's link that opens the item in the IIIF viewer. A Romeka
    /// string.
    pub iiif_view_in_viewer: &'static str,
    /// The item page's link to the item's IIIF manifest. A Romeka string.
    pub iiif_manifest: &'static str,
    /// A browse preview of items on a site that holds no items, in the admin
    /// preview. A port addition: upstream prints an empty list.
    pub no_site_items_preview: &'static str,
    /// The same empty browse preview on the public site, with no admin hint.
    pub no_site_items: &'static str,
    /// A map's accessible name, and the item page's map section label. A
    /// Romeka addition, as every `map_` message is.
    pub map: &'static str,
    /// The disclosure under a map block that lists its places as links.
    pub map_places: &'static str,
    /// A marker that stands for several close places, `%s` the count.
    pub map_place_count: &'static str,
    /// The editor preview's poster for a map block, `%s` the place count.
    pub map_poster: &'static str,
    /// The editor preview's poster for a map block with no places.
    pub map_no_places_preview: &'static str,
    /// A gallery's accessible name. A Romeka addition, as every `gallery_`
    /// message is.
    pub gallery: &'static str,
    /// The lightbox's close button.
    pub gallery_close: &'static str,
    /// The lightbox's button to the image before.
    pub gallery_previous: &'static str,
    /// The lightbox's button to the image after.
    pub gallery_next: &'static str,
    /// The lightbox's position line, `{index}` the image's number and `{count}`
    /// the gallery's image count, named so a locale can put them in its own
    /// order.
    pub gallery_position: &'static str,
    /// The editor preview of a gallery with no images.
    pub gallery_empty_preview: &'static str,
    /// A featured item card's link to the item. A Romeka addition, as every
    /// `featured_` message is.
    pub featured_view_item: &'static str,
    /// The editor preview of a featured item card with no item to show.
    pub featured_empty_preview: &'static str,
    /// The editor preview of a button block with no complete button. A Romeka
    /// addition.
    pub button_empty_preview: &'static str,
    /// The editor preview of a quote block with no quotation. A Romeka
    /// addition.
    pub quote_empty_preview: &'static str,
    /// The editor preview of an accordion with no entry. A Romeka addition.
    pub accordion_empty_preview: &'static str,
    /// The heading of a timeline's items with no date. A Romeka addition, as
    /// every `timeline_` message is.
    pub timeline_undated: &'static str,
    /// The editor preview of a timeline with no item.
    pub timeline_empty_preview: &'static str,
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
    edit_page: "Edit this page",
    edit_item: "Edit item",
    edit_item_set: "Edit item set",
    edit_media: "Edit media",
    resource_class: "Resource class",
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
    pager_count: "%s results · page %s of %s",
    no_title_recorded: "No title recorded",
    no_media_attached: "No media attached",
    footer_links: "Footer links",
    all_rights_reserved: "All rights reserved",
    search_placeholder: "Search the collection",
    iiif_poster: "IIIF viewer: %s. It opens on the public page.",
    iiif_view_in_viewer: "View in the IIIF viewer",
    iiif_manifest: "IIIF manifest",
    no_site_items_preview: "No items on this site yet. Add items on the Items tab.",
    no_site_items: "There are no items to show yet.",
    map: "Map",
    map_places: "Places on this map",
    map_place_count: "%s places",
    map_poster: "A map of %s places. The map itself shows on the public page.",
    map_no_places_preview: "This map has no places yet. Its items need a \"lat, lng\" value in the \
                            location field the site settings name.",
    gallery: "Gallery",
    gallery_close: "Close",
    gallery_previous: "Previous image",
    gallery_next: "Next image",
    gallery_position: "Image {index} of {count}",
    gallery_empty_preview: "This gallery has no images yet. Pick items or assets in the block's \
                            settings.",
    featured_view_item: "View item",
    featured_empty_preview: "Pick an item for this card in the block's settings.",
    button_empty_preview: "Give the button a label and a page or a URL in the block's settings.",
    quote_empty_preview: "Write the quotation in the block's settings.",
    accordion_empty_preview: "Add a heading and its answer in the block's settings.",
    timeline_undated: "Undated",
    timeline_empty_preview: "Pick items or a search for this timeline in the block's settings.",
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
    // "Edit this page" is not in `es.po`; the other edit labels are.
    edit_page: "Editar esta página",
    edit_item: "Editar ficha",
    edit_item_set: "Editar conjunto de fichas",
    edit_media: "Editar medios",
    resource_class: "Clase de recurso",
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
    // The theme's own strings, not in `es.po`.
    pager_count: "%s resultados · página %s de %s",
    no_title_recorded: "Sin título registrado",
    no_media_attached: "Sin medios adjuntos",
    footer_links: "Enlaces del pie de página",
    all_rights_reserved: "Todos los derechos reservados",
    search_placeholder: "Buscar en la colección",
    iiif_poster: "Visor IIIF: %s. Se abre en la página pública.",
    iiif_view_in_viewer: "Ver en el visor IIIF",
    iiif_manifest: "Manifiesto IIIF",
    // The admin's words, because only the admin preview shows it.
    no_site_items_preview: "Este sitio aún no tiene elementos. Añade elementos en la pestaña Elementos.",
    no_site_items: "Todavía no hay fichas que mostrar.",
    map: "Mapa",
    map_places: "Lugares de este mapa",
    map_place_count: "%s lugares",
    map_poster: "Un mapa de %s lugares. El mapa se muestra en la página pública.",
    map_no_places_preview: "Este mapa aún no tiene lugares. Sus fichas necesitan un valor \
                            \"lat, lng\" en el campo de ubicación que indican los ajustes del sitio.",
    gallery: "Galería",
    gallery_close: "Cerrar",
    gallery_previous: "Imagen anterior",
    gallery_next: "Imagen siguiente",
    gallery_position: "Imagen {index} de {count}",
    gallery_empty_preview: "Esta galería aún no tiene imágenes. Elige fichas o recursos en los \
                            ajustes del bloque.",
    featured_view_item: "Ver ficha",
    featured_empty_preview: "Elige una ficha para esta tarjeta en los ajustes del bloque.",
    button_empty_preview: "Da al botón un texto y una página o una URL en los ajustes del bloque.",
    quote_empty_preview: "Escribe la cita en los ajustes del bloque.",
    accordion_empty_preview: "Añade un encabezado y su respuesta en los ajustes del bloque.",
    timeline_undated: "Sin fecha",
    timeline_empty_preview: "Elige fichas o una búsqueda para esta cronología en los ajustes del bloque.",
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
