-- omeka-db 0002_custom_vocab (SQLite): the term list table.
--
-- Source of truth: the Custom Vocab module (github.com/omeka-s-modules/CustomVocab, v2.1.0),
-- `Module::install`. Romeka builds the module's table into core with the same name, columns,
-- unique key and foreign key actions, so an Omeka S install with Custom Vocab reads these rows.
-- The translation rules are the ones `0001_init.sql` applies (SCHEMA-NOTES.md): `terms` and `uris`
-- are `(DC2Type:json)` and become JSON text, and the label collates NOCASE.

CREATE TABLE custom_vocab (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_set_id INTEGER,
    owner_id INTEGER,
    label TEXT NOT NULL COLLATE NOCASE UNIQUE,
    lang TEXT,
    terms TEXT,
    uris TEXT,
    FOREIGN KEY (item_set_id) REFERENCES item_set (id) ON DELETE SET NULL,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_custom_vocab_item_set_id ON custom_vocab (item_set_id);
CREATE INDEX ix_custom_vocab_owner_id ON custom_vocab (owner_id);
