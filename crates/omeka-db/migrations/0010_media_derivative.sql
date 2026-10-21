-- omeka-db 0010_media_derivative (SQLite): the state of every derivative a media gets.
--
-- Not an upstream table (`SCHEMA-NOTES.md`, "Romeka tables"). Upstream makes its derivatives
-- inside the request and keeps the only record of them in `media.has_thumbnails`; Romeka makes
-- them in a job and records what the job is doing here. Upstream never reads it.
--
-- One row per media and kind. `kind` names the work: the three JPEG thumbnails, a video's poster,
-- a PDF's first page, and later an access copy a browser can play. `state` moves pending ->
-- running -> ready, failed, or none when the kind does not apply to this media. `reason` carries
-- the typed skip or failure a reader shows; `progress` is 0-100 while a long derivative runs and
-- NULL otherwise; `tool` names the external command the work used, when it used one. `attempts`
-- counts claims, so a row that failed twice is visible without reading the job log.
--
-- `queued`, `started` and `finished` are UTC text, as every other timestamp here. `started` and
-- `finished` are required by the state they belong to, so a half-written row cannot claim to be
-- running or done.
--
-- 0009 belongs to another branch (staged uploads). sqlx applies every version it has not recorded,
-- so the gap is safe.

CREATE TABLE romeka_media_derivative (
    media_id INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('thumbnail', 'poster', 'pdf_page', 'access_copy')),
    state TEXT NOT NULL CHECK (state IN ('pending', 'running', 'ready', 'failed', 'none')),
    reason TEXT,
    progress INTEGER CHECK (progress IS NULL OR (progress BETWEEN 0 AND 100)),
    tool TEXT,
    queued TEXT NOT NULL,
    started TEXT,
    finished TEXT,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    PRIMARY KEY (media_id, kind),
    CHECK (state <> 'running' OR started IS NOT NULL),
    CHECK (state IN ('pending', 'running') OR finished IS NOT NULL),
    FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE
);
CREATE INDEX ix_romeka_media_derivative_pending ON romeka_media_derivative (state, queued);
