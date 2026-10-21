-- omeka-db 0009_user_upload (SQLite): a user's staged media uploads, the files an
-- editor sends from the item page before a media row names them. Not upstream: a
-- media is created from whatever multipart body arrives, so a file has no life of
-- its own and a 2 GB file cannot be resumed.
--
-- - the key is (user_id, id), because an upload belongs to the person who started
--   it and another account's id is not theirs to reach (`docs/api.md`, Errors:
--   NotFound);
-- - `item_id` is the item the upload was started for, or NULL. The list route
--   filters on it, so a reload resumes the files of the item in front of the person;
-- - `path` is the name the client gave, never a store path: the bytes sit under
--   `uploads/<user_id>/` keyed by the id;
-- - `length` and `received` are the tus `Upload-Length` and the bytes stored, so
--   `received = length` is a file that arrived whole;
-- - `expires` is 24 hours after the last byte arrived. The import sweep job removes
--   a row past it, with its parts and its file;
-- - `claimed` is when one media create took the upload. A create sets it in one
--   UPDATE that only matches an unclaimed row, so two creates cannot both ingest
--   the same bytes. A claim older than an hour is reclaimable, which is what a
--   create that died between the claim and its release leaves.
--
-- `romeka_import_upload` stays import-only: an import's staging area is swept by the
-- import's own retention, and a user's upload by `expires`.

CREATE TABLE romeka_user_upload (
    user_id INTEGER NOT NULL,
    id TEXT NOT NULL,
    item_id INTEGER,
    path TEXT NOT NULL,
    media_type TEXT,
    claimed TEXT,
    length INTEGER NOT NULL CHECK (length >= 0),
    received INTEGER NOT NULL DEFAULT 0 CHECK (received >= 0 AND received <= length),
    created TEXT NOT NULL,
    expires TEXT NOT NULL,
    PRIMARY KEY (user_id, id),
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
    FOREIGN KEY (item_id) REFERENCES item (id) ON DELETE SET NULL
);

CREATE INDEX romeka_user_upload_expires ON romeka_user_upload (expires);
CREATE INDEX romeka_user_upload_item ON romeka_user_upload (user_id, item_id);
