-- Upstream collates property.local_name and resource_class.local_name utf8mb4_bin, so FOAF's
-- givenName and givenname are two terms there. 0001 gave both columns COLLATE NOCASE, which
-- refused the second. SQLite cannot change a column's collation in place, so each table is
-- rebuilt in the order SQLite's ALTER TABLE documentation gives: new table, copy, drop, rename,
-- indexes. `schema::migrate` runs this with foreign key enforcement off, as that order requires;
-- with it on, the DROP would run value's ON DELETE CASCADE and empty the value table.

CREATE TEMP TABLE term_sequence AS
    SELECT name, seq FROM sqlite_sequence WHERE name IN ('property', 'resource_class');

CREATE TABLE resource_class_rebuilt (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    vocabulary_id INTEGER NOT NULL,
    local_name TEXT NOT NULL,
    label TEXT NOT NULL,
    comment TEXT,
    UNIQUE (vocabulary_id, local_name),
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (vocabulary_id) REFERENCES vocabulary (id)
);
INSERT INTO resource_class_rebuilt (id, owner_id, vocabulary_id, local_name, label, comment)
    SELECT id, owner_id, vocabulary_id, local_name, label, comment FROM resource_class;
DROP TABLE resource_class;
ALTER TABLE resource_class_rebuilt RENAME TO resource_class;
CREATE INDEX ix_resource_class_owner_id ON resource_class (owner_id);
CREATE INDEX ix_resource_class_vocabulary_id ON resource_class (vocabulary_id);

CREATE TABLE property_rebuilt (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    vocabulary_id INTEGER NOT NULL,
    local_name TEXT NOT NULL,
    label TEXT NOT NULL,
    comment TEXT,
    UNIQUE (vocabulary_id, local_name),
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (vocabulary_id) REFERENCES vocabulary (id)
);
INSERT INTO property_rebuilt (id, owner_id, vocabulary_id, local_name, label, comment)
    SELECT id, owner_id, vocabulary_id, local_name, label, comment FROM property;
DROP TABLE property;
ALTER TABLE property_rebuilt RENAME TO property;
CREATE INDEX ix_property_owner_id ON property (owner_id);
CREATE INDEX ix_property_vocabulary_id ON property (vocabulary_id);

-- The copy sets each sequence to the highest id kept; a term deleted from the end had raised it
-- further, and that id must stay unused.
UPDATE sqlite_sequence
    SET seq = (SELECT max(term_sequence.seq, sqlite_sequence.seq) FROM term_sequence
               WHERE term_sequence.name = sqlite_sequence.name)
    WHERE name IN (SELECT name FROM term_sequence);
DROP TABLE term_sequence;

-- A broken reference rolls the rebuild back: the CHECK fails the INSERT, and the migration with it.
CREATE TEMP TABLE foreign_key_guard (broken INTEGER NOT NULL CHECK (broken = 0));
INSERT INTO foreign_key_guard SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE foreign_key_guard;
