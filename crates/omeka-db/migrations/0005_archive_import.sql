-- omeka-db 0005_archive_import (SQLite): the archive importer's own tables.
--
-- Not upstream tables. Upstream reads nothing here, so an install that shares the database sees
-- its own tables unchanged (`SCHEMA-NOTES.md`, "Romeka tables").
--
-- - `romeka_import`: one row per import, from the draft the admin edits to the finished run.
--   `plan` and `summary` are JSON; `omeka-api` owns what they mean. `folder` is relative to
--   ROMEKA_ARCHIVE_ROOT. `undoable` marks the one import whose undo window is open.
-- - `romeka_import_resource`: one row per thing a run created (`kind` names the table, `target_id`
--   its id), written in the transaction that created it. Undo walks these rows.
--
-- The partial unique index keeps at most one open undo window. MySQL has no partial index, so
-- `ImportRepo::open_undo_window` enforces the same rule there inside one transaction.

CREATE TABLE romeka_import (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    folder TEXT NOT NULL,
    plan TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN ('draft', 'running', 'done', 'failed', 'undoing', 'undone')),
    undoable INTEGER NOT NULL DEFAULT 0 CHECK (undoable IN (0, 1)),
    job_id INTEGER,
    summary TEXT,
    created TEXT NOT NULL,
    finished TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX romeka_import_one_undoable ON romeka_import (undoable) WHERE undoable = 1;

CREATE TABLE romeka_import_resource (
    import_id INTEGER NOT NULL,
    kind TEXT NOT NULL
        CHECK (kind IN ('item', 'media', 'item_set', 'resource_template', 'property')),
    target_id INTEGER NOT NULL,
    PRIMARY KEY (import_id, kind, target_id),
    FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
);
