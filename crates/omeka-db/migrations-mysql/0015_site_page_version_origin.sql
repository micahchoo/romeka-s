-- omeka-db 0015_site_page_version_origin (MySQL): what made each saved page version; see the
-- SQLite migration of the same number for the values and the backfill. MySQL 8 refuses a `CHECK`
-- on `user_id`, whose foreign key sets it to NULL, so "a before_history row has no user" is kept
-- in code (`PageVersionRepo`).

ALTER TABLE romeka_site_page_version ADD COLUMN origin VARCHAR(16) NOT NULL DEFAULT 'save';
ALTER TABLE romeka_site_page_version ADD CONSTRAINT CK_ROMEKA_SITE_PAGE_VERSION_ORIGIN CHECK (
    origin IN ('before_history', 'create', 'save')
    AND (origin = 'save' OR version = 1)
);

UPDATE romeka_site_page_version SET origin = 'before_history' WHERE version = 1 AND user_id IS NULL;
