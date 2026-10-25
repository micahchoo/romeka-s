-- omeka-db 0013_bulk_trash (MySQL): when the trash of a bulk delete was emptied; see the SQLite
-- migration of the same number.
ALTER TABLE romeka_bulk ADD COLUMN purged DATETIME DEFAULT NULL;
