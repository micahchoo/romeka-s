//! The `fulltext_search` row of a resource an import wrote, built on the record's transaction.
//!
//! This is the text `omeka-api`'s `write/fulltext.rs` builds after an api write, read through the
//! transaction so the row commits with the record: `AbstractResourceEntityAdapter::getFulltextText`
//! joins the text of every public value and the public values of its annotation,
//! `ItemAdapter::getFulltextText` appends each media's text, and `MediaAdapter::getFulltextText`
//! appends the html renderer's text. A media write re-saves its item, whose text includes it.

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::import_writes::ImportWrites;
use super::{bind_id, bind_opt_id};
use crate::DbError;
use crate::backend::Backend;
use crate::fulltext::{linked_resource, strip_tags, value_text};
use crate::ids::{ItemId, MediaId, ResourceId};
use crate::rows::{ImportResource, ValueRow};

impl<'t, DB: Backend> ImportWrites<'t, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    /// Write the index row of `target`, and of its item when `target` is a media.
    pub(super) async fn reindex(&mut self, target: ImportResource) -> Result<(), DbError> {
        let id = target.id();
        let Some(row) = self.resource_row(id).await? else {
            return Ok(());
        };
        let mut texts = self.value_texts(id).await?;
        let mut item = None;
        match target {
            ImportResource::Item(item_id) => {
                for media in self.media_order(item_id).await? {
                    let text = self.media_text(media).await?;
                    if !text.is_empty() {
                        texts.push(text);
                    }
                }
            }
            ImportResource::Media(media) => {
                let (owner, text) = self.renderer_text(media).await?;
                if !text.is_empty() {
                    texts.push(text);
                }
                item = owner;
            }
            ImportResource::ItemSet(_) => {}
        }
        sqlx::query(DB::upsert_fulltext_search())
            .bind(bind_id(id))
            .bind(target.index_name())
            .bind(bind_opt_id(row.owner_id))
            .bind(row.is_public)
            .bind(row.title)
            .bind(texts.join("\n"))
            .execute(&mut *self.conn)
            .await?;
        if let Some(item) = item {
            Box::pin(self.reindex(ImportResource::Item(item))).await?;
        }
        Ok(())
    }

    /// The text of each public value, then the public values of its annotation.
    async fn value_texts(&mut self, id: ResourceId) -> Result<Vec<String>, DbError> {
        let mut texts = Vec::new();
        for value in self.values_of(id).await? {
            if !value.is_public {
                continue;
            }
            texts.push(self.value_text(&value).await?);
            if let Some(annotation) = value.value_annotation_id {
                for inner in self.values_of(annotation.as_resource()).await? {
                    if inner.is_public {
                        texts.push(self.value_text(&inner).await?);
                    }
                }
            }
        }
        Ok(texts)
    }

    /// The value's text by [`crate::fulltext::value_text`], with the linked title read here.
    async fn value_text(&mut self, value: &ValueRow) -> Result<String, DbError> {
        let linked_title = match linked_resource(value) {
            Some(linked) => self.resource_row(linked).await?.and_then(|row| row.title),
            None => None,
        };
        Ok(value_text(value, linked_title))
    }

    async fn media_text(&mut self, media: MediaId) -> Result<String, DbError> {
        let mut texts = self.value_texts(media.as_resource()).await?;
        texts.push(self.renderer_text(media).await?.1);
        Ok(texts
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// The media's item, and `Renderer\Html::getFulltextText`: the `html` member of `data` with
    /// its tags stripped. No other renderer has text.
    async fn renderer_text(&mut self, media: MediaId) -> Result<(Option<ItemId>, String), DbError> {
        let Some(row) = sqlx::query("SELECT item_id, renderer, data FROM media WHERE id = ?")
            .bind(bind_id(media))
            .fetch_optional(&mut *self.conn)
            .await?
        else {
            return Ok((None, String::new()));
        };
        let item: ItemId = DB::id(&row, "item_id")?;
        if DB::text(&row, "renderer")? != "html" {
            return Ok((Some(item), String::new()));
        }
        let data = DB::opt_json(&row, "data")?;
        let html = data
            .as_ref()
            .and_then(|data| data.get("html"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        Ok((Some(item), strip_tags(html)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tags_matches_the_api_index() {
        assert_eq!(strip_tags("<p>Bronze &amp; iron</p>"), "Bronze & iron");
        assert_eq!(strip_tags("a < b"), "a ");
        assert_eq!(strip_tags("caf&#233;"), "café");
        assert_eq!(strip_tags("&#x27;quoted&#x27; &bogus;"), "'quoted' &bogus;");
    }
}
