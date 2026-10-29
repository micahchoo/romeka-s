-- omeka-db 0015_site_page_version_origin (SQLite): what made each saved page version. Upstream has
-- no such table.
--
-- `origin` is one of:
-- - `before_history`: the state a page had before its first recorded save. The save keeps it as
--   version 1, with no user.
-- - `create`: the state the page create made, as version 1, with the creating user.
-- - `save`: a save, with the saving user.
-- A restore is a plain save: the editor loads the old version into the draft and saves it.
--
-- Before 0015, version 1 with no user was always the state before history began, and every other
-- row was a save (the page create recorded no version). The backfill says exactly that. A user
-- deleted later sets `user_id` to NULL on a `create` or `save` row; the author is then a deleted
-- user, never "not recorded". No reference moves, so foreign keys stay valid.

ALTER TABLE romeka_site_page_version ADD COLUMN origin TEXT NOT NULL DEFAULT 'save' CHECK (
    origin IN ('before_history', 'create', 'save')
    AND (origin = 'save' OR version = 1)
    AND (origin <> 'before_history' OR user_id IS NULL)
);

UPDATE romeka_site_page_version SET origin = 'before_history' WHERE version = 1 AND user_id IS NULL;
