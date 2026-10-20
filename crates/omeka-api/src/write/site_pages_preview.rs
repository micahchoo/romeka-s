//! The live preview body: the same checks a `site_pages` write makes, with the
//! values answered instead of stored.
//!
//! `POST /s/:site-slug/preview` renders a page an editor has not saved yet. It
//! must fail where `PUT /api/site_pages/:id` would fail and build
//! exactly what the write would store, so this module composes the write's own
//! pieces ([`super::site_pages::parse_blocks`], `validate`, `slug_or_auto`,
//! `resolve_attachments`) and hands the result to the site renderer. The one
//! difference is a draft with no title or slug yet: the write refuses it, and
//! the preview renders it under placeholders, so a new page previews at once.
//!
//! It lives beside `site_pages` rather than inside it because `docs/api.md` is
//! generated from the write modules the `/api` dispatch names, and `o:id` is a
//! member of this route only: the write does not read it.

use chrono::NaiveDateTime;
use omeka_db::ids::{PageId, SiteId, UserId};
use omeka_db::repos::{SiteBlockAttachmentRepo, SiteRepo};
use omeka_db::rows::SitePageRow;
use serde_json::Value;

use crate::errors::ApiError;
use crate::read::{Api, db_error};
use crate::routes::Operation;
use crate::write::body::{Payload, php_json, ref_row};
use crate::write::resource::not_found as ref_not_found;
use crate::write::site_pages::{
    CONTROL, PAGE_CLASS, ResolvedAttachment, parse_blocks, resolve_attachments, slug_or_auto,
    validate,
};

/// A page assembled from a preview body, with nothing written to storage.
///
/// [`preview`] checks exactly what a write checks, then answers the values a
/// render reads instead of handing them to the repositories.
pub struct Preview {
    /// The page's site: the site the route names.
    pub site: SiteId,
    /// The edited page's id, `None` for a new page.
    pub id: Option<PageId>,
    /// The edited row's `created`, `None` for a new page.
    pub created: Option<NaiveDateTime>,
    /// The edited row's `modified`, `None` for a new page.
    pub modified: Option<NaiveDateTime>,
    /// `o:title` as an update stores it, empty for a draft not yet titled.
    pub title: String,
    /// `o:slug`, or the title's automatic slug, empty when neither exists yet.
    pub slug: String,
    /// `o:is_public`.
    pub is_public: bool,
    /// `o:layout` as stored.
    pub layout: Option<String>,
    /// `o:layout_data` in the shape the storage boundary keeps.
    pub layout_data: Option<Value>,
    /// `o:theme_settings` as the request carries it, `None` when the preview
    /// keeps the site's stored tokens. The preview never stores this.
    pub theme_settings: Option<Value>,
    /// The page's blocks in order, each with its attachments resolved.
    pub blocks: Vec<PreviewBlock>,
}

/// One block of a [`Preview`]: the members the write stores, with the block's
/// attachments already resolved.
pub struct PreviewBlock {
    /// `o:layout`.
    pub layout: String,
    /// `o:data`, purified when the layout is `html`.
    pub data: Value,
    /// `o:layout_data`.
    pub layout_data: Option<Value>,
    /// The block's attachments.
    pub attachments: Vec<ResolvedAttachment>,
}

