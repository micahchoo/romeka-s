-- omeka-db 0011_media_annotation_box_time (SQLite): a `rect` note may also hold a time range,
-- for a box on a video's frames over a span (`xywh=…&t=…`). The range keeps the `time` rule:
-- t_start_ms >= 0 and t_end_ms > t_start_ms. SQLite cannot change a CHECK in place, so the
-- table is rebuilt in the order 0004 uses: new table, copy, drop, rename, indexes, triggers.
-- The ids are copied, so the external-content text index still names the right rows.

CREATE TEMP TABLE annotation_sequence AS
    SELECT name, seq FROM sqlite_sequence WHERE name = 'romeka_media_annotation';

CREATE TABLE romeka_media_annotation_rebuilt (
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
     OR (selector_kind = 'polygon' AND selector IS NOT NULL
         AND t_start_ms IS NULL AND t_end_ms IS NULL)
     OR (selector_kind = 'rect' AND selector IS NOT NULL
         AND ((t_start_ms IS NULL AND t_end_ms IS NULL)
              OR (t_start_ms >= 0 AND t_end_ms > t_start_ms)))
     OR (selector_kind = 'time' AND selector IS NULL
         AND t_start_ms >= 0 AND t_end_ms > t_start_ms)
    ),
    CHECK (motivation <> 'supplementing' OR selector_kind = 'time'),
    CHECK (origin <> 'track' OR motivation = 'supplementing'),
    FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
INSERT INTO romeka_media_annotation_rebuilt
    (id, media_id, owner_id, revision, created, modified, motivation, origin, lang,
     selector_kind, selector, t_start_ms, t_end_ms, body_json, body_text, extensions, client_key)
    SELECT id, media_id, owner_id, revision, created, modified, motivation, origin, lang,
           selector_kind, selector, t_start_ms, t_end_ms, body_json, body_text, extensions,
           client_key
    FROM romeka_media_annotation;
DROP TABLE romeka_media_annotation;
ALTER TABLE romeka_media_annotation_rebuilt RENAME TO romeka_media_annotation;

CREATE INDEX ix_romeka_media_annotation_media ON romeka_media_annotation (media_id, id);
CREATE INDEX ix_romeka_media_annotation_time ON romeka_media_annotation (media_id, t_start_ms, id);
CREATE INDEX ix_romeka_media_annotation_track
    ON romeka_media_annotation (media_id, motivation, origin, lang);
CREATE INDEX ix_romeka_media_annotation_owner ON romeka_media_annotation (owner_id);
CREATE UNIQUE INDEX ux_romeka_media_annotation_client
    ON romeka_media_annotation (media_id, client_key);

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

-- The sequence comes back exactly as it was: a note deleted from the end had raised it past the
-- highest id kept, and that id must stay unused; a table never written to had no row at all.
DELETE FROM sqlite_sequence WHERE name = 'romeka_media_annotation';
INSERT INTO sqlite_sequence (name, seq) SELECT name, seq FROM annotation_sequence;
DROP TABLE annotation_sequence;
