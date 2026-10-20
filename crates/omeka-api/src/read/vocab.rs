//! `vocabularies`, `resource_classes`, `properties` and `resource_templates`.
//!
//! `VocabularyRepresentation::getJsonLd`,
//! `AbstractVocabularyMemberRepresentation::getJsonLd` and
//! `ResourceTemplateRepresentation::getJsonLd` shape the documents.
//! `AbstractVocabularyMemberRepresentation::term` is `prefix:localName`, which
//! is why these builders read the [`Catalog`](super::catalog::Catalog).

use std::collections::HashMap;

use omeka_db::ids::{ClassId, PropertyId, TemplateId, VocabularyId};
use omeka_db::repos::{
    ClassFilter, ClassRepo, Paging, PropertyFilter, PropertyRepo, TemplateRepo, VocabularyFilter,
    VocabularyRepo, compare_text_ci,
};
use omeka_db::rows::{ClassRow, PropertyRow, TemplateRow, VocabularyRow};
use omeka_db::with_db;

use super::paging::{Limit, Window, sorted_page};
use super::params::Params;
use super::query::{id_filter, id_matches};
use super::response::link_header;
use super::{Api, db_error, not_found, parse_id, resource_ref, resource_ref_opt};
use crate::errors::ApiError;
use crate::read::params::Sort;
use crate::read::response::{ApiResponse, serialize};
use crate::wire::JsonLdType;
use crate::wire::resource_template::ResourceTemplateProperty;

// --------------------------------------------------------- vocabularies

pub async fn search_vocabularies(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = VocabularyFilter {
        owner_id: params.id("owner_id"),
        namespace_uri: params.get("namespace_uri").map(str::to_string),
        prefix: params.get("prefix").map(str::to_string),
    };
    let ids = id_filter(params);
    let rows = with_db!(api.pool(), |db| VocabularyRepo::new(db)
        .list(&filter, &Paging::default())
        .await)
    .map_err(db_error)?;
    let needle = search_needle(params);
    let rows: Vec<VocabularyRow> = rows
        .into_iter()
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| {
            matches(&row.label, needle.as_deref()) || matches(&row.prefix, needle.as_deref())
        })
        .collect();
    let (total, page) = sorted_page(
        rows,
        &sort,
        &limit,
        cmp_vocabulary(params.get("sort_by").unwrap_or("id")),
    );
    let documents: Vec<_> = page.iter().map(|row| vocabulary_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "vocabularies", params, &window, total),
    ))
}

pub async fn read_vocabulary(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(VOCABULARY_CLASS, raw_id)?;
    let Some(row) = with_db!(api.pool(), |db| VocabularyRepo::new(db)
        .get(VocabularyId(id))
        .await)
    .map_err(db_error)?
    else {
        return Err(not_found(VOCABULARY_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(&vocabulary_doc(
        api, &row,
    ))?))
}

const VOCABULARY_CLASS: &str = "Omeka\\Entity\\Vocabulary";

fn vocabulary_doc(api: &Api<'_>, row: &VocabularyRow) -> crate::wire::vocabulary::Vocabulary {
    let base = api.base();
    crate::wire::vocabulary::Vocabulary {
        context: base.api_context(),
        id: base.api_resource("vocabularies", row.id.0),
        jsonld_type: JsonLdType::single("o:Vocabulary"),
        o_id: row.id.0,
        namespace_uri: row.namespace_uri.clone(),
        prefix: row.prefix.clone(),
        label: row.label.clone(),
        comment: row.comment.clone(),
        owner: resource_ref_opt(base, "users", row.owner_id.map(|id| id.0)),
    }
}

// ------------------------------------------------------- resource_classes

pub async fn search_classes(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = ClassFilter {
        owner_id: params.id("owner_id"),
        vocabulary_id: params.id("vocabulary_id"),
        local_name: params.get("local_name").map(str::to_string),
    };
    let vocabularies = vocabulary_members(api, params).await?;
    let ids = id_filter(params);
    let rows = with_db!(api.pool(), |db| ClassRepo::new(db)
        .list(&filter, &Paging::default())
        .await)
    .map_err(db_error)?;
    let needle = search_needle(params);
    let rows: Vec<ClassRow> = rows
        .into_iter()
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| member_matches(vocabularies.as_ref(), row.vocabulary_id, &row.local_name))
        .filter(|row| matches(&row.label, needle.as_deref()))
        .collect();
    let (total, page) = sorted_page(
        rows,
        &sort,
        &limit,
        cmp_class(params.get("sort_by").unwrap_or("id")),
    );
    let documents: Vec<_> = page.iter().map(|row| class_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "resource_classes", params, &window, total),
    ))
}

