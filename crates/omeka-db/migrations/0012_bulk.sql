-- omeka-db 0012_bulk (SQLite): bulk changes to items (`briefs/items-bulk-edit-design.md`).
-- Upstream has no such tables.
--
-- - `romeka_bulk`: one row per bulk run. `query` is the browse query the run was previewed from
--   and `change_json` the change it applies, both JSON that `omeka-api` owns. `kind` has no CHECK,
--   so a later kind (delete) needs no table rebuild. `total` is the number of targets frozen at
--   preview; `done` and `failed` are what the job last counted from the targets. `heartbeat` is
--   the job's last proof that it was alive, which the server settles as dead when it is stale.
-- - `romeka_bulk_target`: one row per item the run touches, written at preview. `image` is the
--   item's before-image (`BeforeImage`), written in the same transaction as the item's change,
--   and `written` the item's `modified` that change left: a later `modified` means someone edited
--   the item since. `item_id` has no foreign key: a deleted item keeps its row, so undo can say
--   it is gone. `error` is the code of the reason a target failed.

CREATE TABLE romeka_bulk (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    kind TEXT NOT NULL,
    query TEXT NOT NULL,
    change_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'previewed', 'running', 'stopping', 'stopped', 'done', 'failed',
        'undoing', 'undo_stopping', 'undone', 'partly_undone'
    )),
    total INTEGER NOT NULL CHECK (total >= 0),
    done INTEGER NOT NULL DEFAULT 0 CHECK (done >= 0),
    failed INTEGER NOT NULL DEFAULT 0 CHECK (failed >= 0),
    job_id INTEGER,
    error TEXT,
    created TEXT NOT NULL,
    started TEXT,
    ended TEXT,
    heartbeat TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE SET NULL
);

CREATE INDEX romeka_bulk_owner ON romeka_bulk (owner_id);

CREATE TABLE romeka_bulk_target (
    bulk_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN (
        'pending', 'done', 'unchanged', 'failed', 'restored', 'kept'
    )),
    image TEXT,
    error TEXT,
    written TEXT,
    PRIMARY KEY (bulk_id, item_id),
    FOREIGN KEY (bulk_id) REFERENCES romeka_bulk (id) ON DELETE CASCADE
);

CREATE INDEX romeka_bulk_target_state ON romeka_bulk_target (bulk_id, state, item_id);
