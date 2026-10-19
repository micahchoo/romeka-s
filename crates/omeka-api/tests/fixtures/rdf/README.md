# RDF vocabulary fixtures

The files the vocabulary importer reads in `tests/vocab_import_oracle.rs`, and upstream's
answer for each one under `oracle/`.

| File | Origin |
| --- | --- |
| `dcterms.rdf`, `dctype.rdf`, `bibo.owl`, `foaf.rdf` | upstream `application/data/vocabularies/` at the frozen SHA, the four files `InstallDefaultVocabulariesTask` imports |
| `dctype.ttl`, `dctype.nt`, `dctype.jsonld` | `dctype.rdf` re-serialised by upstream's EasyRdf (`Graph::serialise`) |
| `skos.rdf` | `https://www.w3.org/2009/08/skos-reference/skos.rdf`, fetched 2026-10-18 |
| `dcat.ttl` | `https://www.w3.org/ns/dcat.ttl`, fetched 2026-10-18; labels in many languages |
| `lang.ttl` | written for these tests: language preference, the label fallbacks, the namespace test and the type list |

Each `oracle/<case>.json` is `Omeka\Stdlib\RdfImporter::getMembers('file', ...)` run in the
upstream container on the same file, with the case's format and options, printed as
`{classes: [{local_name, label, comment}], properties: [...]}` in upstream's order. The test
names each case's options. To measure a case again:

```
docker exec omeka-oracle-app-1 mkdir -p /tmp/rdf-oracle
docker cp oracle/members.php omeka-oracle-app-1:/tmp/rdf-oracle/
docker cp <file> omeka-oracle-app-1:/tmp/rdf-oracle/
docker exec omeka-oracle-app-1 php /tmp/rdf-oracle/members.php /tmp/rdf-oracle/<file> <namespace> <format> [lang] [label_property] [comment_property]
```

`members.php` builds an `RdfImporter` without its constructor (`getMembers` reads no service)
and prints the members as JSON; it writes nothing to the upstream database.