pub async fn read_class(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(CLASS_CLASS, raw_id)?;
    let Some(row) =
        with_db!(api.pool(), |db| ClassRepo::new(db).get(ClassId(id)).await).map_err(db_error)?
    else {
        return Err(not_found(CLASS_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(&class_doc(api, &row))?))
}

const CLASS_CLASS: &str = "Omeka\\Entity\\ResourceClass";

fn class_doc(api: &Api<'_>, row: &ClassRow) -> crate::wire::resource_class::ResourceClass {
    let base = api.base();
    crate::wire::resource_class::ResourceClass {
        context: base.api_context(),
        id: base.api_resource("resource_classes", row.id.0),
        jsonld_type: JsonLdType::single("o:ResourceClass"),
        o_id: row.id.0,
        local_name: row.local_name.clone(),
        label: Some(row.label.clone()),
        comment: row.comment.clone(),
        term: api.catalog().prefix(row.vocabulary_id) + ":" + &row.local_name,
        vocabulary: resource_ref(base, "vocabularies", row.vocabulary_id.0),
    }
}

// ------------------------------------------------------------ properties

pub async fn search_properties(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let filter = PropertyFilter {
        owner_id: params.id("owner_id"),
        vocabulary_id: params.id("vocabulary_id"),
        local_name: params.get("local_name").map(str::to_string),
    };
    let vocabularies = vocabulary_members(api, params).await?;
    let ids = id_filter(params);
    let rows = with_db!(api.pool(), |db| PropertyRepo::new(db)
        .list(&filter, &Paging::default())
        .await)
    .map_err(db_error)?;
    let needle = search_needle(params);
    let rows: Vec<PropertyRow> = rows
        .into_iter()
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| member_matches(vocabularies.as_ref(), row.vocabulary_id, &row.local_name))
        .filter(|row| matches(&row.label, needle.as_deref()))
        .collect();
    let (total, page) = sorted_page(
        rows,
        &sort,
        &limit,
        cmp_property(params.get("sort_by").unwrap_or("id")),
    );
    let documents: Vec<_> = page.iter().map(|row| property_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "properties", params, &window, total),
    ))
}

pub async fn read_property(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(PROPERTY_CLASS, raw_id)?;
    let Some(row) = with_db!(api.pool(), |db| PropertyRepo::new(db)
        .get(PropertyId(id))
        .await)
    .map_err(db_error)?
    else {
        return Err(not_found(PROPERTY_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(&property_doc(api, &row))?))
}

const PROPERTY_CLASS: &str = "Omeka\\Entity\\Property";

fn property_doc(api: &Api<'_>, row: &PropertyRow) -> crate::wire::property::Property {
    let base = api.base();
    crate::wire::property::Property {
        context: base.api_context(),
        id: base.api_resource("properties", row.id.0),
        jsonld_type: JsonLdType::single("o:Property"),
        o_id: row.id.0,
        local_name: row.local_name.clone(),
        label: Some(row.label.clone()),
        comment: row.comment.clone(),
        term: api.catalog().prefix(row.vocabulary_id) + ":" + &row.local_name,
        vocabulary: resource_ref(base, "vocabularies", row.vocabulary_id.0),
    }
}

// ---------------------------------------------------- resource_templates

pub async fn search_templates(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let sort = Sort::from_params(params);
    let ids = id_filter(params);
    let rows = with_db!(api.pool(), |db| TemplateRepo::new(db).list().await).map_err(db_error)?;
    let needle = search_needle(params);
    let rows: Vec<TemplateRow> = rows
        .into_iter()
        .filter(|row| id_matches(&ids, row.id.0))
        .filter(|row| matches(&row.label, needle.as_deref()))
        .collect();
    let (total, page) = sorted_page(
        rows,
        &sort,
        &limit,
        cmp_template(params.get("sort_by").unwrap_or("id")),
    );
    let mut documents = Vec::with_capacity(page.len());
    for row in &page {
        documents.push(template_doc(api, row).await?);
    }
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "resource_templates", params, &window, total),
    ))
}