/// Build the page a preview body describes, checking exactly what the write
/// checks and writing nothing.
///
/// `site` is the site the request route names. An edit names its page with
/// `o:id` and is authorized against that page's site, which must be `site`; a
/// new page is authorized as an add-page to `site`. Title, slug and block
/// validation are the write's own, so a body that would fail a write fails the
/// same way here, except that an empty title or slug previews
/// ([`validate_draft`]).
pub async fn preview(api: &Api<'_>, payload: &Payload, site: SiteId) -> Result<Preview, ApiError> {
    crate::write::values::collect(api, payload, &preview_control()).await?;
    let repo = SiteRepo::new(api.pool());
    let existing = match payload.id() {
        Some(raw) => {
            let Some(id) = ref_row(raw) else {
                return Err(ref_not_found(PAGE_CLASS, raw));
            };
            let Some(page) = repo.get_page(PageId(id)).await.map_err(db_error)? else {
                return Err(ref_not_found(PAGE_CLASS, raw));
            };
            // The route's site renders the preview, so a page of another site
            // is not found here even for a caller who may edit it.
            if page.site_id != site {
                return Err(ref_not_found(PAGE_CLASS, raw));
            }
            let owner = site_owner(api, site).await?;
            api.authorize_site_page(Operation::Update, site, owner)
                .await?;
            Some(page)
        }
        None => {
            let owner = site_owner(api, site).await?;
            api.authorize_site_add_page(site, owner).await?;
            None
        }
    };

    let mut blocks = if payload.has("o:block") {
        let mut parsed = Vec::new();
        for block in parse_blocks(payload)? {
            // The same carrier step a write runs, so a preview renders exactly
            // what a save would store: the plain rendering regenerated from the
            // block's own renderer, with its hash recorded.
            let (data, layout_data) = super::site_pages::carry_block(
                api,
                Some(site),
                &block.layout,
                block.data,
                block.layout_data,
            )
            .await?;
            let attachments = resolve_attachments(api, &block.attachments).await?;
            parsed.push(PreviewBlock {
                layout: block.layout,
                data,
                layout_data,
                attachments,
            });
        }
        parsed
    } else if let Some(row) = existing.as_ref() {
        // `update` leaves the blocks alone when the body carries no `o:block`,
        // so the preview renders the stored blocks.
        stored_blocks(api, row).await?
    } else {
        Vec::new()
    };
    if blocks.is_empty() && existing.is_none() {
        blocks.push(PreviewBlock {
            layout: "pageTitle".to_string(),
            data: Value::Array(Vec::new()),
            layout_data: None,
            attachments: Vec::new(),
        });
    }

    let title = payload
        .str_member("o:title")
        .unwrap_or("")
        .trim()
        .to_string();
    let slug = slug_or_auto(payload, &title);
    validate_draft(api, existing.as_ref(), site, &title, &slug).await?;

    Ok(Preview {
        site,
        id: existing.as_ref().map(|row| row.id),
        created: existing.as_ref().map(|row| row.created),
        modified: existing.as_ref().and_then(|row| row.modified),
        title,
        slug,
        is_public: payload.bool_member("o:is_public", true),
        layout: payload.str_member("o:layout").map(str::to_string),
        layout_data: payload.get("o:layout_data").map(php_json),
        theme_settings: payload.get("o:theme_settings").map(php_json),
        blocks,
    })
}

/// The write's title and slug checks, less the two a draft fails before the
/// curator has typed anything: an empty title and an empty slug. The renderer
/// fills both with placeholders, and the save still refuses them.
async fn validate_draft(
    api: &Api<'_>,
    existing: Option<&SitePageRow>,
    site: SiteId,
    title: &str,
    slug: &str,
) -> Result<(), ApiError> {
    match validate(api, existing, site, title, slug).await {
        Err(ApiError::Validation(mut errors)) => {
            // An empty value raises only its "required" message, so removing
            // the member removes exactly that message.
            if title.is_empty() {
                errors.remove("o:title");
            }
            if slug.is_empty() {
                errors.remove("o:slug");
            }
            if errors.is_empty() {
                Ok(())
            } else {
                Err(ApiError::Validation(errors))
            }
        }
        other => other,
    }
}

/// The members a preview body may carry: the write's control list plus
/// `o:id`, which names the page being edited, and `o:theme_settings`, the theme
/// tokens to render with instead of the site's stored ones (never stored).
fn preview_control() -> Vec<&'static str> {
    let mut control: Vec<&'static str> = CONTROL.to_vec();
    control.push("o:id");
    control.push("o:theme_settings");
    control
}

/// The owner the site grants a page write, for the site privilege check.
async fn site_owner(api: &Api<'_>, site: SiteId) -> Result<Option<UserId>, ApiError> {
    Ok(SiteRepo::new(api.pool())
        .get(site)
        .await
        .map_err(db_error)?
        .and_then(|row| row.owner_id))
}

/// The stored blocks of a page, in position order, with their stored
/// attachments.
async fn stored_blocks(api: &Api<'_>, page: &SitePageRow) -> Result<Vec<PreviewBlock>, ApiError> {
    let repo = SiteRepo::new(api.pool());
    let attachments = SiteBlockAttachmentRepo::new(api.pool());
    let mut blocks = Vec::new();
    for row in repo.list_blocks(page.id).await.map_err(db_error)? {
        let rows = attachments.list_for_block(row.id).await.map_err(db_error)?;
        blocks.push(PreviewBlock {
            layout: row.layout,
            data: row.data,
            layout_data: row.layout_data,
            attachments: rows
                .into_iter()
                .map(|row| ResolvedAttachment {
                    item: row.item_id,
                    media: row.media_id,
                    caption: row.caption,
                })
                .collect(),
        });
    }
    Ok(blocks)
}
