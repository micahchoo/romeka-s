-- omeka-db 0013_bulk_trash (SQLite): the trash of a bulk delete (`briefs/items-bulk-edit-design.md`,
-- "Delete and its undo"). Upstream has no such column.
--
-- A delete keeps each item it removed in `romeka_bulk_target.image` (the trash snapshot) and its
-- files under `trash/` in the store, so an undo can put both back. `purged` is when the trash of
-- the run was emptied: from then on the run cannot be undone or run again, and its images are
-- cleared as the purge reaches them. A run that never deleted anything keeps it NULL.

ALTER TABLE romeka_bulk ADD COLUMN purged TEXT;
