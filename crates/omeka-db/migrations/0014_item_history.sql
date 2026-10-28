-- omeka-db 0014_item_history (SQLite): who changed what on an item, and when
-- (`briefs/archive-tools-2-design.md`, "Item history"). Upstream has no such table.
--
-- One row per write that changed an item's values, visibility, class, item type, sets or sites,
-- never updated afterwards except that one import's later pass over the same item folds into its
-- own row. `changes` is JSON: one entry per changed field with the state before and after
-- (`omeka_db::rows::FieldChange`). `source` says what made the write: `edit` (the item form or the
-- API), `bulk` (a bulk run, `bulk_id`), `import` (an import, `import_id`) or `restore` (an undo
-- of the bulk run or the import it names). The row commits in the transaction of the write.
--
-- `item_id` has no foreign key: a bulk delete takes the item row and its undo puts it back with
-- the same id, and the history must survive that. Ids are never reused (`AUTOINCREMENT`), so a
-- row whose item is gone is unreachable; the delete and the trash purge remove them. `bulk_id`
-- and `import_id` have no foreign key either: the entry names the run that made it even if that
-- run's row goes, as `romeka_bulk_target.item_id` does.
-- No migration here moves data, so foreign keys stay valid without help.

CREATE TABLE romeka_item_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL,
    created TEXT NOT NULL,
    user_id INTEGER,
    source TEXT NOT NULL CHECK (source IN ('edit', 'bulk', 'import', 'restore')),
    bulk_id INTEGER,
    import_id INTEGER,
    changes TEXT NOT NULL,
    CHECK (
        (source = 'edit' AND bulk_id IS NULL AND import_id IS NULL)
        OR (source = 'bulk' AND bulk_id IS NOT NULL AND import_id IS NULL)
        OR (source = 'import' AND import_id IS NOT NULL AND bulk_id IS NULL)
        OR (source = 'restore' AND (bulk_id IS NULL) <> (import_id IS NULL))
    ),
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE SET NULL
);

CREATE INDEX romeka_item_history_item ON romeka_item_history (item_id, id);
