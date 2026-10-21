//! The public site layer: slugs and routes, navigation links, the twenty
//! upstream block layouts, and the one bundled theme that renders a site page
//! as HTML.
//!
//! Upstream reference: `source/omeka-s/application` (read only). Each module
//! cites the files it ports:
//!
//! - [`slug`]: the public route table (`config/routes.config.php`) plus the
//!   site and page slug rules, which live in `omeka_core::site`.
//! - [`nav`]: `Site/Navigation/Link/*` and `Site/Navigation/Translator.php`.
//! - [`blocks`]: one module per layout in `Site/BlockLayout/*.php`, ported from
//!   the class plus its view script in `view/common/block-layout/*.phtml`.
//! - [`search`]: the three resource names a search covers, the results pages and
//!   the advanced form's fields (`Controller/Site/IndexController.php`,
//!   `CrossSiteSearchController.php`, `view/omeka/site/index/search.phtml`,
//!   `view/common/advanced-search.phtml`).
//! - [`theme`]: the bundled theme, ported from `view/layout/layout.phtml` and
//!   `view/omeka/site/*` (the site views core ships when a theme overrides
//!   nothing), behind a [`theme::Theme`] trait.
//! - [`i18n`]: the site locale from `site_setting.locale`, the message
//!   catalogue the theme and the blocks render, and locale-aware dates.
//! - [`render`]: renders a site page into a full document.
//! - [`iiif_viewer`]: the IIIF viewer page (`view/omeka/iiif-viewer/index.phtml`).
//!
//! The crate holds no database, no HTTP and no async. Everything it needs from
//! storage arrives through [`view::RenderContext`], so a caller can test a page
//! render with plain values. Validation sits at the two boundaries that reach
//! this crate: navigation JSON ([`nav::parse_navigation`]) and block `data`
//! JSON ([`blocks::parse::parse_block_data`]). Inside, the types are trusted.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod annotation;
pub mod blocks;
pub mod html;
pub mod i18n;
pub mod iiif_viewer;
pub mod map;
pub mod model_reader;
pub mod nav;
pub mod render;
pub mod resource_page;
pub mod resource_page_schema;
pub mod search;
pub mod settings;
pub mod share;
pub mod slug;
pub mod theme;
pub mod transcript;
pub mod view;
