//! A draft's life before it runs: made from sources or copied, its plan
//! saved by revision, deleted (`docs/import-v2.md`, "Lifecycle and the
//! ledger", "Copy", "Plan saves").
//!
//! Every plan that reaches the database passes [`ImportPlan::from_json`]
//! and the install check [`ImportPlan::refuse_existing_labels`], and is
//! stored in its canonical form ([`ImportPlan::to_json`]).

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, UserId};
use omeka_db::repos::{ImportRepo, PlanSave, TemplateRepo};
use omeka_db::rows::{ImportRow, ImportStatus, NewImport};
use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::plan::{ImportPlan, PlanError, SourcePlan, default_plan};

/// Why a draft operation was refused.
#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    #[error("no such import")]
    Missing,
    /// The caller edited an older revision than this one.
    #[error("the plan was saved again since revision {0} was read")]
    Stale(u64),
    #[error("the import is {}, not a draft", .0.as_str())]
    NotDraft(ImportStatus),
    /// The plan, or the plan of the import to copy, is a v1 plan.
    #[error("the plan is a v1 plan")]
    V1Plan,
    #[error("a template already has the label {0:?}")]
    TypeExists(String),
    #[error(transparent)]
    Plan(PlanError),
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<PlanError> for DraftError {
    fn from(error: PlanError) -> Self {
        match error {
            PlanError::V1Plan => Self::V1Plan,
            PlanError::TypeExists(label) => Self::TypeExists(label),
            other => Self::Plan(other),
        }
    }
}

/// What a new draft starts from.
#[derive(Debug, Clone)]
pub enum Start {
    /// The default plan on these sources.
    Sources(Vec<SourcePlan>),
    /// The plan of another import, decisions and all.
    CopyFrom(ImportId),
}

fn plan_value(plan: &ImportPlan) -> Result<Value, DraftError> {
    serde_json::from_str(&plan.to_json()).map_err(|error| DraftError::Plan(PlanError::Shape(error)))
}

/// Make a draft owned by `owner`.
///
/// # Errors
/// [`DraftError::Missing`] for a `copyFrom` that names no import,
/// [`DraftError::V1Plan`] when that import holds a v1 plan, and
/// [`DraftError::Plan`] for sources the boundary refuses.
pub async fn create<DB>(
    pool: &Pool<DB>,
    owner: Option<UserId>,
    start: Start,
) -> Result<ImportRow, DraftError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let repo = ImportRepo::new(pool);
    let (plan, copied_from) = match start {
        Start::Sources(sources) => (default_plan(sources)?, None),
        Start::CopyFrom(from) => {
            let row = repo.get(from).await?.ok_or(DraftError::Missing)?;
            (ImportPlan::from_json(&row.plan.to_string())?, Some(from))
        }
    };
    Ok(repo
        .create_draft(&NewImport {
            owner_id: owner,
            sources: serde_json::to_value(&plan.sources)
                .map_err(|error| DraftError::Plan(PlanError::Shape(error)))?,
            plan: plan_value(&plan)?,
            copied_from,
        })
        .await?)
}

/// Save `plan` on draft `id`, if `revision` is the one stored. Returns the
/// new revision.
///
/// # Errors
/// [`DraftError::V1Plan`], [`DraftError::Plan`] or
/// [`DraftError::TypeExists`] for a plan the boundary refuses;
/// [`DraftError::Stale`] for an old revision; [`DraftError::NotDraft`] once
/// the import has started.
pub async fn save<DB>(
    pool: &Pool<DB>,
    id: ImportId,
    plan: &Value,
    revision: u64,
) -> Result<u64, DraftError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let plan = ImportPlan::from_json(&plan.to_string())?;
    let labels: Vec<String> = TemplateRepo::new(pool)
        .list()
        .await?
        .into_iter()
        .map(|row| row.label)
        .collect();
    plan.refuse_existing_labels(labels.iter().map(String::as_str))?;
    match ImportRepo::new(pool)
        .save_plan(id, &plan_value(&plan)?, revision)
        .await?
    {
        PlanSave::Saved(revision) => Ok(revision),
        PlanSave::Stale(stored) => Err(DraftError::Stale(stored)),
        PlanSave::NotDraft(status) => Err(DraftError::NotDraft(status)),
        PlanSave::Missing => Err(DraftError::Missing),
    }
}

/// Delete draft `id` with its index and uploads.
///
/// # Errors
/// [`DraftError::Missing`], or [`DraftError::NotDraft`] once it has started.
pub async fn delete<DB>(pool: &Pool<DB>, id: ImportId) -> Result<(), DraftError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let repo = ImportRepo::new(pool);
    if repo.delete_draft(id).await? {
        return Ok(());
    }
    match repo.get(id).await? {
        Some(row) => Err(DraftError::NotDraft(row.status)),
        None => Err(DraftError::Missing),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use serde_json::json;

    use super::*;
    use crate::import::plan::{RelPath, SourceId};

    fn sources() -> Vec<SourcePlan> {
        vec![SourcePlan::Folder {
            id: SourceId("s1".into()),
            root: "archive".into(),
            path: RelPath::parse("Box 1").unwrap(),
        }]
    }

    #[tokio::test]
    async fn a_save_needs_the_stored_revision_and_a_draft() {
        let pool = crate::testing::temp_db().await;
        let draft = create(&pool, None, Start::Sources(sources()))
            .await
            .unwrap();
        let mut plan = draft.plan.clone();
        plan["structure"]["levels"][0]["grouping"]["pattern"] = json!("^(?<identifier>.+)$");

        assert_eq!(save(&pool, draft.id, &plan, 0).await.unwrap(), 1);
        assert!(matches!(
            save(&pool, draft.id, &plan, 0).await,
            Err(DraftError::Stale(1))
        ));
        assert!(matches!(
            save(&pool, draft.id, &json!({ "setLevel": 1 }), 1).await,
            Err(DraftError::V1Plan)
        ));
        let repo = ImportRepo::new(&pool);
        repo.start_run(draft.id, 1).await.unwrap();
        assert!(matches!(
            save(&pool, draft.id, &plan, 1).await,
            Err(DraftError::NotDraft(ImportStatus::Running))
        ));
        assert!(matches!(
            delete(&pool, draft.id).await,
            Err(DraftError::NotDraft(_))
        ));
    }

    #[tokio::test]
    async fn a_copy_keeps_the_plan_and_its_decisions_and_a_v1_import_is_refused() {
        let pool = crate::testing::temp_db().await;
        let draft = create(&pool, None, Start::Sources(sources()))
            .await
            .unwrap();
        let mut plan = draft.plan.clone();
        plan["decisions"] = json!([{ "kind": "leave",
            "entry": { "file": { "source": "s1", "path": "a.jpg" } } }]);
        save(&pool, draft.id, &plan, 0).await.unwrap();

        let copy = create(&pool, None, Start::CopyFrom(draft.id))
            .await
            .unwrap();
        assert_eq!(copy.plan["decisions"], plan["decisions"]);
        assert_eq!(copy.copied_from, Some(draft.id));
        assert_eq!(copy.revision, 0);

        let v1 = ImportRepo::new(&pool)
            .create_draft(&NewImport {
                owner_id: None,
                sources: json!([]),
                plan: json!({ "setLevel": 1 }),
                copied_from: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            create(&pool, None, Start::CopyFrom(v1.id)).await,
            Err(DraftError::V1Plan)
        ));
        assert!(matches!(
            create(&pool, None, Start::CopyFrom(ImportId(999))).await,
            Err(DraftError::Missing)
        ));
    }
}
