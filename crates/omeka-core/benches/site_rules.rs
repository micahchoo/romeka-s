//! Micro-benches for the slug and template rules.
//!
//! Covers `crates/omeka-core/src/site.rs` (the slug rule from
//! `SiteSlugTrait`, the block layout names and the block data parse) and
//! `crates/omeka-core/src/template.rs` (data type normalisation, the required
//! property check and the unknown data type report).
//!
//! Run with `cargo bench -p omeka-core --bench site_rules`. The numbers, their
//! limiters and the checks that did not apply are in `crates/omeka-core/README.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use omeka_core::datatypes::Registry;
use omeka_core::ids::{PageId, PropertyId, SiteId, TemplateId};
use omeka_core::site::{BlockData, BlockLayout, Slug, is_valid_slug, slugify};
use omeka_core::template::{ResourceTemplate, TemplateProp, normalize_data_types};

/// A page title that slugifies to a plain dashed string.
const TITLE: &str = "About the Omeka S Port";

/// A longer body of text, with punctuation and runs of separators.
const BODY: &str = "Omeka S is a web publication system. It has many sites; \
each site has pages, and each page has blocks. Blocks are typed, and each \
block carries its own data object (a JSON object, stored in one column).";

fn long_slug() -> String {
    "a".repeat(200)
}

fn browse_preview_data() -> serde_json::Value {
    serde_json::json!({
        "resource_type": "items",
        "query": "dcterms:title:port",
        "heading": "Recent additions",
        "limit": "12",
        "components": ["resourceClass", "thumbnail", "title", "description"],
        "link-text": "Browse all",
    })
}

fn template() -> ResourceTemplate {
    let props = (1..=10)
        .map(|index| {
            let mut prop = TemplateProp::new(PropertyId::new(index))
                .with_alternate_label(format!("Label {index}"))
                .with_data_types(["literal", "uri", "literal", " "]);
            if index % 3 == 0 {
                prop = prop.required();
            }
            prop
        })
        .collect();
    ResourceTemplate::new(TemplateId::new(1), "Base Resource").with_props(props)
}

fn site_rules(c: &mut Criterion) {
    let registry = Registry::core();
    let long = long_slug();
    let template = template();
    let present = [PropertyId::new(3), PropertyId::new(6), PropertyId::new(2)];
    let browse_preview = browse_preview_data();
    let browse_preview_parsed =
        BlockData::parse(&BlockLayout::BrowsePreview, &browse_preview).unwrap();

    // Run once, outside the timed region: a case that started failing would
    // otherwise be timed as an error path.
    assert!(Slug::parse("about-the-port").is_ok());
    assert!(Slug::parse(&long).is_ok());
    assert!(Slug::parse("about the port").is_err());
    assert!(Slug::parse("").is_err());
    assert!(is_valid_slug("about-the-port") && !is_valid_slug("about the port"));
    assert_eq!(slugify(TITLE), "about-the-omeka-s-port");
    assert_eq!(slugify("Café — Über Ångström").matches('-').count(), 3);
    assert_eq!(
        BlockLayout::from_wire_name("browsePreview"),
        BlockLayout::BrowsePreview
    );
    assert!(matches!(
        BlockLayout::from_wire_name("notARealLayout"),
        BlockLayout::Fallback(_)
    ));
    assert_eq!(BlockLayout::Asset.label(), "Asset");
    assert_eq!(
        normalize_data_types(["literal", "uri", "literal", " ", ""]).len(),
        2
    );
    assert_eq!(template.missing_required_props(&present).len(), 1);
    assert_eq!(
        template
            .unknown_data_types(|name| registry.is_registered(name))
            .len(),
        0
    );

    let mut group = c.benchmark_group("core/site_rules");
    group
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);

    for (name, raw) in [
        ("valid", "about-the-port"),
        ("long", long.as_str()),
        ("invalid_character", "about the port"),
        ("empty", ""),
    ] {
        group.bench_function(format!("slug_parse/{name}"), |b| {
            b.iter(|| Slug::parse(black_box(raw)))
        });
    }
    group.bench_function("is_valid_slug/valid", |b| {
        b.iter(|| is_valid_slug(black_box("about-the-port")))
    });
    group.bench_function("is_valid_slug/invalid", |b| {
        b.iter(|| is_valid_slug(black_box("about the port")))
    });

    for (name, input) in [
        ("short_title", TITLE),
        ("long_body", BODY),
        ("non_ascii", "Café — Über Ångström"),
    ] {
        group.bench_function(format!("slugify/{name}"), |b| {
            b.iter(|| slugify(black_box(input)))
        });
    }

    for (name, layout) in [("known", "browsePreview"), ("fallback", "notARealLayout")] {
        group.bench_function(format!("block_layout_from_wire_name/{name}"), |b| {
            b.iter(|| BlockLayout::from_wire_name(black_box(layout)))
        });
    }
    group.bench_function("block_layout_label", |b| {
        b.iter(|| BlockLayout::Asset.label())
    });

    group.bench_function("block_data_parse/browse_preview", |b| {
        b.iter(|| {
            BlockData::parse(
                black_box(&BlockLayout::BrowsePreview),
                black_box(&browse_preview),
            )
        })
    });
    group.bench_function("block_data_to_json/browse_preview", |b| {
        b.iter(|| black_box(&browse_preview_parsed).to_json())
    });
    group.bench_function("block_data_parse/html", |b| {
        let data = serde_json::json!({ "html": "<p>A block of HTML.</p>" });
        b.iter(|| BlockData::parse(black_box(&BlockLayout::Html), black_box(&data)))
    });

    group.bench_function("template_normalize_data_types/with_duplicates", |b| {
        b.iter(|| normalize_data_types(black_box(vec!["literal", "uri", "literal", "   ", ""])))
    });
    group.bench_function("template_missing_required_props", |b| {
        b.iter(|| template.missing_required_props(black_box(&present)))
    });
    group.bench_function("template_unknown_data_types", |b| {
        b.iter(|| template.unknown_data_types(|name| registry.is_registered(black_box(name))))
    });

    let site_slug = Slug::parse("a-site").unwrap();
    group.bench_function("site_new", |b| {
        b.iter(|| {
            omeka_core::site::Site::new(SiteId::new(1), site_slug.clone(), "default", "A site")
        })
    });
    group.bench_function("site_page_new", |b| {
        b.iter(|| {
            omeka_core::site::SitePage::new(
                PageId::new(1),
                SiteId::new(1),
                site_slug.clone(),
                "A page",
            )
        })
    });

    group.finish();
}

criterion_group!(benches, site_rules);
criterion_main!(benches);
