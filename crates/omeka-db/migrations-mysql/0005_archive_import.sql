-- omeka-db 0005_archive_import (MySQL): the archive importer's own tables.
--
-- The SQLite file says what each column holds. This is the same schema in the conventions of
-- `0003_publication.sql`: `IF NOT EXISTS`, InnoDB, utf8mb4, JSON as LONGTEXT. MySQL has no partial
-- index, so `undoable` carries no unique index here and `ImportRepo::open_undo_window` keeps at
-- most one open window by clearing the others in the same transaction.
--
-- There is no 0004 in this directory on purpose: 0004 makes SQLite's term `local_name` columns
-- case-sensitive, and upstream's MySQL schema already has `utf8mb4_bin` there.

CREATE TABLE IF NOT EXISTS romeka_import (
  id INT AUTO_INCREMENT NOT NULL,
  owner_id INT DEFAULT NULL,
  folder TEXT NOT NULL,
  plan LONGTEXT NOT NULL,
  status VARCHAR(16) NOT NULL,
  undoable TINYINT(1) NOT NULL DEFAULT 0,
  job_id INT DEFAULT NULL,
  summary LONGTEXT DEFAULT NULL,
  created DATETIME NOT NULL,
  finished DATETIME DEFAULT NULL,
  INDEX IDX_ROMEKA_IMPORT_OWNER (owner_id),
  INDEX IDX_ROMEKA_IMPORT_JOB (job_id),
  PRIMARY KEY(id),
  CONSTRAINT CHK_ROMEKA_IMPORT_STATUS CHECK (status IN ('draft', 'running', 'done', 'failed', 'undoing', 'undone')),
  CONSTRAINT CHK_ROMEKA_IMPORT_UNDOABLE CHECK (undoable IN (0, 1)),
  CONSTRAINT FK_ROMEKA_IMPORT_OWNER FOREIGN KEY (owner_id) REFERENCES `user` (id) ON DELETE SET NULL,
  CONSTRAINT FK_ROMEKA_IMPORT_JOB FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_import_resource (
  import_id INT NOT NULL,
  kind VARCHAR(32) NOT NULL,
  target_id INT NOT NULL,
  PRIMARY KEY(import_id, kind, target_id),
  CONSTRAINT CHK_ROMEKA_IMPORT_RESOURCE_KIND CHECK (kind IN ('item', 'media', 'item_set', 'resource_template', 'property')),
  CONSTRAINT FK_ROMEKA_IMPORT_RESOURCE_IMPORT FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
