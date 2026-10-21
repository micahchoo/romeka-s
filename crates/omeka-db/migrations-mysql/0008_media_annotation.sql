-- omeka-db 0008_media_annotation (MySQL). The SQLite file says what each column holds.
-- Conventions of 0007: IF NOT EXISTS, InnoDB, utf8mb4, JSON as LONGTEXT. InnoDB FULLTEXT on
-- body_text replaces the FTS5 table and needs no triggers. CHECK needs MySQL 8.0.16+ or
-- MariaDB 10.2+; older servers accept and ignore it, and the repo's own checks still hold.

CREATE TABLE IF NOT EXISTS romeka_media_annotation (
  id INT AUTO_INCREMENT NOT NULL, media_id INT NOT NULL, owner_id INT DEFAULT NULL,
  revision INT NOT NULL DEFAULT 1, created DATETIME NOT NULL, modified DATETIME NOT NULL,
  motivation VARCHAR(16) NOT NULL, origin VARCHAR(8) NOT NULL DEFAULT 'author', lang VARCHAR(35) DEFAULT NULL,
  selector_kind VARCHAR(8) NOT NULL, selector LONGTEXT DEFAULT NULL,
  t_start_ms BIGINT DEFAULT NULL, t_end_ms BIGINT DEFAULT NULL,
  body_json LONGTEXT NOT NULL, body_text LONGTEXT NOT NULL, extensions LONGTEXT DEFAULT NULL,
  client_key VARCHAR(64) DEFAULT NULL,
  PRIMARY KEY (id),
  INDEX IDX_ROMEKA_MEDIA_ANNOTATION_MEDIA (media_id, id),
  INDEX IDX_ROMEKA_MEDIA_ANNOTATION_TIME (media_id, t_start_ms, id),
  INDEX IDX_ROMEKA_MEDIA_ANNOTATION_TRACK (media_id, motivation, origin, lang),
  INDEX IDX_ROMEKA_MEDIA_ANNOTATION_OWNER (owner_id),
  UNIQUE INDEX UNIQ_ROMEKA_MEDIA_ANNOTATION_CLIENT (media_id, client_key),
  FULLTEXT INDEX FT_ROMEKA_MEDIA_ANNOTATION_TEXT (body_text),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_REVISION CHECK (revision >= 1),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_MOTIVATION CHECK (motivation IN
    ('commenting','tagging','describing','identifying','highlighting','linking',
     'classifying','questioning','replying','supplementing')),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_ORIGIN CHECK (origin IN ('author','track')),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_KIND CHECK (selector_kind IN ('whole','rect','polygon','time')),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_BODY CHECK (CHAR_LENGTH(body_text) <= 16000),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_SHAPE CHECK (
       (selector_kind = 'whole' AND selector IS NULL AND t_start_ms IS NULL AND t_end_ms IS NULL)
    OR (selector_kind IN ('rect','polygon') AND selector IS NOT NULL
        AND t_start_ms IS NULL AND t_end_ms IS NULL)
    OR (selector_kind = 'time' AND selector IS NULL AND t_start_ms >= 0 AND t_end_ms > t_start_ms)),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_CUE CHECK (motivation <> 'supplementing' OR selector_kind = 'time'),
  CONSTRAINT CK_ROMEKA_MEDIA_ANNOTATION_TRACK CHECK (origin <> 'track' OR motivation = 'supplementing'),
  CONSTRAINT FK_ROMEKA_MEDIA_ANNOTATION_MEDIA FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE,
  CONSTRAINT FK_ROMEKA_MEDIA_ANNOTATION_OWNER FOREIGN KEY (owner_id) REFERENCES `user` (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_media_extent (
  media_id INT NOT NULL, storage_path VARCHAR(190) NOT NULL, probed DATETIME NOT NULL,
  duration_ms BIGINT DEFAULT NULL, width INT DEFAULT NULL, height INT DEFAULT NULL,
  PRIMARY KEY (media_id),
  CONSTRAINT CK_ROMEKA_MEDIA_EXTENT_DURATION CHECK (duration_ms IS NULL OR duration_ms > 0),
  CONSTRAINT FK_ROMEKA_MEDIA_EXTENT_MEDIA FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
