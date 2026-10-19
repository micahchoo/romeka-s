//! `GET /vocab/<slug>/[<local>]`: a published scheme or one of its terms, for anyone.
//!
//! The newest version answers unless the query names one (`?version=2`). The representation is
//! negotiated ([`super::negotiate`]); every answer varies on `Accept` and may be read from any
//! origin, because the point of publishing is that other sites and tools use it.

use http::StatusCode;
use omeka_db::repos::PublicationRepo;
use sqlx::SqlitePool;

use super::html::{self, PageAddress};
use super::negotiate::{self, Representation};
use super::rdf;
use super::{Published, published};
use crate::read::Params;

/// One answer, ready for the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    pub status: StatusCode,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Served {
    fn text(status: StatusCode, body: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }
}

/// A request for a published scheme, `local` naming one of its terms.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub slug: &'a str,
    pub local: Option<&'a str>,
    pub query: &'a str,
    pub accept: Option<&'a str>,
}

/// Answer one request. A slug nothing is published under, a version or a term that does not
/// exist, is a 404; an `Accept` header that names no representation this serves is a 406.
pub async fn serve(pool: &SqlitePool, request: Request<'_>) -> Served {
    match answer(pool, request).await {
        Ok(served) => served,
        Err(served) => served,
    }
}

async fn answer(pool: &SqlitePool, request: Request<'_>) -> Result<Served, Served> {
    let not_found = || Served::text(StatusCode::NOT_FOUND, "Not Found");
    let failed = || Served::text(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
    let params = Params::parse(request.query);
    let representation = negotiate::choose(params.get("format"), request.accept).ok_or_else(|| {
        Served::text(
            StatusCode::NOT_ACCEPTABLE,
            "Not Acceptable: text/turtle, application/ld+json, application/rdf+xml, application/n-triples or text/html",
        )
    })?;
    let repo = PublicationRepo::new(pool);
    let record = repo
        .by_slug(request.slug)
        .await
        .map_err(|_| failed())?
        .ok_or_else(not_found)?;
    let versions = repo.versions(record.id).await.map_err(|_| failed())?;
    let wanted = match params.get("version") {
        Some(raw) => Some(raw.parse::<u32>().map_err(|_| not_found())?),
        None => None,
    };
    let row = match wanted {
        Some(number) => versions.iter().find(|version| version.version == number),
        None => versions.last(),
    }
    .ok_or_else(not_found)?;
    let Published { snapshot, meta } = published(row).map_err(|_| failed())?;
    let term = match request.local {
        Some(local) => Some(snapshot.term(local).ok_or_else(not_found)?),
        None => None,
    };
    let scheme_path = format!("/vocab/{}", request.slug);
    let address = PageAddress {
        scheme_path: &scheme_path,
        version: wanted,
    };
    let body = match (representation, term) {
        (Representation::Html, None) => {
            let numbers: Vec<u32> = versions.iter().map(|version| version.version).collect();
            html::scheme_page(&snapshot, meta, &numbers, address).into_bytes()
        }
        (Representation::Html, Some(term)) => {
            html::term_page(&snapshot, term, meta, address).into_bytes()
        }
        (Representation::Rdf(syntax), None) => {
            rdf::serialize(&rdf::graph(&snapshot, meta), syntax).map_err(|_| failed())?
        }
        (Representation::Rdf(syntax), Some(term)) => {
            rdf::serialize(&rdf::term_graph(&snapshot, term), syntax).map_err(|_| failed())?
        }
    };
    Ok(Served {
        status: StatusCode::OK,
        content_type: content_type(representation),
        body,
    })
}

fn content_type(representation: Representation) -> &'static str {
    match representation {
        Representation::Html => "text/html; charset=utf-8",
        Representation::Rdf(rdf::Syntax::Turtle) => "text/turtle; charset=utf-8",
        Representation::Rdf(rdf::Syntax::NTriples) => "application/n-triples; charset=utf-8",
        Representation::Rdf(syntax) => syntax.media_type(),
    }
}
