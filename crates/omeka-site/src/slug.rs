//! Public site routing: the route table in
//! `application/config/routes.config.php`, the URLs the site layer builds from
//! it, and the parser that recovers a route from a request path.
//!
//! Site and page slugs obey `omeka_core::site::Slug`. Core owns that rule
//! (`Api/Adapter/SiteAdapter.php:260-266`) because the API validates it on
//! write; this module re-exports it so a caller at the site boundary does not
//! reach past the site layer, and adds what core does not have: the route
//! table.
//!
//! Route list (`routes.config.php:16-116`), all under `/s/:site-slug`:
//!
//! | name | route | defaults |
//! | --- | --- | --- |
//! | `site` | `/s/:site-slug` | site index |
//! | `site/resource` | `/:controller[/:action]` | action `browse` |
//! | `site/resource-id` | `/:controller/:id[/:action]` | action `show`, `id` is `\d+` |
//! | `site/item-set` | `/item-set/:item-set-id` | controller `Item`, action `browse` |
//! | `site/page-browse` | `/page` | controller `Page`, action `browse` |
//! | `site/page` | `/page/:page-slug` | controller `Page`, action `show` |
//! | `site/cross-site-search` | `/cross-site-search[/:action]` | action `index` |
//!
//! `/page` and `/cross-site-search` are literal routes that sit alongside the
//! generic `:controller` route. Both would match `/s/main/page`; upstream's
//! router resolves the tie to the literal route (Laminas `PriorityList` with
//! equal priorities), and so does this parser. The tie rule is inferred: it is
//! the only reading under which `SitePageRepresentation::siteUrl`
//! (`Api/Representation/SitePageRepresentation.php:140-154`) resolves to the
//! page controller instead of `Site\ResourceController`.
//!
//! A generated URL omits a segment whose value equals the route default
//! (`Laminas\Router\Http\Segment`), so item browse is `/s/main/item`, not
//! `/s/main/item/browse`.

use std::fmt;

pub use omeka_core::site::{Slug, SlugError, is_valid_slug, slugify};

/// Why a request path is not a public site route.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    /// The path is not under `/s/`.
    #[error("not a site path: {0:?}")]
    NotASitePath(String),
    /// The site slug segment is missing or fails `Slug::parse`.
    #[error("invalid site slug in {0:?}")]
    InvalidSiteSlug(String),
    /// An id segment matched `\d+` but does not fit the port's `u64` ids.
    #[error("id {0:?} is out of range")]
    InvalidId(String),
}

/// A parsed public site route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicRoute {
    /// `site`: `/s/:site-slug`.
    Home {
        /// The site slug.
        site: Slug,
    },
    /// `site/page-browse`: `/s/:site-slug/page`.
    PageBrowse {
        /// The site slug.
        site: Slug,
    },
    /// `site/page`: `/s/:site-slug/page/:page-slug`. The page slug is returned
    /// unvalidated, because the upstream route places no constraint on it.
    Page {
        /// The site slug.
        site: Slug,
        /// The page slug segment.
        page_slug: String,
    },
    /// `site/cross-site-search`: `/s/:site-slug/cross-site-search[/:action]`.
    CrossSiteSearch {
        /// The site slug.
        site: Slug,
        /// The action, `None` for the route's `index` default.
        action: Option<String>,
    },
    /// `site/resource` with the default `browse` action.
    ResourceBrowse {
        /// The site slug.
        site: Slug,
        /// The controller segment, e.g. `item`.
        controller: String,
    },
    /// `site/resource` with a named action.
    ResourceAction {
        /// The site slug.
        site: Slug,
        /// The controller segment.
        controller: String,
        /// The action segment.
        action: String,
    },
    /// `site/resource-id` with the default `show` action.
    ResourceShow {
        /// The site slug.
        site: Slug,
        /// The controller segment.
        controller: String,
        /// The resource id.
        id: u64,
    },
    /// `site/resource-id` with a named action.
    ResourceShowAction {
        /// The site slug.
        site: Slug,
        /// The controller segment.
        controller: String,
        /// The resource id.
        id: u64,
        /// The action segment.
        action: String,
    },
}

/// `/s/:site-slug`, the site index.
#[must_use]
pub fn site_url(site: &Slug) -> String {
    format!("/s/{site}")
}

/// `/s/:site-slug/page/:page-slug` (`SitePageRepresentation::siteUrl`).
#[must_use]
pub fn page_url(site: &Slug, page: &Slug) -> String {
    format!("/s/{site}/page/{page}")
}

/// `/s/:site-slug/page`, the page browse.
#[must_use]
pub fn page_browse_url(site: &Slug) -> String {
    format!("/s/{site}/page")
}

