-- omeka-db 0003_publication (SQLite): Romeka's publication state for term lists and vocabularies.
--
-- Not an upstream table. Upstream reads `custom_vocab` and `vocabulary` unchanged, so everything
-- publishing adds lives here (`SCHEMA-NOTES.md`, "Romeka tables"):
--
-- - `romeka_publication`: one row per term list or vocabulary that has Romeka state. `slug` is the
--   path under `/vocab/` and `namespace_uri` the namespace its terms are minted in; both are set at
--   the first publish and never change. `terms` is a JSON map from each term's key (a list's word or
--   URI, a vocabulary member's local name) to its state: the minted local name, whether it is
--   deprecated, and its exactMatch and closeMatch links.
-- - `romeka_publication_version`: one row per publish, never updated. `snapshot` is the whole
--   published scheme as JSON, so a version serves on its own.
--
-- The target keys go to NULL when upstream deletes the list or the vocabulary, and the versions
-- stay: a published term keeps resolving even if the thing it was published from is gone.

CREATE TABLE romeka_publication (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    custom_vocab_id INTEGER UNIQUE,
    vocabulary_id INTEGER UNIQUE,
    slug TEXT UNIQUE,
    namespace_uri TEXT,
    terms TEXT NOT NULL,
    CHECK (custom_vocab_id IS NULL OR vocabulary_id IS NULL),
    FOREIGN KEY (custom_vocab_id) REFERENCES custom_vocab (id) ON DELETE SET NULL,
    FOREIGN KEY (vocabulary_id) REFERENCES vocabulary (id) ON DELETE SET NULL
);

CREATE TABLE romeka_publication_version (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publication_id INTEGER NOT NULL,
    version INTEGER NOT NULL,
    created TEXT NOT NULL,
    snapshot TEXT NOT NULL,
    UNIQUE (publication_id, version),
    FOREIGN KEY (publication_id) REFERENCES romeka_publication (id) ON DELETE CASCADE
);
