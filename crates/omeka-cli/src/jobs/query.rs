//! Batch job arguments: the resource query and the chunk size.
//!
//! `BatchDelete` and `BatchUpdate` search a resource kind with a query, then
//! work in chunks. Upstream reads `batch_chunk_size` from settings and defaults
//! to 100 (`BatchDelete::perform`, `BatchUpdate::perform`). The query is the
//! subset of the API query parameters the repositories filter on; unknown keys
//! are ignored, as `AbstractResourceEntityAdapter::buildQuery` ignores params it
//! does not register.

use serde::Deserialize;
use serde_json::Value;

use omeka_db::ids::{ClassId, ItemSetId, TemplateId, UserId};
use omeka_db::repos::{ItemFilter, ItemSetFilter, MediaFilter};

use super::{JobContext, JobError};
use crate::store;

/// Upstream's default when the `batch_chunk_size` setting is absent.
pub const DEFAULT_CHUNK_SIZE: i64 = 100;

/// A batch resource query.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BatchQuery {
    pub owner_id: Option<u64>,
    pub resource_class_id: Option<u64>,
    pub resource_template_id: Option<u64>,
    pub item_set_id: Option<u64>,
    pub is_public: Option<bool>,
    pub is_open: Option<bool>,
}

impl BatchQuery {
    /// The same query as an [`ItemFilter`].
    #[must_use]
    pub fn item_filter(&self) -> ItemFilter {
        ItemFilter {
            owner_id: self.owner_id.map(UserId::from),
            resource_class_id: self.resource_class_id.map(ClassId::from),
            resource_template_id: self.resource_template_id.map(TemplateId::from),
            item_set_id: self.item_set_id.map(ItemSetId::from),
            is_public: self.is_public,
        }
    }

    /// The same query as an [`ItemSetFilter`].
    #[must_use]
    pub fn item_set_filter(&self) -> ItemSetFilter {
        ItemSetFilter {
            owner_id: self.owner_id.map(UserId::from),
            resource_class_id: self.resource_class_id.map(ClassId::from),
            resource_template_id: self.resource_template_id.map(TemplateId::from),
            is_public: self.is_public,
            is_open: self.is_open,
        }
    }

    /// The same query as a [`MediaFilter`]. Media has no item-set filter.
    #[must_use]
    pub fn media_filter(&self) -> MediaFilter {
        MediaFilter {
            owner_id: self.owner_id.map(UserId::from),
            resource_class_id: self.resource_class_id.map(ClassId::from),
            resource_template_id: self.resource_template_id.map(TemplateId::from),
            is_public: self.is_public,
            ..MediaFilter::default()
        }
    }
}

/// Read a query argument, treating an absent or null value as "match all".
///
/// # Errors
/// A [`JobError::InvalidArgument`] when the value is not a query object.
pub fn query_arg(context: &JobContext<'_>, name: &str) -> Result<BatchQuery, JobError> {
    match context.arg(name) {
        None | Some(Value::Null) => Ok(BatchQuery::default()),
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|error| JobError::InvalidArgument(format!("argument \"{name}\": {error}"))),
    }
}

/// The configured chunk size, clamped to at least one.
///
/// # Errors
/// A database error while reading the `batch_chunk_size` setting.
pub async fn chunk_size(context: &JobContext<'_>) -> Result<usize, JobError> {
    let configured = store::read_setting_i64(context.pool, "batch_chunk_size")
        .await?
        .unwrap_or(DEFAULT_CHUNK_SIZE);
    let size =
        usize::try_from(configured).unwrap_or(usize::try_from(DEFAULT_CHUNK_SIZE).unwrap_or(1));
    Ok(size.max(1))
}
