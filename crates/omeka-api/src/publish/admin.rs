//! `GET` and `POST /api/{custom_vocabs|vocabularies}/{id}/publication`: the admin's side of
//! publishing.
//!
//! `GET` answers a [`View`]: the draft or published state, the address, the version history, every
//! term with its URI and state, and the Turtle the next publish would serve. `POST` takes one
//! [`Command`] and answers the view after it. Every refusal is a 422 with a message from
//! [`crate::messages`].
//!
//! The address is decided once. A term list is served at `/vocab/<slug>/`, with its namespace
//! either that address or a persistent one (w3id.org, purl.org) the admin redirects there. A
//! vocabulary's namespace is its own upstream column, so its address follows from it: a namespace
//! under this archive's `/vocab/` names the slug, and a persistent one takes a slug from the
//! request. The first publish fixes both.

use omeka_core::publication::{
    Refusal, SchemeKind, Snapshot, TermStates, archive_namespace, archive_slug, check, draft,
    is_persistent_namespace, is_slug, suggest_slug,
};
use omeka_db::repos::{NewPublicationVersion, PublicationRepo};
use serde::{Deserialize, Serialize};

use super::rdf::{self, Syntax, VersionMeta};
use super::source::{self, Upstream, VOCABULARY_CLASS};
use super::{Record, now, record, refused, validate_iris};
use crate::errors::ApiError;
use crate::messages;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error};
use crate::routes::Operation;
use crate::write::access;

/// Whether anything is published yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Draft,
    Published,
}

/// The admin's view of one term list or vocabulary.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub state: State,
    pub kind: SchemeKind,
    /// Why this cannot be published, when it cannot; the rest of the view is then empty.
    pub refusal: Option<String>,
    /// The path segment under `/vocab/`: fixed once published, suggested before.
    pub slug: String,
    /// Whether the slug is fixed, by a publish or by the vocabulary's namespace.
    pub slug_fixed: bool,
    pub namespace: String,
    /// Whether the namespace is fixed, by a publish or by the vocabulary's own column.
    pub namespace_fixed: bool,
    /// Where this archive serves it.
    pub url: String,
    pub versions: Vec<VersionView>,
    /// Whether the draft differs from the newest version, so a publish would write a new one.
    pub changed: bool,
    pub terms: Vec<TermView>,
    /// The Turtle the next publish would serve, or the newest version's when nothing changed.
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionView {
    pub version: u32,
    pub created: String,
    pub terms: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TermView {
    pub key: String,
    pub label: String,
    pub uri: String,
    /// Whether a version carries this term, so it can no longer be removed.
    pub published: bool,
    pub deprecated: bool,
    /// Whether a version carries the deprecation, so it can no longer be taken back.
    pub deprecation_published: bool,
    pub exact_match: Option<String>,
    pub close_match: Option<String>,
    /// The key of the term a merge moved this one's values to.
    pub replaced_by: Option<String>,
}

/// One `POST` body.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Write the next version. `slug` and `namespace` only count at the first publish.
    Publish {
        #[serde(default)]
        slug: Option<String>,
        #[serde(default)]
        namespace: Option<String>,
    },
    /// Mark a published term deprecated from the next version on.
    Deprecate { key: String },
    /// Take back a deprecation no version carries yet.
    Undeprecate { key: String },
    /// Set or clear a term list term's links to an authority.
    Link {
        key: String,
        #[serde(default)]
        exact_match: Option<String>,
        #[serde(default)]
        close_match: Option<String>,
    },
}

/// Where a publication is served, and whether the request may still choose it.
struct Address {
    slug: String,
    slug_fixed: bool,
    namespace: String,
    namespace_fixed: bool,
}

/// `GET .../publication`.
pub async fn view(api: &Api<'_>, resource: &str, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let upstream = source::load(api, resource, raw_id).await?;
    let record = record(api.pool(), upstream.target()).await?;
    answer(&build_view(api, &upstream, &record).await?)
}

