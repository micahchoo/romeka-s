#!/usr/bin/env python3
"""Generate crates/omeka-cli/data/default-vocabularies.json from the upstream RDF files.

This mirrors Omeka\\Stdlib\\RdfImporter::getMembersOfTypes at omeka-s 5e413f0:
a term is imported when its rdf:type is one of the class or property types, and
when its URI is exactly the vocabulary namespace plus the local name. Labels
come from the first of skos:prefLabel, rdfs:label, foaf:name, rss:title,
dc:title; comments from rdfs:comment.

Usage: generate-default-vocabularies.py <vocabularies-dir> <out.json>
"""
import json
import sys
import xml.etree.ElementTree as ET

RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
RDFS = "http://www.w3.org/2000/01/rdf-schema#"
OWL = "http://www.w3.org/2002/07/owl#"
SKOS = "http://www.w3.org/2004/02/skos/core#"
FOAF = "http://xmlns.com/foaf/0.1/"
RSS = "http://purl.org/rss/1.0/"
DC = "http://purl.org/dc/elements/1.1/"

CLASS_TYPES = {RDFS + "Class", OWL + "Class"}
PROPERTY_TYPES = {
    RDF + "Property",
    OWL + "ObjectProperty",
    OWL + "DatatypeProperty",
    OWL + "SymmetricProperty",
    OWL + "TransitiveProperty",
    OWL + "FunctionalProperty",
    OWL + "InverseFunctionalProperty",
}
LABEL_PROPERTIES = [SKOS + "prefLabel", RDFS + "label", FOAF + "name", RSS + "title", DC + "title"]
COMMENT_PROPERTIES = [RDFS + "comment"]

# InstallDefaultVocabulariesTask::$vocabularies (label, comment) plus the file each imports.
VOCABULARIES = [
    ("dcterms.rdf", "dcterms", "http://purl.org/dc/terms/", "Dublin Core",
     "Basic resource metadata (DCMI Metadata Terms)"),
    ("dctype.rdf", "dctype", "http://purl.org/dc/dcmitype/", "Dublin Core Type",
     "Basic resource types (DCMI Type Vocabulary)"),
    ("bibo.owl", "bibo", "http://purl.org/ontology/bibo/", "Bibliographic Ontology",
     "Bibliographic metadata (BIBO)"),
    ("foaf.rdf", "foaf", "http://xmlns.com/foaf/0.1/", "Friend of a Friend",
     "Relationships between people and organizations (FOAF)"),
]


def uri(tag):
    return tag[1:].replace("}", "", 1) if tag.startswith("{") else tag


def first_literal(element, properties):
    for prop in properties:
        for child in element:
            if uri(child.tag) == prop:
                value = (child.text or "").strip()
                if value:
                    return value
    return None


def members(path, namespace):
    classes = {}
    properties = {}
    seen_classes = set()
    seen_properties = set()
    for element in ET.parse(path).getroot():
        about = element.get("{" + RDF + "}about")
        if not about or not about.startswith(namespace):
            continue
        local = about[len(namespace):]
        if not local or "/" in local or "#" in local:
            continue
        types = set()
        own = uri(element.tag)
        if own in CLASS_TYPES or own in PROPERTY_TYPES:
            types.add(own)
        for child in element:
            if uri(child.tag) == RDF + "type":
                types.add(child.get("{" + RDF + "}resource"))
        info = {
            "local_name": local,
            "label": first_literal(element, LABEL_PROPERTIES) or local,
            "comment": first_literal(element, COMMENT_PROPERTIES),
        }
        # The stored unique key is (vocabulary_id, local_name) under a
        # case-insensitive collation, so foaf's `givenName` and `givenname`
        # collide. Keep the first in document order, as the database would.
        if types & CLASS_TYPES and local.lower() not in seen_classes:
            seen_classes.add(local.lower())
            classes[local] = info
        if types & PROPERTY_TYPES and local.lower() not in seen_properties:
            seen_properties.add(local.lower())
            properties[local] = info
    return classes, properties


def main():
    directory, out = sys.argv[1], sys.argv[2]
    vocabularies = []
    for filename, prefix, namespace, label, comment in VOCABULARIES:
        classes, properties = members(f"{directory}/{filename}", namespace)
        vocabularies.append({
            "prefix": prefix,
            "namespace_uri": namespace,
            "label": label,
            "comment": comment,
            "classes": list(classes.values()),
            "properties": list(properties.values()),
        })
    # One term per line, so the committed data stays reviewable.
    term = lambda obj: json.dumps(obj, ensure_ascii=False, separators=(",", ":"))
    lines = ["{", ' "vocabularies": [']
    for index, vocab in enumerate(vocabularies):
        comma = "," if index + 1 < len(vocabularies) else ""
        head = {k: vocab[k] for k in ("prefix", "namespace_uri", "label", "comment")}
        lines.append("   " + term(head)[:-1] + ",")
        for key in ("classes", "properties"):
            lines.append(f'   "{key}": [')
            entries = vocab[key]
            for position, entry in enumerate(entries):
                tail = "," if position + 1 < len(entries) else ""
                lines.append("    " + term(entry) + tail)
            lines.append("   ]" + ("," if key == "classes" else ""))
        lines.append("  }" + comma)
    lines.append(" ]")
    lines.append("}")
    with open(out, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
