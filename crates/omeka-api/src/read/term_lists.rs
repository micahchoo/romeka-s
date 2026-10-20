//! `custom_vocabs`: the term lists (Custom Vocab module v2.1.0).
//!
//! `CustomVocabRepresentation::getJsonLd` shapes the document. The adapter has
//! no `buildQuery`, so a search reads the base `id` filter and the
//! `$sortFields` `id`, `label` and `owner`, and nothing else.

use omeka_core::term_list;
use omeka_db::ids::CustomVocabId;
use omeka_db::repos::{CustomVocabRepo, compare_text_ci};
use omeka_db::rows::CustomVocabRow;
use omeka_db::with_db;

use super::paging::{Limit, Window, sorted_page};
use super::params::{Params, Sort};
use super::query::{id_filter, id_matches};
use super::response::{ApiResponse, link_header, serialize};
use super::{Api, db_error, not_found, parse_id, resource_ref_opt};
use crate::errors::ApiError;
use crate::wire::JsonLdType;
use crate::wire::custom_vocab::{CustomVocab, RESOURCE_NAME};

/// The entity class a missing list is reported as.
pub(crate) const CUSTOM_VOCAB_CLASS: &str = "CustomVocab\\Entity\\CustomVocab";

pub async fn search_custom_vocabs(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let ids = id_filter(params);
    let rows: Vec<CustomVocabRow> =
        with_db!(api.pool(), |db| CustomVocabRepo::new(db).list().await)
            .map_err(db_error)?
            .into_iter()
            .filter(|row| id_matches(&ids, row.id.0))
            .collect();
    let (total, page) = sorted_page(
        rows,
        &sort,
        &limit,
        cmp_custom_vocab(params.get("sort_by").unwrap_or("id")),
    );
    let documents: Vec<CustomVocab> = page.iter().map(|row| document(api, row)).collect();
    Ok(ApiResponse::listing(
        serialize(&documents)?,
        total,
        link_header(api.base(), RESOURCE_NAME, params, &window, total),
    ))
}

pub async fn read_custom_vocab(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(CUSTOM_VOCAB_CLASS, raw_id)?;
    let Some(row) = with_db!(api.pool(), |db| CustomVocabRepo::new(db)
        .get(CustomVocabId(id))
        .await)
    .map_err(db_error)?
    else {
        return Err(not_found(CUSTOM_VOCAB_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(&document(api, &row))?))
}

/// The data types the lists register, `(name, label)` in id order: the order
/// `Module::addVocabularyServices` appends them to the registered names.
pub(crate) async fn data_types(api: &Api<'_>) -> Result<Vec<(String, String)>, ApiError> {
    Ok(
        with_db!(api.pool(), |db| CustomVocabRepo::new(db).list().await)
            .map_err(db_error)?
            .into_iter()
            .map(|row| (term_list::data_type_name(row.id.0), row.label))
            .collect(),
    )
}

fn document(api: &Api<'_>, row: &CustomVocabRow) -> CustomVocab {
    let base = api.base();
    CustomVocab {
        context: base.api_context(),
        id: base.api_resource(RESOURCE_NAME, row.id.0),
        jsonld_type: JsonLdType::single("o:CustomVocab"),
        o_id: row.id.0,
        label: row.label.clone(),
        lang: row.lang.clone(),
        terms: row.terms.clone(),
        uris: row.uris.clone(),
        item_set: resource_ref_opt(base, "item_sets", row.item_set_id.map(|id| id.0)),
        owner: resource_ref_opt(base, "users", row.owner_id.map(|id| id.0)),
    }
}

/// `CustomVocabAdapter::$sortFields`. `owner` orders by the owner's id, which
/// is what sorting on a Doctrine association does; a list with no owner sorts
/// first, as MySQL orders NULL first ascending.
fn cmp_custom_vocab(
    field: &str,
) -> impl Fn(&CustomVocabRow, &CustomVocabRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "label" => compare_text_ci(&a.label, &b.label),
        "owner" => a.owner_id.cmp(&b.owner_id).then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    }
}