/// `POST .../publication`.
pub async fn command(
    api: &Api<'_>,
    resource: &str,
    raw_id: &str,
    bytes: &[u8],
) -> Result<ApiResponse, ApiError> {
    let upstream = source::load(api, resource, raw_id).await?;
    authorize(api, &upstream)?;
    let command: Command =
        serde_json::from_slice(bytes).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let record = record(api.pool(), upstream.target()).await?;
    match command {
        Command::Publish { slug, namespace } => {
            publish(
                api,
                &upstream,
                &record,
                slug.as_deref(),
                namespace.as_deref(),
            )
            .await?;
        }
        Command::Deprecate { key } => set_deprecated(api, &upstream, record, &key, true).await?,
        Command::Undeprecate { key } => set_deprecated(api, &upstream, record, &key, false).await?,
        Command::Link {
            key,
            exact_match,
            close_match,
        } => link(api, &upstream, record, &key, exact_match, close_match).await?,
    }
    let record = super::record(api.pool(), upstream.target()).await?;
    answer(&build_view(api, &upstream, &record).await?)
}

fn answer(view: &View) -> Result<ApiResponse, ApiError> {
    Ok(ApiResponse::document(serialize(view)?))
}

/// The caller must be able to update the list or vocabulary itself.
fn authorize(api: &Api<'_>, upstream: &Upstream) -> Result<(), ApiError> {
    match upstream {
        Upstream::TermList(row) => {
            access::adapter(api.actor(), "custom_vocabs", Operation::Update)?;
            crate::write::term_lists::owner_only(api, row, Operation::Update)
        }
        Upstream::Vocabulary(row) => {
            access::adapter(api.actor(), "vocabularies", Operation::Update)?;
            access::vocabulary_entity(api.actor(), VOCABULARY_CLASS, row.owner_id)
        }
    }
}

/// The address a view shows, or the refusal that keeps it from being published at all.
fn address(api: &Api<'_>, upstream: &Upstream, record: &Record) -> Result<Address, ApiError> {
    if let Some((slug, namespace)) = record
        .row
        .as_ref()
        .and_then(|row| Some((row.slug.clone()?, row.namespace_uri.clone()?)))
    {
        return Ok(Address {
            slug,
            slug_fixed: true,
            namespace,
            namespace_fixed: true,
        });
    }
    let base = api.base().as_str();
    match upstream {
        Upstream::TermList(row) => {
            let mut slug = suggest_slug(&row.label);
            if slug.is_empty() {
                slug = format!("list-{}", row.id);
            }
            Ok(Address {
                namespace: archive_namespace(base, &slug),
                slug,
                slug_fixed: false,
                namespace_fixed: false,
            })
        }
        Upstream::Vocabulary(row) => {
            if let Some(slug) = archive_slug(base, &row.namespace_uri) {
                return Ok(Address {
                    slug: slug.to_string(),
                    slug_fixed: true,
                    namespace: row.namespace_uri.clone(),
                    namespace_fixed: true,
                });
            }
            if is_persistent_namespace(&row.namespace_uri) {
                return Ok(Address {
                    slug: suggest_slug(&row.prefix),
                    slug_fixed: false,
                    namespace: row.namespace_uri.clone(),
                    namespace_fixed: true,
                });
            }
            Err(refused(
                "o:namespace_uri",
                messages::publication_foreign_namespace(&row.namespace_uri),
            ))
        }
    }
}

/// The address a publish uses: the view's, with the request's choices where it may still choose.
async fn chosen_address(
    api: &Api<'_>,
    upstream: &Upstream,
    record: &Record,
    slug: Option<&str>,
    namespace: Option<&str>,
) -> Result<Address, ApiError> {
    let mut address = address(api, upstream, record)?;
    if let Some(slug) = slug.filter(|slug| *slug != address.slug) {
        if address.slug_fixed {
            return Err(refused(
                "slug",
                messages::PUBLICATION_ADDRESS_FIXED.to_string(),
            ));
        }
        if !is_slug(slug) {
            return Err(refused(
                "slug",
                messages::PUBLICATION_SLUG_INVALID.to_string(),
            ));
        }
        address.slug = slug.to_string();
        if !address.namespace_fixed {
            address.namespace = archive_namespace(api.base().as_str(), slug);
        }
    }
    if let Some(namespace) = namespace.filter(|namespace| *namespace != address.namespace) {
        if address.namespace_fixed {
            return Err(refused(
                "namespace",
                messages::PUBLICATION_ADDRESS_FIXED.to_string(),
            ));
        }
        if !is_persistent_namespace(namespace) {
            return Err(refused(
                "namespace",
                messages::PUBLICATION_NAMESPACE_NOT_PERSISTENT.to_string(),
            ));
        }
        address.namespace = namespace.to_string();
    }
    if !is_slug(&address.slug) {
        return Err(refused(
            "slug",
            messages::PUBLICATION_SLUG_INVALID.to_string(),
        ));
    }
    let holder = PublicationRepo::new(api.pool())
        .by_slug(&address.slug)
        .await
        .map_err(db_error)?;
    if holder.is_some_and(|row| Some(row.id) != record.row.as_ref().map(|own| own.id)) {
        return Err(refused(
            "slug",
            messages::publication_slug_taken(&address.slug),
        ));
    }
    Ok(address)
}

