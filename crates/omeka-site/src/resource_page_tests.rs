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

/// The default item blocks render media, values, item sets, site pages and
/// linked resources, in that order. Upstream's separate media list is not a
/// default: the media block's strip carries every file, so a second list of
/// the same files would repeat it, and it would load every thumbnail eagerly.
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
        media_kind: None,
    });
    item.item_sets.push(ResourceRef {
        url: "/s/main/item-set/2".to_string(),
        title: "A set".to_string(),
        thumbnail: None,
        media_kind: None,
    });
    let out = render_blocks(&item, &ctx);
    assert!(
        out.contains("<div class=\"media-embeds media-empty\">"),
        "{out}"
    );
    assert!(
        out.contains("<dl class=\"values\"><div class=\"property\"><dt>Title</dt>"),
        "{out}"
    );
    assert!(out.contains("<dl class=\"item-sets\">"), "{out}");
    assert!(!out.contains("<dl class=\"media-list\">"), "{out}");
    let media_at = out
        .find("<div class=\"media-embeds media-empty\">")
        .expect("media");
    let values_at = out.find("<dl class=\"values\">").expect("values");
    let sets_at = out.find("<dl class=\"item-sets\">").expect("item sets");
    assert!(media_at < values_at && values_at < sets_at, "{out}");
    // The layout is still registered, so a site may configure it.
    assert!(
        render_configured(&item, &["mediaList"], "en").contains("<dl class=\"media-list\">"),
        "{out}"
    );
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
        media_type: None,
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
        media_kind: None,
    });
    item.linked_resources.push(LinkedResource {
        property: "Creator".to_string(),
        kind: ResourceKind::Item,
        resource: ResourceRef {
            url: "/s/main/item/3".to_string(),
            title: "A letter".to_string(),
            thumbnail: None,
            media_kind: None,
        },
        class_label: Some("Letter".to_string()),
    });
    item.linked_resources.push(LinkedResource {
        property: "Creator".to_string(),
        kind: ResourceKind::Item,
        resource: ResourceRef {
            url: "/s/main/item/4".to_string(),
            title: "A note".to_string(),
            thumbnail: None,
            media_kind: None,
        },
        class_label: None,
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
    assert!(
        en.contains(
            "A letter</span></a></td><td><span class=\"resource-class\">Letter</span></td>"
        ),
        "a linking resource's class fills the Class column: {en}"
    );
    assert!(
        en.contains("A note</span></a></td><td></td>"),
        "a linking resource with no class leaves the cell empty: {en}"
    );
    let es = linked_item_in("es");
    assert!(es.contains("<dt>Páginas del sitio</dt>"), "{es}");
    assert!(
        es.contains("<caption class=\"linked-resource-property\">Fichas con &quot;Creator: An item&quot;</caption>"),
        "{es}"
    );
    assert!(es.contains("<th>Título</th><th>Clase</th>"), "{es}");
}

/// Blocks configured for a resource kind, rendered in `locale`.
fn render_configured(resource: &ResourceView, blocks: &[&str], locale: &str) -> String {
    let site = site();
    let resources = NoResources;
    let mut ctx = context(&site, &resources);
    ctx.settings.locale = Some(locale.to_string());
    let key = match resource.kind {
        ResourceKind::Item => "items",
        ResourceKind::ItemSet => "item_sets",
        ResourceKind::Media => "media",
    };
    ctx.resource_page_blocks = Some(ResourcePageBlocks::parse(
        &serde_json::json!({ key: { "main": blocks } }),
    ));
    render_blocks(resource, &ctx)
}

/// `resource-class.phtml`: the class label under upstream's heading, and
/// nothing for a resource with no class.
#[test]
fn the_resource_class_block_prints_the_class_label() {
    for kind in [
        ResourceKind::Item,
        ResourceKind::ItemSet,
        ResourceKind::Media,
    ] {
        let mut classed = resource(kind);
        classed.class_label = Some("Photograph <b>".to_string());
        assert_eq!(
            render_configured(&classed, &["resourceClass"], "en"),
            "<dl class=\"resource-class\"><div class=\"property\"><dt>Resource class</dt>\
             <dd class=\"value\">Photograph &lt;b&gt;</dd></div></dl>",
            "{kind:?}"
        );
        assert!(
            render_configured(&classed, &["resourceClass"], "es")
                .contains("<dt>Clase de recurso</dt>"),
            "{kind:?}"
        );
        assert_eq!(
            render_configured(&resource(kind), &["resourceClass"], "en"),
            ""
        );
    }
}

/// One media file as the media blocks see it.
fn file(title: &str, media_type: Option<&str>, original: Option<&str>, alt: &str) -> MediaRef {
    MediaRef {
        url: format!("/s/main/media/{title}"),
        original_url: original.map(str::to_string),
        title: title.to_string(),
        media_type: media_type.map(str::to_string),
        thumbnail: original.map(|_| Thumbnail::new(format!("/files/square/{title}.jpg"), alt)),
        embed: Some(format!("<div class=\"media-render file\">{title}</div>")),
    }
}

/// `lightbox-gallery-item.phtml` without its script: each image file is its
/// thumbnail, linked to the full image. Files that are not images, or that
/// have no stored original, stay out of the grid.
#[test]
fn the_lightbox_gallery_links_each_image_to_its_full_file() {
    let mut item = resource(ResourceKind::Item);
    item.media_embeds = vec![
        file(
            "map",
            Some("image/png"),
            Some("/files/original/map.png"),
            "A map",
        ),
        file(
            "scan",
            Some("application/pdf"),
            Some("/files/original/scan.pdf"),
            "",
        ),
        file(
            "photo",
            Some("image/jpeg"),
            Some("/files/original/photo.jpg"),
            "",
        ),
        file("remote", Some("image/jpeg"), None, ""),
    ];
    assert_eq!(
        render_configured(&item, &["lightboxGallery"], "en"),
        "<div class=\"lightbox-gallery\">\
         <a href=\"/files/original/map.png\" class=\"resource-link\">\
         <img src=\"/files/square/map.jpg\" alt=\"A map\"></a>\
         <a href=\"/files/original/photo.jpg\" class=\"resource-link\">\
         <img src=\"/files/square/photo.jpg\" alt=\"photo\"></a>\
         </div>"
    );

    let mut bare = resource(ResourceKind::Item);
    bare.media_embeds = vec![file(
        "scan",
        Some("application/pdf"),
        Some("/files/original/scan.pdf"),
        "",
    )];
    assert_eq!(render_configured(&bare, &["lightboxGallery"], "en"), "");
}

/// `lightbox-gallery-media.phtml`: an image media page shows its image in
/// the gallery; any other file falls back to `media-render.phtml`.
#[test]
fn the_lightbox_gallery_on_a_media_page_falls_back_to_the_media_render() {
    let mut image = resource(ResourceKind::Media);
    image.media_embeds = vec![file(
        "map",
        Some("image/png"),
        Some("/files/original/map.png"),
        "A map",
    )];
    assert!(
        render_configured(&image, &["lightboxGallery"], "en")
            .starts_with("<div class=\"lightbox-gallery\"><a href=\"/files/original/map.png\""),
    );
    let mut pdf = resource(ResourceKind::Media);
    pdf.media_embeds = vec![file(
        "scan",
        Some("application/pdf"),
        Some("/files/original/scan.pdf"),
        "",
    )];
    assert_eq!(
        render_configured(&pdf, &["lightboxGallery"], "en"),
        "<div class=\"media-render file\">scan</div>"
    );
}

/// `LightboxGallery::getCompatibleResourceNames` names items and media; the
/// admin never offers it for an item set.
#[test]
fn the_lightbox_gallery_is_offered_for_items_and_media_only() {
    let spec = RESOURCE_PAGE_BLOCKS
        .iter()
        .find(|spec| spec.name == "lightboxGallery")
        .expect("the layout is registered");
    assert_eq!(spec.resources, &[ResourceKind::Item, ResourceKind::Media]);
}

/// The item page stages the first file and lists the files after it as a strip:
/// each one a link to its media page, named, and an image that waits until it
/// scrolls into view. The strip says how many files the item holds.
#[test]
fn the_item_page_stages_the_first_file_and_strips_the_rest() {
    let mut item = resource(ResourceKind::Item);
    item.media_embeds = vec![
        file(
            "first",
            Some("image/jpeg"),
            Some("/files/original/first.jpg"),
            "First",
        ),
        file(
            "second",
            Some("image/png"),
            Some("/files/original/second.png"),
            "Second",
        ),
        file("third", Some("application/pdf"), None, "Third"),
    ];
    item.media = vec![
        ResourceRef {
            url: "/s/main/media/1".to_string(),
            title: "first".to_string(),
            thumbnail: Some(Thumbnail::new("/files/square/first.jpg", "First")),
            media_kind: None,
        },
        ResourceRef {
            url: "/s/main/media/2".to_string(),
            title: "second".to_string(),
            thumbnail: Some(Thumbnail::new("/files/square/second.jpg", "Second")),
            media_kind: None,
        },
        ResourceRef {
            url: "/s/main/media/3".to_string(),
            title: "third".to_string(),
            thumbnail: None,
            media_kind: Some(crate::view::MediaKindCue {
                kind: "pdf",
                label: "PDF",
                icon: crate::view::MediaIcon::Pdf,
            }),
        },
    ];
    let out = render_configured(&item, &["mediaEmbeds"], "en");
    assert_eq!(
        out.matches("<div class=\"media-render file\">").count(),
        1,
        "one file renders whole: {out}"
    );
    assert!(
        out.contains("<div class=\"media-render file\">first</div>"),
        "the first file is the one shown whole: {out}"
    );
    assert!(
        !out.contains("<div class=\"media-render file\">second</div>"),
        "the other files appear only as strip entries: {out}"
    );
    assert!(
        out.contains("<p class=\"media-files-title\">3 files</p>"),
        "{out}"
    );
    assert_eq!(out.matches("<li class=\"media-file\">").count(), 2, "{out}");
    assert!(
        out.contains(
            "<a class=\"media-file-link\" href=\"/s/main/media/2\"><span class=\"media-file-thumb\">\
             <img src=\"/files/square/second.jpg\" alt=\"\" loading=\"lazy\" width=\"200\" \
             height=\"200\"></span><span class=\"media-file-name\">second</span></a>"
        ),
        "{out}"
    );
    assert!(
        out.contains(
            "<a class=\"media-file-link\" href=\"/s/main/media/3\">\
             <span class=\"media-file-thumb media-file-placeholder\">"
        ),
        "a file with no thumbnail gets a kind placeholder: {out}"
    );
    assert!(
        out.contains("<span class=\"resource-kind\">PDF</span>"),
        "the placeholder names the file's kind: {out}"
    );
    assert!(
        out.contains("<span class=\"media-file-name\">third</span>"),
        "each strip entry is named: {out}"
    );
    assert!(
        out.contains("<div class=\"annotation-list\"></div>"),
        "the notes list has its place after the first file: {out}"
    );
}

/// An item with one file has nothing to strip, and says no file count.
#[test]
fn the_item_page_shows_no_strip_for_one_file() {
    let mut item = resource(ResourceKind::Item);
    item.media_embeds = vec![file(
        "only",
        Some("image/jpeg"),
        Some("/files/original/only.jpg"),
        "Only",
    )];
    item.media = vec![ResourceRef {
        url: "/s/main/media/1".to_string(),
        title: "only".to_string(),
        thumbnail: Some(Thumbnail::new("/files/square/only.jpg", "Only")),
        media_kind: None,
    }];
    let out = render_configured(&item, &["mediaEmbeds"], "en");
    assert!(
        out.contains("<div class=\"media-render file\">only</div>"),
        "{out}"
    );
    assert!(!out.contains("media-files"), "{out}");
}
