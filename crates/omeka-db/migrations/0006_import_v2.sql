-- omeka-db 0006_import_v2 (SQLite): import v2's tables replace the v1 archive importer's
-- (`docs/import-v2.md`, "Database").
--
-- No install holds a v1 import worth keeping (measured 2026-10-20), so this drops the v1 tables
-- and their rows instead of converting them. `migrate` runs it with foreign key enforcement off,
-- and nothing outside these tables references them, so the drops touch no other row.
--
-- - `romeka_import`: one row per import. `sources`, `plan`, `ran_plan` and `summary` are JSON that
--   `omeka-api` owns. `ran_plan` is the plan frozen at run start; `revision` counts plan saves.
--   The summary also holds the worker's `heartbeat` and, once a reader found it dead, `died`.
--   `stop_requested` is its own column, not a summary member: a stop request writes it without a
--   heartbeat, and the worker's whole-summary writes cannot drop it.
-- - `romeka_import_resource`: the ledger. One row per resource an import created or changed, with
--   the key of the record that wrote it and the time it was written. `target_id` has no foreign
--   key: it points at one of five tables, and undo deletes the target before the row.
--   `change_kind` has no CHECK, so a later action adds a kind without a table rebuild.
-- - `romeka_import_before`: the state of a changed resource before the import changed it.
-- - `romeka_import_upload`: one tus upload into the import's staging area. The bytes received
--   are `received`, not `offset`, which MySQL reserves.
-- - `romeka_import_entry`: the evidence index. `path_hash` is the SHA-256 of `path`, computed in
--   Rust, so the key stays short on MySQL however long the path is.
-- - `romeka_import_scan`: one row per scanned source, replaced in the same transaction as the
--   source's entries: what the walk skipped (`anomalies`, JSON that `omeka-api` owns), the error
--   when the source could not be read, and the scan's `token`.

DROP TABLE romeka_import_resource;
DROP TABLE romeka_import;

CREATE TABLE romeka_import (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    sources TEXT NOT NULL,
    plan TEXT NOT NULL,
    ran_plan TEXT,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    status TEXT NOT NULL CHECK (status IN (
        'draft', 'running', 'done', 'run_failed', 'undoing', 'undone', 'undo_failed'
    )),
    job_id INTEGER,
    summary TEXT,
    stop_requested INTEGER NOT NULL DEFAULT 0 CHECK (stop_requested IN (0, 1)),
    copied_from INTEGER,
    created TEXT NOT NULL,
    finished TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE SET NULL,
    FOREIGN KEY (copied_from) REFERENCES romeka_import (id) ON DELETE SET NULL
);

CREATE TABLE romeka_import_resource (
    import_id INTEGER NOT NULL,
    kind TEXT NOT NULL
        CHECK (kind IN ('item', 'media', 'item_set', 'resource_template', 'property')),
    target_id INTEGER NOT NULL,
    record TEXT NOT NULL,
    change_kind TEXT NOT NULL DEFAULT 'created',
    written TEXT NOT NULL,
    PRIMARY KEY (import_id, kind, target_id),
    FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
);

CREATE INDEX romeka_import_resource_record ON romeka_import_resource (import_id, record);

CREATE TABLE romeka_import_before (
    import_id INTEGER NOT NULL,
    kind TEXT NOT NULL
        CHECK (kind IN ('item', 'media', 'item_set', 'resource_template', 'property')),
    target_id INTEGER NOT NULL,
    image TEXT NOT NULL,
    PRIMARY KEY (import_id, kind, target_id),
    FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
);

CREATE TABLE romeka_import_upload (
    import_id INTEGER NOT NULL,
    id TEXT NOT NULL,
    path TEXT NOT NULL,
    length INTEGER NOT NULL CHECK (length >= 0),
    received INTEGER NOT NULL DEFAULT 0 CHECK (received >= 0 AND received <= length),
    created TEXT NOT NULL,
    PRIMARY KEY (import_id, id),
    FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
);

CREATE TABLE romeka_import_entry (
    import_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    path TEXT NOT NULL,
    path_hash TEXT NOT NULL,
    size INTEGER NOT NULL CHECK (size >= 0),
    modified TEXT NOT NULL,
    media_type TEXT,
    flags TEXT NOT NULL,
    PRIMARY KEY (import_id, source, path_hash),
    FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
);

CREATE TABLE romeka_import_scan (
    import_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    anomalies TEXT NOT NULL,
    error TEXT,
    token TEXT NOT NULL,
    scanned TEXT NOT NULL,
    PRIMARY KEY (import_id, source),
    FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
);
