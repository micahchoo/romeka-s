//! Publishing term lists and vocabularies at permanent addresses. Romeka's own feature.
//!
//! A term list publishes as a SKOS concept scheme and a vocabulary as an RDFS/OWL ontology, each
//! under `/vocab/<slug>/` with a URI per term. The rules live in `omeka_core::publication`; this
//! module connects them to the stored rows and the wire:
//!
//! - [`admin`]: `GET` and `POST /api/{custom_vocabs|vocabularies}/{id}/publication`, the admin's
//!   view of the draft, the versions and the Turtle preview, and the publish, deprecate and link
//!   commands.
//! - [`public`]: `GET /vocab/<slug>/[<local>]`, one version of a scheme or a term in the syntax the
//!   request negotiates ([`negotiate`]): Turtle, JSON-LD, RDF/XML, N-Triples ([`rdf`]) or a page
//!   ([`html`]).
//! - [`guard`]: the check the upstream write paths run, so a `PUT`, `PATCH` or `DELETE` on a
//!   published list or vocabulary cannot drop, rename or move a published term (a 422).
//!
//! The state is in Romeka's own tables (`romeka_publication`, migration 0003); the upstream rows are
//! read, never extended, so an Omeka S install reads them unchanged.

pub mod admin;
pub(crate) mod guard;
pub mod html;
pub mod negotiate;
pub mod public;
pub mod rdf;
mod source;

use chrono::{NaiveDateTime, Timelike};
use omeka_core::publication::{Snapshot, TermStates};
use omeka_db::DbPool;
use omeka_db::repos::PublicationRepo;
use omeka_db::rows::{PublicationRow, PublicationTarget, PublicationVersionRow};
use omeka_db::with_db;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::db_error;
use rdf::VersionMeta;

/// What is stored for one target: the record, its term state and its newest version.
struct Record {
    row: Option<PublicationRow>,
    states: TermStates,
    latest: Option<Published>,
}

/// One stored version, parsed.
struct Published {
    snapshot: Snapshot,
    meta: VersionMeta,
}

async fn record(pool: &DbPool, target: PublicationTarget) -> Result<Record, ApiError> {
    let Some(row) =
        with_db!(pool, |db| PublicationRepo::new(db).get(target).await).map_err(db_error)?
    else {
        return Ok(Record {
            row: None,
            states: TermStates::new(),
            latest: None,
        });
    };
    let latest = with_db!(pool, |db| PublicationRepo::new(db).latest(row.id).await)
        .map_err(db_error)?
        .map(|version| published(&version))
        .transpose()?;
    Ok(Record {
        states: states_of(&row)?,
        row: Some(row),
        latest,
    })
}

/// The term state column, parsed. Only this module writes it, so a shape it cannot read is a
/// storage fault.
fn states_of(row: &PublicationRow) -> Result<TermStates, ApiError> {
    serde_json::from_value(row.terms.clone()).map_err(ApiError::internal)
}

fn published(version: &PublicationVersionRow) -> Result<Published, ApiError> {
    Ok(Published {
        snapshot: serde_json::from_value(version.snapshot.clone()).map_err(ApiError::internal)?,
        meta: VersionMeta {
            number: version.version,
            created: version.created,
        },
    })
}

/// Now, to the second, as a version's `created` stores it.
fn now() -> NaiveDateTime {
    let now = chrono::Utc::now().naive_utc();
    now.with_nanosecond(0).unwrap_or(now)
}

/// A 422 with one message under one key.
fn refused(key: &str, message: String) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert(key.to_string(), vec![message]);
    ApiError::Validation(errors)
}

/// Every IRI a snapshot will serve, checked once before it is published.
fn validate_iris(snapshot: &Snapshot) -> Result<(), ApiError> {
    let iris = std::iter::once(snapshot.scheme_iri().to_string())
        .chain(snapshot.terms.iter().map(|term| snapshot.term_iri(term)));
    for iri in iris {
        if oxrdf::NamedNode::new(&iri).is_err() {
            return Err(refused(
                "o:local_name",
                messages::publication_term_iri_invalid(iri),
            ));
        }
    }
    Ok(())
}
