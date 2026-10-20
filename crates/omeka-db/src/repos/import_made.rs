//! The fields and item types a run makes before its records, on the caller's transaction, so the
//! ledger row that records each one commits with it (`docs/import-v2.md`, "Run details"). A crash
//! between the two would leave a field or a type that no undo knows about.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::import_writes::ImportWrites;
use super::{PropertyRepo, TemplateRepo};
use crate::DbError;
use crate::backend::Backend;
use crate::rows::{NewProperty, NewTemplate, NewTemplateProperty, PropertyRow, TemplateRow};

impl<DB: Backend> ImportWrites<'_, DB>
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
    pub async fn create_property(&mut self, new: &NewProperty) -> Result<PropertyRow, DbError> {
        PropertyRepo::<DB>::insert(self.conn, new).await
    }

    /// A template with its fields. Each field's `resource_template_id` is replaced by the new
    /// template's id.
    pub async fn create_template(
        &mut self,
        new: &NewTemplate,
        fields: &[NewTemplateProperty],
    ) -> Result<TemplateRow, DbError> {
        let row = TemplateRepo::<DB>::insert(self.conn, new).await?;
        for field in fields {
            let field = NewTemplateProperty {
                resource_template_id: row.id,
                ..field.clone()
            };
            TemplateRepo::<DB>::insert_property(self.conn, &field).await?;
        }
        Ok(row)
    }
}
