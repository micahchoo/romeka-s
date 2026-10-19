# Unit: publish (branch vocab/publish, CARGO_TARGET_DIR unit name: vocab-publish)

Read the brief head first: /tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/e64d830e-2bd4-4b59-9b52-2538fdd41bda/scratchpad/brief-head.md . Then /tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/e64d830e-2bd4-4b59-9b52-2538fdd41bda/scratchpad/i18n-note.txt and /tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/e64d830e-2bd4-4b59-9b52-2538fdd41bda/scratchpad/decisions.tsv (the "PROPOSED ...
publish" row is now APPROVED by the user; the "Publish routes" row constrains URLs).
Main now has term lists (custom_vocab, customvocab:<id>), Item types, a local vocabulary made by "Create a new
field" (prefix local, namespace <origin>/vocab/local/), the Standards page, nine locales.

## Why
Advanced users want their own term lists and vocabularies to have permanent web addresses that machines and people
can read. Publishing is a promise: once others use a term, it must stay resolvable.

## Design constraints (decide the rest; design it twice per the architect principle and record the choice)
- Two states: draft and published. A draft changes freely. Once published, terms can only be ADDED or DEPRECATED
  (owl:deprecated true), never deleted or renamed; each publish records a version (a snapshot you can serve).
  Enforce in the API (a refused delete/rename is a 422 with a plain message); this is a Romeka-only rule: add it to
  docs/compat.md. Upstream must still read the tables unchanged: keep Romeka state in a NEW table (migration 0003,
  SQLite and MySQL, tested on both like 0002 was) rather than new columns on upstream tables.
- Term lists publish as SKOS (skos:ConceptScheme + skos:Concept, prefLabel with language, a URI per term);
  vocabularies publish as RDFS/OWL (rdfs:Class, rdf:Property, labels, comments, isDefinedBy).
- Content negotiation per resource and per term: text/turtle, application/ld+json, application/rdf+xml (if the RDF
  crate makes it cheap), text/html (a readable page per scheme and per term). URL prefix: not /s/ and not /assets/;
  add the prefix to deploy/Caddyfile's @app matcher and the e2e harness proxy.
- Namespace: suggest a persistent namespace (w3id.org or PURL redirect) and explain why; allow the archive's own URL.
  A term list's terms need stable URIs: decide how a term URI is minted (slug vs id) and make it never change.
- Linking a term to an authority (e.g. GeoNames) is the step before: allow an exactMatch/closeMatch URI per term
  (stored Romeka-side), served in the SKOS output; suggestion lookup is out of scope (manual URI entry).
- Admin: "Publish" on a term list page and on a vocabulary page in Standards, with the draft/published state, the
  version history, the public URL, and a preview of the Turtle.
Tests first: Rust serializer tests (parse your output back with the RDF crate and compare triples), HTTP tests for
negotiation and for the add/deprecate-only rule, e2e for publish + fetch, screenshots.
If vocab/rdf-import adds an RDF crate first, reuse that crate (check main and ask the runner); do not add a second.