pub async fn read_template(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(TEMPLATE_CLASS, raw_id)?;
    let Some(row) = with_db!(api.pool(), |db| TemplateRepo::new(db)
        .get(TemplateId(id))
        .await)
    .map_err(db_error)?
    else {
        return Err(not_found(TEMPLATE_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(
        &template_doc(api, &row).await?,
    )?))
}

const TEMPLATE_CLASS: &str = "Omeka\\Entity\\ResourceTemplate";

async fn template_doc(
    api: &Api<'_>,
    row: &TemplateRow,
) -> Result<crate::wire::resource_template::ResourceTemplate, ApiError> {
    let base = api.base();
    let properties = with_db!(api.pool(), |db| TemplateRepo::new(db)
        .list_properties(row.id)
        .await)
    .map_err(db_error)?;
    let resource_template_property: Vec<ResourceTemplateProperty> = properties
        .iter()
        .map(|property| ResourceTemplateProperty {
            property: resource_ref(base, "properties", property.property_id.0),
            alternate_label: property.alternate_label.clone(),
            alternate_comment: property.alternate_comment.clone(),
            data_type: property.data_type.clone().unwrap_or_default(),
            is_required: property.is_required,
            is_private: property.is_private,
            default_lang: property.default_lang.clone(),
        })
        .collect();
    Ok(crate::wire::resource_template::ResourceTemplate {
        context: base.api_context(),
        id: base.api_resource("resource_templates", row.id.0),
        jsonld_type: JsonLdType::single("o:ResourceTemplate"),
        o_id: row.id.0,
        label: row.label.clone(),
        owner: resource_ref_opt(base, "users", row.owner_id.map(|id| id.0)),
        resource_class: resource_ref_opt(
            base,
            "resource_classes",
            row.resource_class_id.map(|id| id.0),
        ),
        title_property: resource_ref_opt(base, "properties", row.title_property_id.map(|id| id.0)),
        description_property: resource_ref_opt(
            base,
            "properties",
            row.description_property_id.map(|id| id.0),
        ),
        resource_template_property,
    })
}

// --------------------------------------------------------------- helpers

/// The vocabulary filters `PropertyAdapter::buildQuery` and
/// `ResourceClassAdapter::buildQuery` apply to a vocabulary member.
///
/// `vocabulary_prefix` and `vocabulary_namespace_uri` are equalities on the
/// member's vocabulary row, and `term` is `prefix:localName` narrowed to the
/// one property or class a valid JSON-LD term names. `vocabulary_id` and
/// `local_name` are equalities on the member itself and reach the repository's
/// own filter, so they are not here.
struct MemberFilter {
    prefix: Option<String>,
    namespace_uri: Option<String>,
    term: Option<(String, String)>,
}

impl MemberFilter {
    /// `None` when the request asked for none of the three, so the vocabulary
    /// table is not read.
    fn from_params(params: &Params) -> Option<Self> {
        let prefix = params.get("vocabulary_prefix").map(str::to_string);
        let namespace_uri = params.get("vocabulary_namespace_uri").map(str::to_string);
        let term = params.get("term").and_then(term_parts);
        if prefix.is_none() && namespace_uri.is_none() && term.is_none() {
            return None;
        }
        Some(Self {
            prefix,
            namespace_uri,
            term,
        })
    }

    fn matches(&self, vocabulary: &VocabularyRow, local_name: &str) -> bool {
        if let Some(prefix) = &self.prefix
            && &vocabulary.prefix != prefix
        {
            return false;
        }
        if let Some(uri) = &self.namespace_uri
            && &vocabulary.namespace_uri != uri
        {
            return false;
        }
        if let Some((prefix, name)) = &self.term
            && (&vocabulary.prefix != prefix || local_name != name)
        {
            return false;
        }
        true
    }
}

/// `Vocabulary::TERM_REGEX`, `^[a-z0-9_-]+:[a-z0-9_-]+$`, as the two parts a
/// valid term splits into. Anything else leaves the `term` parameter ignored,
/// which is the `isTerm` guard.
fn term_parts(term: &str) -> Option<(String, String)> {
    let (prefix, local_name) = term.split_once(':')?;
    if !is_term_part(prefix) || !is_term_part(local_name) {
        return None;
    }
    Some((prefix.to_string(), local_name.to_string()))
}

fn is_term_part(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// The vocabulary row of every member a `vocabulary_prefix`,
/// `vocabulary_namespace_uri` or `term` filter can match, or `None` when the
/// request asked for none of them.
async fn vocabulary_members(
    api: &Api<'_>,
    params: &Params,
) -> Result<Option<VocabularyMembers>, ApiError> {
    let Some(filter) = MemberFilter::from_params(params) else {
        return Ok(None);
    };
    let rows = with_db!(api.pool(), |db| VocabularyRepo::new(db)
        .list(&VocabularyFilter::default(), &Paging::default())
        .await)
    .map_err(db_error)?;
    Ok(Some(VocabularyMembers {
        filter,
        rows: rows.into_iter().map(|row| (row.id, row)).collect(),
    }))
}

/// The requested vocabulary filters and the rows they read.
struct VocabularyMembers {
    filter: MemberFilter,
    rows: HashMap<VocabularyId, VocabularyRow>,
}

impl VocabularyMembers {
    /// Whether a member row passes every requested filter. A row whose
    /// vocabulary is missing matches nothing, which is what the inner join
    /// `buildQuery` builds does.
    fn matches(&self, vocabulary: VocabularyId, local_name: &str) -> bool {
        self.rows
            .get(&vocabulary)
            .is_some_and(|row| self.filter.matches(row, local_name))
    }
}

/// Whether a member row passes the requested vocabulary filters. `None` means
/// the request asked for none, so every row passes.
fn member_matches(
    members: Option<&VocabularyMembers>,
    vocabulary: VocabularyId,
    local_name: &str,
) -> bool {
    members.is_none_or(|members| members.matches(vocabulary, local_name))
}

fn search_needle(params: &Params) -> Option<String> {
    params
        .get("search")
        .filter(|needle| !needle.is_empty())
        .map(str::to_lowercase)
}

fn matches(text: &str, needle: Option<&str>) -> bool {
    match needle {
        Some(needle) => text.to_lowercase().contains(needle),
        None => true,
    }
}

/// `VocabularyAdapter::$sortFields`: the label sorts through the collation. The
/// field name is the adapter's own, not a shared sort-field name.
fn cmp_vocabulary(
    field: &str,
) -> impl Fn(&VocabularyRow, &VocabularyRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "label" => compare_text_ci(&a.label, &b.label),
        _ => a.id.cmp(&b.id),
    }
}

fn cmp_class(field: &str) -> impl Fn(&ClassRow, &ClassRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "label" => compare_text_ci(&a.label, &b.label),
        _ => a.id.cmp(&b.id),
    }
}

fn cmp_property(field: &str) -> impl Fn(&PropertyRow, &PropertyRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "label" => compare_text_ci(&a.label, &b.label),
        _ => a.id.cmp(&b.id),
    }
}

fn cmp_template(field: &str) -> impl Fn(&TemplateRow, &TemplateRow) -> std::cmp::Ordering + '_ {
    move |a, b| match field {
        "label" => compare_text_ci(&a.label, &b.label),
        _ => a.id.cmp(&b.id),
    }
}
