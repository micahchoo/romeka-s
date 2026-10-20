//! An XMP packet's Dublin Core, Photoshop and Lightroom properties.
//!
//! Properties are found by namespace, not prefix, in both forms XMP allows:
//! an element (`<dc:subject><rdf:Bag><rdf:li>…`) and an attribute of
//! `rdf:Description` (`photoshop:DateCreated="1950-03-04"`). A bag or a
//! sequence gives every item; a language alternative gives one value, the
//! `x-default` item, else the first.

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;

use super::{Metadata, decode_text};

const DC: &[u8] = b"http://purl.org/dc/elements/1.1/";
const PHOTOSHOP: &[u8] = b"http://ns.adobe.com/photoshop/1.0/";
const LIGHTROOM: &[u8] = b"http://ns.adobe.com/lightroom/1.0/";
const RDF: &[u8] = b"http://www.w3.org/1999/02/22-rdf-syntax-ns#";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A language alternative: one value.
    Alt,
    /// A bag or a sequence: every item.
    List,
    /// A plain text value.
    Simple,
}

const PROPERTIES: [(&[u8], &[u8], &str, Shape); 7] = [
    (DC, b"title", "XMP dc:title", Shape::Alt),
    (DC, b"description", "XMP dc:description", Shape::Alt),
    (DC, b"creator", "XMP dc:creator", Shape::List),
    (DC, b"subject", "XMP dc:subject", Shape::List),
    (DC, b"rights", "XMP dc:rights", Shape::Alt),
    (
        PHOTOSHOP,
        b"DateCreated",
        "XMP photoshop:DateCreated",
        Shape::Simple,
    ),
    (
        LIGHTROOM,
        b"hierarchicalSubject",
        "XMP lr:hierarchicalSubject",
        Shape::List,
    ),
];

/// What an element is, by its namespace and local name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Description,
    Item,
    Property(usize),
    Other,
}

fn property(namespace: &ResolveResult<'_>, local: &[u8]) -> Option<usize> {
    let ResolveResult::Bound(Namespace(namespace)) = namespace else {
        return None;
    };
    PROPERTIES
        .iter()
        .position(|(ns, name, ..)| ns == namespace && *name == local)
}

fn kind(namespace: &ResolveResult<'_>, event: &Event<'_>) -> Kind {
    let local = match event {
        Event::Start(start) | Event::Empty(start) => start.local_name(),
        Event::End(end) => end.local_name(),
        _ => return Kind::Other,
    };
    let rdf = matches!(namespace, ResolveResult::Bound(Namespace(ns)) if *ns == RDF);
    match local.as_ref() {
        b"Description" if rdf => Kind::Description,
        b"li" if rdf => Kind::Item,
        local => property(namespace, local).map_or(Kind::Other, Kind::Property),
    }
}

/// The property being read: its place in [`PROPERTIES`], the depth it
/// opened at, its items as (language, text), and the text outside items.
struct Open {
    property: usize,
    depth: usize,
    items: Vec<(Option<String>, String)>,
    item: Option<(Option<String>, String)>,
    text: String,
}

/// The properties of an XMP packet into `tags`. A packet that is not
/// UTF-8 is read as Windows-1252 and its values marked so; a packet that
/// is not XML gives what was read before the fault.
pub(super) fn read(packet: &[u8], tags: &mut Metadata) {
    let Some((packet, windows_1252)) = decode_text(packet) else {
        return;
    };
    let mut reader = NsReader::from_reader(packet.as_bytes());
    let mut found: Vec<(usize, Vec<String>)> = Vec::new();
    let mut open: Option<Open> = None;
    let mut depth = 0_usize;
    while let Ok((kind, event)) = reader
        .read_resolved_event()
        .map(|(namespace, event)| (kind(&namespace, &event), event))
    {
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) if open.is_none() => {
                match kind {
                    Kind::Description => attributes(&reader, &start, &mut found),
                    Kind::Property(property) if !empty => {
                        open = Some(Open {
                            property,
                            depth,
                            items: Vec::new(),
                            item: None,
                            text: String::new(),
                        });
                    }
                    _ => {}
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::Start(start) => {
                if let Some(open) = open.as_mut()
                    && kind == Kind::Item
                {
                    open.item = Some((language(&start), String::new()));
                }
                depth += 1;
            }
            Event::Text(text) => {
                if let (Some(open), Ok(text)) = (open.as_mut(), text.unescape()) {
                    match open.item.as_mut() {
                        Some((_, item)) => item.push_str(&text),
                        None => open.text.push_str(&text),
                    }
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                let Some(current) = open.as_mut() else {
                    continue;
                };
                if current.depth == depth {
                    if let Some(current) = open.take() {
                        found.push(finish(current));
                    }
                } else if kind == Kind::Item
                    && let Some(item) = current.item.take()
                {
                    current.items.push(item);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    for (property, values) in found {
        let values: Vec<String> = values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        if values.is_empty() {
            continue;
        }
        let entry = tags.entry(PROPERTIES[property].2).or_default();
        if entry.values.is_empty() {
            entry.values = values;
            entry.windows_1252 = windows_1252;
        }
    }
}

/// The properties written as attributes of an `rdf:Description`.
fn attributes(
    reader: &NsReader<&[u8]>,
    start: &BytesStart<'_>,
    found: &mut Vec<(usize, Vec<String>)>,
) {
    for attribute in start.attributes().flatten() {
        let (namespace, local) = reader.resolve_attribute(attribute.key);
        if let (Some(property), Ok(value)) = (
            property(&namespace, local.as_ref()),
            attribute.unescape_value(),
        ) {
            found.push((property, vec![value.into_owned()]));
        }
    }
}

fn language(start: &BytesStart<'_>) -> Option<String> {
    let attribute = start.try_get_attribute("xml:lang").ok()??;
    Some(attribute.unescape_value().ok()?.into_owned())
}

fn finish(open: Open) -> (usize, Vec<String>) {
    let shape = PROPERTIES[open.property].3;
    let values = match shape {
        _ if open.items.is_empty() => vec![open.text],
        Shape::Alt => {
            let chosen = open
                .items
                .iter()
                .position(|(lang, _)| lang.as_deref() == Some("x-default"))
                .unwrap_or(0);
            vec![
                open.items
                    .into_iter()
                    .nth(chosen)
                    .map(|(_, text)| text)
                    .unwrap_or_default(),
            ]
        }
        Shape::List | Shape::Simple => open.items.into_iter().map(|(_, text)| text).collect(),
    };
    (open.property, values)
}