/// `site/resource` for a browse action. The action is omitted because it equals
/// the route default.
#[must_use]
pub fn resource_browse_url(site: &Slug, controller: &str, query: Option<&str>) -> String {
    let mut url = format!("/s/{site}/{controller}");
    if let Some(query) = query.filter(|value| !value.is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    url
}

/// `site/resource` for a named action, e.g. item search.
#[must_use]
pub fn resource_action_url(site: &Slug, controller: &str, action: &str) -> String {
    format!("/s/{site}/{controller}/{action}")
}

/// `site/resource-id` for the default `show` action, e.g. `/s/main/item/5`.
#[must_use]
pub fn resource_show_url(site: &Slug, controller: &str, id: u64) -> String {
    format!("/s/{site}/{controller}/{id}")
}

/// `site/item-set`, the item-set browse by id (`/s/:site-slug/item-set/:id`).
#[must_use]
pub fn item_set_url(site: &Slug, item_set_id: u64) -> String {
    format!("/s/{site}/item-set/{item_set_id}")
}

/// The controller segment a resource kind uses in a site URL
/// (`routes.config.php` child routes).
#[must_use]
pub fn controller_for(kind: omeka_core::resource::ResourceKind) -> &'static str {
    match kind {
        omeka_core::resource::ResourceKind::Item => "item",
        omeka_core::resource::ResourceKind::ItemSet => "item-set",
        omeka_core::resource::ResourceKind::Media => "media",
    }
}

/// The resource kind a site URL's controller segment names, `None` for a
/// segment that is not a resource controller (`page`, `cross-site-search`, or
/// an unknown name).
///
/// The inverse of [`controller_for`]; both read the same three route segments,
/// so a caller at the site boundary resolves a request path without repeating
/// the route table.
#[must_use]
pub fn kind_for_controller(controller: &str) -> Option<omeka_core::resource::ResourceKind> {
    match controller {
        "item" => Some(omeka_core::resource::ResourceKind::Item),
        "item-set" => Some(omeka_core::resource::ResourceKind::ItemSet),
        "media" => Some(omeka_core::resource::ResourceKind::Media),
        _ => None,
    }
}

/// Parse a request path into a [`PublicRoute`].
///
/// # Errors
/// [`RouteError`] when the path is not under `/s/`, the site slug segment
/// breaks the upstream slug rule, or an id segment does not fit `u64`.
pub fn parse_public_path(path: &str) -> Result<PublicRoute, RouteError> {
    let trimmed = path.split(['?', '#']).next().unwrap_or(path);
    let segments: Vec<&str> = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let [marker, site_segment, rest @ ..] = segments.as_slice() else {
        return Err(RouteError::NotASitePath(path.to_string()));
    };
    if *marker != "s" {
        return Err(RouteError::NotASitePath(path.to_string()));
    }
    let site = Slug::parse(site_segment)
        .map_err(|_| RouteError::InvalidSiteSlug(site_segment.to_string()))?;

    match rest {
        [] => Ok(PublicRoute::Home { site }),
        ["page"] => Ok(PublicRoute::PageBrowse { site }),
        ["page", page_slug] => Ok(PublicRoute::Page {
            site,
            page_slug: (*page_slug).to_string(),
        }),
        ["cross-site-search"] => Ok(PublicRoute::CrossSiteSearch { site, action: None }),
        ["cross-site-search", action] => Ok(PublicRoute::CrossSiteSearch {
            site,
            action: Some((*action).to_string()),
        }),
        [controller] => Ok(PublicRoute::ResourceBrowse {
            site,
            controller: (*controller).to_string(),
        }),
        [controller, last] => {
            if !last.is_empty() && last.bytes().all(|byte| byte.is_ascii_digit()) {
                // The route constraint for the id segment is `\d+`.
                let id =
                    parse_id(last).ok_or_else(|| RouteError::InvalidId((*last).to_string()))?;
                Ok(PublicRoute::ResourceShow {
                    site,
                    controller: (*controller).to_string(),
                    id,
                })
            } else {
                Ok(PublicRoute::ResourceAction {
                    site,
                    controller: (*controller).to_string(),
                    action: (*last).to_string(),
                })
            }
        }
        [controller, id_segment, action] => {
            if id_segment.is_empty() || !id_segment.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(RouteError::NotASitePath(path.to_string()));
            }
            let id = parse_id(id_segment)
                .ok_or_else(|| RouteError::InvalidId((*id_segment).to_string()))?;
            Ok(PublicRoute::ResourceShowAction {
                site,
                controller: (*controller).to_string(),
                id,
                action: (*action).to_string(),
            })
        }
        _ => Err(RouteError::NotASitePath(path.to_string())),
    }
}

fn parse_id(segment: &str) -> Option<u64> {
    if segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    segment.parse().ok()
}

