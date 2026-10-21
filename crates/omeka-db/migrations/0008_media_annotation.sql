-- omeka-db 0008_media_annotation (SQLite): W3C Web Annotations on a media, and the extent
-- of audio and video. Not upstream tables (SCHEMA-NOTES.md, "Romeka tables"). Writes go
-- through MediaAnnotationRepo only; the W3C, IIIF and WebVTT documents are projections.
-- revision: the strong ETag "r<revision>". origin 'track': a caption import's row, replaced by
-- the next import of the same lang. lang, body_text: derived on write. selector: "x,y,w,h"
-- (rect) or "x1,y1 x2,y2 ..." (polygon), upright original pixels; a time range lives only in
-- t_start_ms/t_end_ms. extensions: unknown extension keys, JSON. client_key: Idempotency-Key.
-- romeka_media_extent: ffprobe's answer for the original at storage_path; stale if it moved.

CREATE TABLE romeka_media_annotation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_id INTEGER NOT NULL, owner_id INTEGER,
    revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
    created TEXT NOT NULL, modified TEXT NOT NULL,
    motivation TEXT NOT NULL CHECK (motivation IN
        ('commenting', 'tagging', 'describing', 'identifying', 'highlighting', 'linking',
         'classifying', 'questioning', 'replying', 'supplementing')),
    origin TEXT NOT NULL DEFAULT 'author' CHECK (origin IN ('author', 'track')),
    lang TEXT,
    selector_kind TEXT NOT NULL CHECK (selector_kind IN ('whole', 'rect', 'polygon', 'time')),
    selector TEXT, t_start_ms INTEGER, t_end_ms INTEGER,
    body_json TEXT NOT NULL, body_text TEXT NOT NULL, extensions TEXT, client_key TEXT,
    CHECK (length(body_text) <= 16000),
    CHECK (client_key IS NULL OR length(client_key) BETWEEN 1 AND 64),
    CHECK (
        (selector_kind = 'whole' AND selector IS NULL AND t_start_ms IS NULL AND t_end_ms IS NULL)
     OR (selector_kind IN ('rect', 'polygon') AND selector IS NOT NULL
         AND t_start_ms IS NULL AND t_end_ms IS NULL)
     OR (selector_kind = 'time' AND selector IS NULL
         AND t_start_ms >= 0 AND t_end_ms > t_start_ms)
    ),
    CHECK (motivation <> 'supplementing' OR selector_kind = 'time'),
    CHECK (origin <> 'track' OR motivation = 'supplementing'),
    FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_romeka_media_annotation_media ON romeka_media_annotation (media_id, id);
CREATE INDEX ix_romeka_media_annotation_time ON romeka_media_annotation (media_id, t_start_ms, id);
CREATE INDEX ix_romeka_media_annotation_track
    ON romeka_media_annotation (media_id, motivation, origin, lang);
CREATE INDEX ix_romeka_media_annotation_owner ON romeka_media_annotation (owner_id);
CREATE UNIQUE INDEX ux_romeka_media_annotation_client
    ON romeka_media_annotation (media_id, client_key);

-- The text index, external content, kept in step by triggers like fulltext_search_fts (0001).
-- The triggers fire on cascade deletes too, so deleting a media empties its hits.
CREATE VIRTUAL TABLE romeka_media_annotation_fts USING fts5(body_text,
    content = 'romeka_media_annotation', content_rowid = 'id', tokenize = 'unicode61 remove_diacritics 2');
CREATE TRIGGER romeka_media_annotation_ai AFTER INSERT ON romeka_media_annotation BEGIN
    INSERT INTO romeka_media_annotation_fts (rowid, body_text) VALUES (new.id, new.body_text);
END;
CREATE TRIGGER romeka_media_annotation_ad AFTER DELETE ON romeka_media_annotation BEGIN
    INSERT INTO romeka_media_annotation_fts (romeka_media_annotation_fts, rowid, body_text)
        VALUES ('delete', old.id, old.body_text);
END;
CREATE TRIGGER romeka_media_annotation_au AFTER UPDATE OF body_text ON romeka_media_annotation BEGIN
    INSERT INTO romeka_media_annotation_fts (romeka_media_annotation_fts, rowid, body_text)
        VALUES ('delete', old.id, old.body_text);
    INSERT INTO romeka_media_annotation_fts (rowid, body_text) VALUES (new.id, new.body_text);
END;

CREATE TABLE romeka_media_extent (
    media_id INTEGER PRIMARY KEY, storage_path TEXT NOT NULL, probed TEXT NOT NULL,
    duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms > 0),
    width INTEGER CHECK (width IS NULL OR width > 0), height INTEGER CHECK (height IS NULL OR height > 0),
    FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE
);
