-- omeka-db 0010_media_derivative (MySQL). The SQLite file says what each column holds.
-- Conventions of 0007 and 0008: IF NOT EXISTS, InnoDB, utf8mb4. CHECK needs MySQL 8.0.16+ or
-- MariaDB 10.2+; older servers accept and ignore it, and the repository's own checks still hold.
-- `kind` and `state` are ASCII words, so `utf8mb4_bin` keeps them exact; a reader matches them by
-- name, and `romeka_media_derivative_queued` orders the queue.

CREATE TABLE IF NOT EXISTS romeka_media_derivative (
  media_id INT NOT NULL,
  kind VARCHAR(16) NOT NULL,
  state VARCHAR(16) NOT NULL,
  reason TEXT DEFAULT NULL,
  progress INT DEFAULT NULL,
  tool VARCHAR(32) DEFAULT NULL,
  queued DATETIME NOT NULL,
  started DATETIME DEFAULT NULL,
  finished DATETIME DEFAULT NULL,
  attempts INT NOT NULL DEFAULT 0,
  PRIMARY KEY (media_id, kind),
  INDEX ix_romeka_media_derivative_pending (state, queued),
  CONSTRAINT CK_ROMEKA_MEDIA_DERIVATIVE_KIND CHECK (kind IN
    ('thumbnail','poster','pdf_page','access_copy')),
  CONSTRAINT CK_ROMEKA_MEDIA_DERIVATIVE_STATE CHECK (state IN
    ('pending','running','ready','failed','none')),
  CONSTRAINT CK_ROMEKA_MEDIA_DERIVATIVE_PROGRESS CHECK (progress IS NULL OR progress BETWEEN 0 AND 100),
  CONSTRAINT CK_ROMEKA_MEDIA_DERIVATIVE_ATTEMPTS CHECK (attempts >= 0),
  CONSTRAINT CK_ROMEKA_MEDIA_DERIVATIVE_STARTED CHECK (state <> 'running' OR started IS NOT NULL),
  CONSTRAINT CK_ROMEKA_MEDIA_DERIVATIVE_FINISHED CHECK
    (state IN ('pending','running') OR finished IS NOT NULL),
  CONSTRAINT FK_ROMEKA_MEDIA_DERIVATIVE_MEDIA FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_bin` ENGINE = InnoDB;
