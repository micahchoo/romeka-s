use super::*;
use crate::view::{NoResources, RenderContext, ResourceView};
use omeka_core::ids::ResourceId;
use omeka_core::resource::ResourceKind;
use omeka_core::site::Site;

fn site() -> Site {
    Site::new(
        omeka_core::ids::SiteId::new(1),
        omeka_core::site::Slug::parse("main").expect("a valid slug"),
        "default".to_string(),
        "Main",
    )
}

fn context<'a>(site: &'a Site, resources: &'a NoResources) -> RenderContext<'a> {
    RenderContext::new(site, resources)
}

fn resource(kind: ResourceKind) -> ResourceView {
    ResourceView::new(ResourceId::new(1), kind, "/s/main/item/1", "An item")
}

fn value() -> ValueRef {
    ValueRef {
        property_id: 1,
        property: "Title".to_string(),
        property_term: "dcterms:title".to_string(),
        vocabulary: "Dublin Core".to_string(),
        value: "An item".to_string(),
        html: "An item".to_string(),
        lang: None,
        class: "literal".to_string(),
    }
}

/// The default item blocks render media, values, item sets, site pages, the
/// media list and linked resources, in that order, and the empty lists
/// leave their blocks out.
#[test]
fn item_default_blocks_render_in_theme_order() {
    let site = site();
    let resources = NoResources;
    let ctx = context(&site, &resources);
    let mut item = resource(ResourceKind::Item);
    item.values.push(value());
    item.media.push(ResourceRef {
        url: "/s/main/media/9".to_string(),
        title: "A file".to_string(),
        thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
    });
    item.item_sets.push(ResourceRef {
        url: "/s/main/item-set/2".to_string(),
        title: "A set".to_string(),
        thumbnail: None,
    });
    let out = render_blocks(&item, &ctx);
    assert!(
        out.contains("<dl class=\"values\"><div class=\"property\"><dt>Title</dt>"),
        "{out}"
    );
    assert!(out.contains("<dl class=\"item-sets\">"), "{out}");
    assert!(out.contains("<dl class=\"media-list\">"), "{out}");
    assert!(!out.contains("<div class=\"media-embeds\">"), "{out}");
    let values_at = out.find("<dl class=\"values\">").expect("values");
    let sets_at = out.find("<dl class=\"item-sets\">").expect("item sets");
    let media_at = out.find("<dl class=\"media-list\">").expect("media list");
    assert!(values_at < sets_at && sets_at < media_at, "{out}");
}

/// The media page renders its own markup, then its values.
#[test]
fn media_default_blocks_render_the_file_then_values() {
    let site = site();
    let resources = NoResources;
    let ctx = context(&site, &resources);
    let mut media = resource(ResourceKind::Media);
    media.media_embeds.push(crate::view::MediaRef {
        url: "/s/main/media/1".to_string(),
        original_url: None,
        title: "A file".to_string(),
        thumbnail: None,
        embed: Some("<div class=\"media-render html\">A file</div>".to_string()),
    });
    media.values.push(value());
    assert_eq!(
        render_blocks(&media, &ctx),
        "<div class=\"media-render html\">A file</div>\
         <dl class=\"values\"><div class=\"property\"><dt>Title</dt>\
         <dd class=\"value literal\" lang=\"\"><span class=\"value-content\">An item</span></dd>\
         </div></dl>"
    );
}

/// The stored configuration wins, and a resource it omits renders nothing.
#[test]
fn stored_configuration_replaces_the_default() {
    let site = site();
    let resources = NoResources;
    let mut ctx = context(&site, &resources);
    ctx.resource_page_blocks = Some(ResourcePageBlocks::parse(&serde_json::json!({
        "items": {"main": ["values"]}
    })));
    let mut item = resource(ResourceKind::Item);
    item.values.push(value());
    assert_eq!(
        render_blocks(&item, &ctx),
        "<dl class=\"values\"><div class=\"property\"><dt>Title</dt>\
         <dd class=\"value literal\" lang=\"\"><span class=\"value-content\">An item</span></dd>\
         </div></dl>"
    );
    let other = resource(ResourceKind::Media);
    assert_eq!(render_blocks(&other, &ctx), "");
}