impl fmt::Display for PublicRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Home { site } => write!(f, "/s/{site}"),
            Self::PageBrowse { site } => write!(f, "/s/{site}/page"),
            Self::Page { site, page_slug } => write!(f, "/s/{site}/page/{page_slug}"),
            Self::CrossSiteSearch { site, action } => match action {
                Some(action) => write!(f, "/s/{site}/cross-site-search/{action}"),
                None => write!(f, "/s/{site}/cross-site-search"),
            },
            Self::ResourceBrowse { site, controller } => write!(f, "/s/{site}/{controller}"),
            Self::ResourceAction {
                site,
                controller,
                action,
            } => write!(f, "/s/{site}/{controller}/{action}"),
            Self::ResourceShow {
                site,
                controller,
                id,
            } => write!(f, "/s/{site}/{controller}/{id}"),
            Self::ResourceShowAction {
                site,
                controller,
                id,
                action,
            } => write!(f, "/s/{site}/{controller}/{id}/{action}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_core::resource::ResourceKind;

    fn slug(raw: &str) -> Slug {
        Slug::parse(raw).unwrap()
    }

    /// `routes.config.php:16-116` plus `SitePageRepresentation::siteUrl`: the
    /// side layer builds the URLs upstream assembles.
    #[test]
    fn urls_match_upstream_route_table() {
        let site = slug("main");
        assert_eq!(site_url(&site), "/s/main");
        assert_eq!(page_url(&site, &slug("welcome")), "/s/main/page/welcome");
        assert_eq!(page_browse_url(&site), "/s/main/page");
        assert_eq!(resource_browse_url(&site, "item", None), "/s/main/item");
        assert_eq!(
            resource_browse_url(&site, "item", Some("year=2000&site_id=1")),
            "/s/main/item?year=2000&site_id=1"
        );
        assert_eq!(
            resource_browse_url(&site, "item-set", Some("")),
            "/s/main/item-set"
        );
        assert_eq!(
            resource_action_url(&site, "item", "search"),
            "/s/main/item/search"
        );
        assert_eq!(resource_show_url(&site, "item", 5), "/s/main/item/5");
        assert_eq!(item_set_url(&site, 3), "/s/main/item-set/3");
    }

    /// The controller segment per resource kind (`Entity/*` -> site routes).
    #[test]
    fn controller_names_match_upstream_routes() {
        assert_eq!(controller_for(ResourceKind::Item), "item");
        assert_eq!(controller_for(ResourceKind::ItemSet), "item-set");
        assert_eq!(controller_for(ResourceKind::Media), "media");
    }

    /// `kind_for_controller` inverts `controller_for`, and refuses a segment
    /// that is not a resource controller (`page`, an unknown name).
    #[test]
    fn kind_for_controller_inverts_the_route_segments() {
        for kind in [
            ResourceKind::Item,
            ResourceKind::ItemSet,
            ResourceKind::Media,
        ] {
            assert_eq!(kind_for_controller(controller_for(kind)), Some(kind));
        }
        assert_eq!(kind_for_controller("page"), None);
        assert_eq!(kind_for_controller("cross-site-search"), None);
        assert_eq!(kind_for_controller("Item"), None);
    }

    /// `routes.config.php`: every public route parses back to its shape.
    #[test]
    fn public_routes_parse_from_paths() {
        let site = slug("main");
        assert_eq!(
            parse_public_path("/s/main"),
            Ok(PublicRoute::Home { site: site.clone() })
        );
        assert_eq!(
            parse_public_path("/s/main/page"),
            Ok(PublicRoute::PageBrowse { site: site.clone() })
        );
        assert_eq!(
            parse_public_path("/s/main/page/welcome"),
            Ok(PublicRoute::Page {
                site: site.clone(),
                page_slug: "welcome".to_string()
            })
        );
        assert_eq!(
            parse_public_path("/s/main/cross-site-search/items"),
            Ok(PublicRoute::CrossSiteSearch {
                site: site.clone(),
                action: Some("items".to_string())
            })
        );
        assert_eq!(
            parse_public_path("/s/main/item"),
            Ok(PublicRoute::ResourceBrowse {
                site: site.clone(),
                controller: "item".to_string()
            })
        );
        assert_eq!(
            parse_public_path("/s/main/item/search"),
            Ok(PublicRoute::ResourceAction {
                site: site.clone(),
                controller: "item".to_string(),
                action: "search".to_string()
            })
        );
        assert_eq!(
            parse_public_path("/s/main/item-set/3"),
            Ok(PublicRoute::ResourceShow {
                site: site.clone(),
                controller: "item-set".to_string(),
                id: 3
            })
        );
        assert_eq!(
            parse_public_path("/s/main/media/9/show"),
            Ok(PublicRoute::ResourceShowAction {
                site: site.clone(),
                controller: "media".to_string(),
                id: 9,
                action: "show".to_string()
            })
        );
    }

    /// A query string and a trailing slash do not change the route; a path
    /// outside `/s/`, a bad site slug and an out-of-range id are errors.
    #[test]
    fn public_path_rejects_invalid_routes() {
        assert_eq!(
            parse_public_path("/s/main/item?page=2"),
            Ok(PublicRoute::ResourceBrowse {
                site: slug("main"),
                controller: "item".to_string()
            })
        );
        assert!(matches!(
            parse_public_path("/admin/items"),
            Err(RouteError::NotASitePath(_))
        ));
        assert!(matches!(
            parse_public_path("/s/My Site"),
            Err(RouteError::InvalidSiteSlug(_))
        ));
        assert!(matches!(
            parse_public_path("/s/main/item/18446744073709551616/"),
            Err(RouteError::InvalidId(_))
        ));
    }
}
