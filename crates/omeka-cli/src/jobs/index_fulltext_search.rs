//! Port of `application/src/Job/IndexFulltextSearch.php`.
//!
//! Clears `fulltext_search`, then indexes every fulltext-searchable resource
//! type. Upstream indexes the adapters that implement
//! `FulltextSearchableInterface` and skips generic resources and value
//! annotations; at the frozen SHA those adapters are items, item sets and
//! media. Each page is 100 rows, and `shouldStop` is checked per page.
//!
//! The indexed text mirrors `AbstractResourceEntityAdapter::getFulltextText`:
//! every public value's data-type text, joined by a newline. The value column
//! holds the literal; `uri` uses `uri value` (`Uri::getFulltextText`); a
//! resource value contributes the referenced resource's title
//! (`AbstractResource::getFulltextText`).

use async_trait::async_trait;

use omeka_db::ids::ResourceId;
use omeka_db::repos::{
    FulltextRepo, ItemRepo, ItemSetRepo, MediaRepo, Paging, RelationRepo, ResourceRepo, ValueRepo,
};
use omeka_db::rows::{FulltextSearchRow, ResourceRow};
use omeka_db::with_db;

use super::{Job, JobContext, JobError};

/// Upstream pages at `per_page => 100`.
const PAGE_SIZE: i64 = 100;

/// The resource types this job indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Searchable {
    Items,
    ItemSets,
    Media,
}

impl Searchable {
    const ALL: [Self; 3] = [Self::Items, Self::ItemSets, Self::Media];

    fn name(self) -> &'static str {
        match self {
            Self::Items => "items",
            Self::ItemSets => "item_sets",
            Self::Media => "media",
        }
    }
}

/// `Omeka\Job\IndexFulltextSearch`.
pub struct IndexFulltextSearch;

#[async_trait]
impl Job for IndexFulltextSearch {
    fn class(&self) -> &'static str {
        "Omeka\\Job\\IndexFulltextSearch"
    }

    async fn perform(&self, context: &JobContext<'_>) -> Result<(), JobError> {
        with_db!(context.pool, |db| FulltextRepo::new(db).clear().await)?;
        for kind in Searchable::ALL {
            index_kind(context, kind).await?;
        }
        Ok(())
    }
}

async fn index_kind(context: &JobContext<'_>, kind: Searchable) -> Result<(), JobError> {
    let mut page = 0i64;
    loop {
        if context.should_stop().await? {
            return Ok(());
        }
        let paging = Paging::new(PAGE_SIZE, page * PAGE_SIZE);
        let resources = fetch_page(context, kind, &paging).await?;
        if resources.is_empty() {
            return Ok(());
        }
        for resource in resources {
            let row = fulltext_row(context, kind, &resource).await?;
            with_db!(context.pool, |db| FulltextRepo::new(db).save(&row).await)?;
        }
        page += 1;
    }
}

async fn fetch_page(
    context: &JobContext<'_>,
    kind: Searchable,
    paging: &Paging,
) -> Result<Vec<ResourceRow>, JobError> {
    Ok(match kind {
        Searchable::Items => with_db!(context.pool, |db| ItemRepo::new(db)
            .list(&Default::default(), paging)
            .await)?
        .into_iter()
        .map(|row| row.resource)
        .collect(),
        Searchable::ItemSets => with_db!(context.pool, |db| ItemSetRepo::new(db)
            .list(&Default::default(), paging)
            .await)?
        .into_iter()
        .map(|row| row.resource)
        .collect(),
        Searchable::Media => with_db!(context.pool, |db| MediaRepo::new(db)
            .list(&Default::default(), paging)
            .await)?
        .into_iter()
        .map(|row| row.resource)
        .collect(),
    })
}

async fn fulltext_row(
    context: &JobContext<'_>,
    kind: Searchable,
    resource: &ResourceRow,
) -> Result<FulltextSearchRow, JobError> {
    // A title drawn from a private value stays out of the index, as private value text does.
    let title = with_db!(context.pool, |db| RelationRepo::new(db)
        .public_title(resource)
        .await)?;
    Ok(FulltextSearchRow {
        id: resource.id,
        resource: kind.name().to_string(),
        owner_id: resource.owner_id,
        is_public: resource.is_public,
        title,
        text: Some(fulltext_text(context, resource).await?),
    })
}

async fn fulltext_text(
    context: &JobContext<'_>,
    resource: &ResourceRow,
) -> Result<String, JobError> {
    let values = with_db!(context.pool, |db| ValueRepo::new(db)
        .list_for_resource(resource.id)
        .await)?;
    let mut texts = Vec::new();
    for value in values {
        if !value.is_public {
            continue;
        }
        let text = if value.r#type == "uri" {
            format!(
                "{} {}",
                value.uri.as_deref().unwrap_or_default(),
                value.value.as_deref().unwrap_or_default()
            )
            .trim()
            .to_string()
        } else if value.r#type.starts_with("resource") {
            referenced_title(context, value.value_resource_id).await?
        } else {
            value.value.unwrap_or_default()
        };
        if !text.is_empty() {
            texts.push(text);
        }
    }
    Ok(texts.join("\n"))
}

async fn referenced_title(
    context: &JobContext<'_>,
    resource: Option<ResourceId>,
) -> Result<String, JobError> {
    let Some(id) = resource else {
        return Ok(String::new());
    };
    let Some(row) = with_db!(context.pool, |db| ResourceRepo::new(db).get(id).await)? else {
        return Ok(String::new());
    };
    Ok(with_db!(context.pool, |db| RelationRepo::new(db)
        .public_title(&row)
        .await)?
    .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::Runner;
    use crate::testing::{compose, seeded_db};
    use omeka_db::rows::{NewItem, NewItemSet, NewResource, NewValue};

    #[tokio::test]
    async fn index_fulltext_search_rows_per_resource_type() {
        let db = seeded_db().await;
        let item = ItemRepo::new(&db.pool)
            .create(&NewItem {
                resource: NewResource {
                    title: Some("A painting".to_string()),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        let description = crate::terms::resolve_property(&db.db_pool, "dcterms:description")
            .await
            .unwrap();
        ValueRepo::new(&db.pool)
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id: description,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some("Oil on canvas".to_string()),
                uri: None,
                is_public: true,
            })
            .await
            .unwrap();
        // A private value must not enter the index text.
        ValueRepo::new(&db.pool)
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id: description,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some("secret".to_string()),
                uri: None,
                is_public: false,
            })
            .await
            .unwrap();
        ItemSetRepo::new(&db.pool)
            .create(&NewItemSet {
                resource: NewResource {
                    title: Some("A set".to_string()),
                    ..NewResource::default()
                },
                is_open: false,
            })
            .await
            .unwrap();

        let job_id = compose(&db.db_pool, &IndexFulltextSearch, serde_json::json!({})).await;
        Runner::new(&db.db_pool).run(job_id).await.expect("run");

        let rows = FulltextRepo::new(&db.pool).list().await.unwrap();
        assert_eq!(rows.len(), 2, "one row per indexed resource");
        assert_eq!(rows[0].resource, "item_sets");
        assert_eq!(rows[1].resource, "items");
        assert_eq!(rows[1].title.as_deref(), Some("A painting"));
        let text = rows[1].text.as_deref().unwrap();
        assert!(text.contains("Oil on canvas"), "text was {text:?}");
        assert!(!text.contains("secret"), "private value leaked: {text:?}");
        assert_eq!(
            omeka_db::repos::JobRepo::new(&db.pool)
                .status(job_id)
                .await
                .unwrap()
                .as_deref(),
            Some("completed")
        );
    }
}