/// `property_label_information` adds the term or the vocabulary label, and
/// `show_locale_label` adds the language span.
#[test]
fn value_labels_follow_the_site_settings() {
    let site = site();
    let resources = NoResources;
    let mut ctx = context(&site, &resources);
    ctx.settings.property_label_information = Some("term".to_string());
    ctx.settings.show_locale_label = Some(true);
    let mut item = resource(ResourceKind::Item);
    let mut labelled = value();
    labelled.lang = Some("fr".to_string());
    item.values.push(labelled);
    let out = render_blocks(&item, &ctx);
    assert!(
        out.contains("<dt>Title<span class=\"field-term\">(dcterms:title)</span></dt>"),
        "{out}"
    );
    assert!(out.contains("lang=\"fr\""), "{out}");
    assert!(out.contains("<span class=\"language\">fr</span>"), "{out}");
}

/// A resource value is a pretty link and its class names the resource kind.
#[test]
fn resource_value_renders_a_pretty_link() {
    let site = site();
    let resources = NoResources;
    let ctx = context(&site, &resources);
    let mut item = resource(ResourceKind::Item);
    item.values.push(ValueRef {
        property_id: 2,
        property: "Subject".to_string(),
        property_term: "dcterms:subject".to_string(),
        vocabulary: "Dublin Core".to_string(),
        value: "Another item".to_string(),
        html: "<a href=\"/s/main/item/2\" class=\"resource-link\"><img src=\"/files/square/2.jpg\" alt=\"X\"><span class=\"resource-name\">Another item</span></a>".to_string(),
        lang: None,
        class: "resource items".to_string(),
    });
    let out = render_blocks(&item, &ctx);
    assert!(
        out.contains("<dd class=\"value resource items\" lang=\"\">"),
        "{out}"
    );
    assert!(out.contains("class=\"resource-link\""), "{out}");
}

/// An item with attached pages and a linking item, its labels in `locale`.
fn linked_item_in(locale: &str) -> String {
    let site = site();
    let resources = NoResources;
    let mut ctx = context(&site, &resources);
    ctx.settings.locale = Some(locale.to_string());
    let mut item = resource(ResourceKind::Item);
    item.site_pages.push(ResourceRef {
        url: "/s/main/page/about".to_string(),
        title: "About".to_string(),
        thumbnail: None,
    });
    item.linked_resources.push(LinkedResource {
        property: "Creator".to_string(),
        kind: ResourceKind::Item,
        resource: ResourceRef {
            url: "/s/main/item/3".to_string(),
            title: "A letter".to_string(),
            thumbnail: None,
        },
    });
    render_blocks(&item, &ctx)
}

/// `site-pages.phtml` and `linked-resources.phtml` print upstream's labels
/// in English and `es.po`'s in Spanish; the property label and the title
/// stay as stored.
#[test]
fn resource_page_labels_follow_the_site_locale() {
    let en = linked_item_in("en");
    assert!(en.contains("<dt>Site pages</dt>"), "{en}");
    assert!(
        en.contains("<caption class=\"linked-resource-property\">Items with &quot;Creator: An item&quot;</caption>"),
        "{en}"
    );
    assert!(en.contains("<th>Title</th><th>Class</th>"), "{en}");
    let es = linked_item_in("es");
    assert!(es.contains("<dt>Páginas del sitio</dt>"), "{es}");
    assert!(
        es.contains("<caption class=\"linked-resource-property\">Fichas con &quot;Creator: An item&quot;</caption>"),
        "{es}"
    );
    assert!(es.contains("<th>Título</th><th>Clase</th>"), "{es}");
}
