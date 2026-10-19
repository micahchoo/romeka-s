//! Content negotiation for a published scheme or term: which representation a request gets.
//!
//! A `format` query parameter wins, so a plain link can name a syntax. Otherwise the `Accept`
//! header decides by quality, with the most specific matching range setting each candidate's
//! quality (RFC 9110, section 12.5.1). A tie goes to the earlier candidate in [`CANDIDATES`], so a
//! client that accepts anything (`*/*`) gets Turtle and a browser, which names `text/html`, gets
//! the page.

use super::rdf::Syntax;

/// What a request is answered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Representation {
    Rdf(Syntax),
    Html,
}

impl Representation {
    #[must_use]
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Rdf(syntax) => syntax.media_type(),
            Self::Html => "text/html",
        }
    }
}

/// Every representation in tie-break order, with the media types that name it.
const CANDIDATES: &[(Representation, &[&str])] = &[
    (Representation::Rdf(Syntax::Turtle), &["text/turtle"]),
    (
        Representation::Rdf(Syntax::JsonLd),
        &["application/ld+json", "application/json"],
    ),
    (
        Representation::Rdf(Syntax::RdfXml),
        &["application/rdf+xml"],
    ),
    (
        Representation::Rdf(Syntax::NTriples),
        &["application/n-triples"],
    ),
    (
        Representation::Html,
        &["text/html", "application/xhtml+xml"],
    ),
];

/// The `format` parameter values and what each names.
const FORMATS: &[(&str, Representation)] = &[
    ("ttl", Representation::Rdf(Syntax::Turtle)),
    ("jsonld", Representation::Rdf(Syntax::JsonLd)),
    ("rdf", Representation::Rdf(Syntax::RdfXml)),
    ("nt", Representation::Rdf(Syntax::NTriples)),
    ("html", Representation::Html),
];

/// The `format` value that names a representation, for links.
#[must_use]
pub fn format_name(representation: Representation) -> &'static str {
    FORMATS
        .iter()
        .find(|(_, candidate)| *candidate == representation)
        .map_or("ttl", |(name, _)| name)
}

/// Every representation with its `format` value, in the order a page lists them.
#[must_use]
pub fn formats() -> &'static [(&'static str, Representation)] {
    FORMATS
}

/// The representation a request gets, or `None` when it accepts none of them (a 406).
#[must_use]
pub fn choose(format: Option<&str>, accept: Option<&str>) -> Option<Representation> {
    if let Some(name) = format {
        return FORMATS
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, representation)| *representation);
    }
    let Some(accept) = accept.filter(|header| !header.trim().is_empty()) else {
        return Some(CANDIDATES[0].0);
    };
    let ranges: Vec<(String, f32)> = accept.split(',').filter_map(range).collect();
    let mut best: Option<(Representation, f32)> = None;
    for (representation, types) in CANDIDATES {
        let quality = types
            .iter()
            .filter_map(|media_type| quality(&ranges, media_type))
            .fold(0.0_f32, f32::max);
        if quality > 0.0 && best.is_none_or(|(_, chosen)| quality > chosen) {
            best = Some((*representation, quality));
        }
    }
    best.map(|(representation, _)| representation)
}

/// One `Accept` entry: the lowercased range and its quality.
fn range(entry: &str) -> Option<(String, f32)> {
    let mut parts = entry.split(';');
    let media = parts.next()?.trim().to_ascii_lowercase();
    if media.is_empty() {
        return None;
    }
    let mut q = 1.0_f32;
    for parameter in parts {
        if let Some((name, value)) = parameter.split_once('=')
            && name.trim().eq_ignore_ascii_case("q")
        {
            q = value.trim().parse().unwrap_or(0.0);
        }
    }
    Some((media, q.clamp(0.0, 1.0)))
}

/// The quality the most specific matching range gives a media type, `None` when none matches.
fn quality(ranges: &[(String, f32)], media_type: &str) -> Option<f32> {
    let main = media_type.split('/').next().unwrap_or(media_type);
    let wildcard = format!("{main}/*");
    [media_type, wildcard.as_str(), "*/*"]
        .iter()
        .find_map(|wanted| {
            ranges
                .iter()
                .filter(|(range, _)| range == wanted)
                .map(|(_, q)| *q)
                .reduce(f32::max)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TURTLE: Representation = Representation::Rdf(Syntax::Turtle);
    const JSON_LD: Representation = Representation::Rdf(Syntax::JsonLd);
    const RDF_XML: Representation = Representation::Rdf(Syntax::RdfXml);

    #[test]
    fn a_client_that_names_nothing_or_anything_gets_turtle() {
        assert_eq!(choose(None, None), Some(TURTLE));
        assert_eq!(choose(None, Some("*/*")), Some(TURTLE));
    }

    #[test]
    fn a_browser_gets_the_page() {
        let firefox = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
        assert_eq!(choose(None, Some(firefox)), Some(Representation::Html));
    }

    #[test]
    fn the_highest_quality_wins_and_a_tie_keeps_the_server_order() {
        assert_eq!(
            choose(None, Some("text/turtle;q=0.5, application/ld+json")),
            Some(JSON_LD)
        );
        assert_eq!(
            choose(None, Some("application/rdf+xml, text/turtle")),
            Some(TURTLE)
        );
        assert_eq!(choose(None, Some("application/rdf+xml")), Some(RDF_XML));
    }

    #[test]
    fn a_specific_range_overrides_a_wildcard() {
        assert_eq!(
            choose(None, Some("text/*, text/turtle;q=0")),
            Some(Representation::Html)
        );
    }

    #[test]
    fn nothing_acceptable_is_none() {
        assert_eq!(choose(None, Some("image/png")), None);
        assert_eq!(choose(None, Some("*/*;q=0")), None);
    }

    #[test]
    fn the_format_parameter_wins_over_the_header() {
        assert_eq!(choose(Some("rdf"), Some("text/html")), Some(RDF_XML));
        assert_eq!(choose(Some("pdf"), None), None);
    }
}