async fn publish(
    api: &Api<'_>,
    upstream: &Upstream,
    record: &Record,
    slug: Option<&str>,
    namespace: Option<&str>,
) -> Result<(), ApiError> {
    let source = source::source(api, upstream).await?;
    let address = chosen_address(api, upstream, record, slug, namespace).await?;
    let (next, states) = draft(&source, &address.namespace, &record.states);
    validate_iris(&next)?;
    if let Some(latest) = &record.latest {
        check(&latest.snapshot, &next).map_err(refusal)?;
        if latest.snapshot == next {
            return Ok(());
        }
    }
    let terms = serde_json::to_value(&states).map_err(ApiError::internal)?;
    let snapshot = serde_json::to_value(&next).map_err(ApiError::internal)?;
    PublicationRepo::new(api.pool())
        .publish(
            upstream.target(),
            &NewPublicationVersion {
                slug: &address.slug,
                namespace_uri: &address.namespace,
                terms: &terms,
                snapshot: &snapshot,
            },
            now(),
        )
        .await
        .map_err(db_error)?;
    Ok(())
}

/// A rule refusal as the 422 the write paths and the publish command answer.
pub(super) fn refusal(refusal: Refusal) -> ApiError {
    match refusal {
        Refusal::Removed(keys) => refused(
            "o:terms",
            messages::publication_terms_removed(
                keys.iter()
                    .map(|key| format!("\"{key}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ),
        Refusal::NamespaceChanged => refused(
            "o:namespace_uri",
            messages::PUBLICATION_NAMESPACE_FIXED.to_string(),
        ),
        Refusal::Undeprecated(key) => refused("key", messages::publication_deprecation_fixed(key)),
    }
}

async fn set_deprecated(
    api: &Api<'_>,
    upstream: &Upstream,
    mut record: Record,
    key: &str,
    deprecated: bool,
) -> Result<(), ApiError> {
    let published = record
        .latest
        .as_ref()
        .and_then(|latest| latest.snapshot.terms.iter().find(|term| term.key == key));
    let Some(published) = published else {
        let source = source::source(api, upstream).await?;
        return Err(if source.terms.iter().any(|term| term.key == key) {
            refused("key", messages::publication_term_not_published(key))
        } else {
            refused("key", messages::publication_unknown_term(key))
        });
    };
    if published.deprecated && !deprecated {
        return Err(refused("key", messages::publication_deprecation_fixed(key)));
    }
    let state = record.states.entry(key.to_string()).or_default();
    state.deprecated = deprecated;
    if !deprecated {
        state.replaced_by = None;
    }
    save(api, upstream, &record.states).await
}

async fn link(
    api: &Api<'_>,
    upstream: &Upstream,
    mut record: Record,
    key: &str,
    exact_match: Option<String>,
    close_match: Option<String>,
) -> Result<(), ApiError> {
    let source = source::source(api, upstream).await?;
    if source.kind != SchemeKind::TermList {
        return Err(refused(
            "key",
            messages::PUBLICATION_MATCH_TERM_LISTS.to_string(),
        ));
    }
    if !source.terms.iter().any(|term| term.key == key) {
        return Err(refused("key", messages::publication_unknown_term(key)));
    }
    let exact_match = match_uri("exact_match", exact_match)?;
    let close_match = match_uri("close_match", close_match)?;
    let state = record.states.entry(key.to_string()).or_default();
    state.exact_match = exact_match;
    state.close_match = close_match;
    save(api, upstream, &record.states).await
}

/// A match as the command sends it: blank clears it, anything else must be an absolute
/// `http` or `https` IRI.
fn match_uri(key: &str, raw: Option<String>) -> Result<Option<String>, ApiError> {
    let Some(uri) = raw
        .map(|uri| uri.trim().to_string())
        .filter(|uri| !uri.is_empty())
    else {
        return Ok(None);
    };
    let web = uri.starts_with("http://") || uri.starts_with("https://");
    if web && oxrdf::NamedNode::new(&uri).is_ok() {
        Ok(Some(uri))
    } else {
        Err(refused(
            key,
            messages::PUBLICATION_MATCH_INVALID.to_string(),
        ))
    }
}

async fn save(api: &Api<'_>, upstream: &Upstream, states: &TermStates) -> Result<(), ApiError> {
    let terms = serde_json::to_value(states).map_err(ApiError::internal)?;
    PublicationRepo::new(api.pool())
        .save_terms(upstream.target(), &terms)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn build_view(api: &Api<'_>, upstream: &Upstream, record: &Record) -> Result<View, ApiError> {
    let kind = match upstream {
        Upstream::TermList(_) => SchemeKind::TermList,
        Upstream::Vocabulary(_) => SchemeKind::Vocabulary,
    };
    let state = if record.latest.is_some() {
        State::Published
    } else {
        State::Draft
    };
    let ready = match address(api, upstream, record) {
        Ok(address) => source::source(api, upstream)
            .await
            .map(|source| (address, source)),
        Err(error) => Err(error),
    };
    let (address, source) = match ready {
        Ok(ready) => ready,
        Err(ApiError::Validation(errors)) => {
            return Ok(refused_view(
                kind,
                state,
                errors.into_values().flatten().next(),
            ));
        }
        Err(error) => return Err(error),
    };
    let (next, _) = draft(&source, &address.namespace, &record.states);
    let latest = record.latest.as_ref();
    let versions = match &record.row {
        Some(row) => PublicationRepo::new(api.pool())
            .versions(row.id)
            .await
            .map_err(db_error)?
            .iter()
            .map(|version| {
                Ok(VersionView {
                    version: version.version,
                    created: version.created.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    terms: super::published(version)?.snapshot.terms.len(),
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?,
        None => Vec::new(),
    };
    let changed = latest.is_none_or(|latest| latest.snapshot != next);
    let preview_meta = match latest {
        Some(latest) if !changed => latest.meta,
        _ => VersionMeta {
            number: latest.map_or(1, |latest| latest.meta.number + 1),
            created: now(),
        },
    };
    let preview = rdf::serialize(&rdf::graph(&next, preview_meta), Syntax::Turtle)
        .map_err(ApiError::internal)
        .and_then(|bytes| String::from_utf8(bytes).map_err(ApiError::internal))?;
    Ok(View {
        state,
        kind,
        refusal: None,
        url: format!(
            "{}/vocab/{}/",
            api.base().as_str().trim_end_matches('/'),
            address.slug
        ),
        slug: address.slug,
        slug_fixed: address.slug_fixed,
        namespace: address.namespace,
        namespace_fixed: address.namespace_fixed,
        versions,
        changed,
        terms: term_views(&next, latest.map(|latest| &latest.snapshot)),
        preview,
    })
}

fn term_views(next: &Snapshot, latest: Option<&Snapshot>) -> Vec<TermView> {
    next.terms
        .iter()
        .map(|term| {
            let published = latest.and_then(|latest| {
                latest
                    .terms
                    .iter()
                    .find(|candidate| candidate.key == term.key)
            });
            TermView {
                key: term.key.clone(),
                label: term.label.clone(),
                uri: next.term_iri(term),
                published: published.is_some(),
                deprecated: term.deprecated,
                deprecation_published: published.is_some_and(|published| published.deprecated),
                exact_match: term.exact_match.clone(),
                close_match: term.close_match.clone(),
                replaced_by: term
                    .replaced_by
                    .as_ref()
                    .and_then(|local| next.term(local).map(|replacement| replacement.key.clone())),
            }
        })
        .collect()
}

fn refused_view(kind: SchemeKind, state: State, reason: Option<String>) -> View {
    View {
        state,
        kind,
        refusal: reason,
        slug: String::new(),
        slug_fixed: false,
        namespace: String::new(),
        namespace_fixed: false,
        url: String::new(),
        versions: Vec::new(),
        changed: false,
        terms: Vec::new(),
        preview: String::new(),
    }
}
