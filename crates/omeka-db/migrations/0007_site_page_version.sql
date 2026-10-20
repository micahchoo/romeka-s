-- omeka-db 0007_site_page_version (SQLite): the saved versions of each site page.
--
-- Not an upstream table. Upstream reads nothing here, so an install that shares the database sees
-- its own tables unchanged (`SCHEMA-NOTES.md`, "Romeka tables").
--
-- One row per saved state of a page, never updated. `snapshot` is the page document as the API
-- answers it, blocks and attachments included; `omeka-api` owns that shape. `created` is when the
-- state went live and `user_id` who saved it; a page's first save also keeps the state it replaced,
-- with no user. `PageVersionRepo::save` keeps the newest 100 rows per page.
--
-- 0006 belongs to another branch. sqlx applies every version it has not recorded, so the gap is safe.

CREATE TABLE romeka_site_page_version (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL,
    version INTEGER NOT NULL,
    created TEXT NOT NULL,
    user_id INTEGER,
    snapshot TEXT NOT NULL,
    UNIQUE (page_id, version),
    FOREIGN KEY (page_id) REFERENCES site_page (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE SET NULL
);
