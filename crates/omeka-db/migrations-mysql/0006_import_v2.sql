-- omeka-db 0006_import_v2 (MySQL): import v2's tables replace the v1 archive importer's.
--
-- The SQLite file says what each column holds. This is the same schema in the conventions of
-- `0005_archive_import.sql`: InnoDB, utf8mb4, JSON as LONGTEXT, `IF NOT EXISTS` so a run that
-- stopped part way (MySQL commits each statement) can run again.
--
-- Every text column in a key, and every path, is `utf8mb4_bin`, so `A.jpg` and `a.jpg` stay two
-- keys. No column is named `offset` or `change`: both are reserved. The entry key uses
-- `path_hash`, computed in Rust: a path in the key would pass InnoDB's 3,072-byte limit, and
-- MariaDB may refuse a generated column in a key.

DROP TABLE IF EXISTS romeka_import_scan;
DROP TABLE IF EXISTS romeka_import_resource;
DROP TABLE IF EXISTS romeka_import;

CREATE TABLE IF NOT EXISTS romeka_import (
  id INT AUTO_INCREMENT NOT NULL,
  owner_id INT DEFAULT NULL,
  sources LONGTEXT NOT NULL,
  plan LONGTEXT NOT NULL,
  ran_plan LONGTEXT DEFAULT NULL,
  revision INT NOT NULL DEFAULT 0,
  status VARCHAR(16) NOT NULL,
  job_id INT DEFAULT NULL,
  summary LONGTEXT DEFAULT NULL,
  stop_requested TINYINT(1) NOT NULL DEFAULT 0,
  copied_from INT DEFAULT NULL,
  created DATETIME NOT NULL,
  saved DATETIME NOT NULL,
  finished DATETIME DEFAULT NULL,
  INDEX IDX_ROMEKA_IMPORT_OWNER (owner_id),
  INDEX IDX_ROMEKA_IMPORT_JOB (job_id),
  INDEX IDX_ROMEKA_IMPORT_COPIED_FROM (copied_from),
  PRIMARY KEY(id),
  CONSTRAINT CHK_ROMEKA_IMPORT_REVISION CHECK (revision >= 0),
  CONSTRAINT CHK_ROMEKA_IMPORT_STOP CHECK (stop_requested IN (0, 1)),
  CONSTRAINT CHK_ROMEKA_IMPORT_STATUS CHECK (status IN ('draft', 'running', 'done', 'run_failed', 'undoing', 'undone', 'partly_undone', 'undo_failed')),
  CONSTRAINT FK_ROMEKA_IMPORT_OWNER FOREIGN KEY (owner_id) REFERENCES `user` (id) ON DELETE SET NULL,
  CONSTRAINT FK_ROMEKA_IMPORT_JOB FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE SET NULL,
  CONSTRAINT FK_ROMEKA_IMPORT_COPIED_FROM FOREIGN KEY (copied_from) REFERENCES romeka_import (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_import_resource (
  import_id INT NOT NULL,
  kind VARCHAR(32) NOT NULL,
  target_id INT NOT NULL,
  record VARCHAR(512) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  change_kind VARCHAR(16) NOT NULL DEFAULT 'created',
  written DATETIME NOT NULL,
  PRIMARY KEY(import_id, kind, target_id),
  INDEX IDX_ROMEKA_IMPORT_RESOURCE_RECORD (import_id, record),
  CONSTRAINT CHK_ROMEKA_IMPORT_RESOURCE_KIND CHECK (kind IN ('item', 'media', 'item_set', 'resource_template', 'property')),
  CONSTRAINT FK_ROMEKA_IMPORT_RESOURCE_IMPORT FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_import_before (
  import_id INT NOT NULL,
  kind VARCHAR(32) NOT NULL,
  target_id INT NOT NULL,
  image LONGTEXT NOT NULL,
  PRIMARY KEY(import_id, kind, target_id),
  CONSTRAINT CHK_ROMEKA_IMPORT_BEFORE_KIND CHECK (kind IN ('item', 'media', 'item_set', 'resource_template', 'property')),
  CONSTRAINT FK_ROMEKA_IMPORT_BEFORE_IMPORT FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_import_upload (
  import_id INT NOT NULL,
  id VARCHAR(64) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  path TEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  length BIGINT NOT NULL,
  received BIGINT NOT NULL DEFAULT 0,
  created DATETIME NOT NULL,
  PRIMARY KEY(import_id, id),
  CONSTRAINT CHK_ROMEKA_IMPORT_UPLOAD_LENGTH CHECK (length >= 0),
  CONSTRAINT CHK_ROMEKA_IMPORT_UPLOAD_RECEIVED CHECK (received >= 0 AND received <= length),
  CONSTRAINT FK_ROMEKA_IMPORT_UPLOAD_IMPORT FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_import_entry (
  import_id INT NOT NULL,
  source VARCHAR(64) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  path TEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  path_hash CHAR(64) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  size BIGINT NOT NULL,
  modified DATETIME NOT NULL,
  media_type VARCHAR(255) DEFAULT NULL,
  flags LONGTEXT NOT NULL,
  PRIMARY KEY(import_id, source, path_hash),
  CONSTRAINT CHK_ROMEKA_IMPORT_ENTRY_SIZE CHECK (size >= 0),
  CONSTRAINT FK_ROMEKA_IMPORT_ENTRY_IMPORT FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_import_scan (
  import_id INT NOT NULL,
  source VARCHAR(64) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  anomalies LONGTEXT NOT NULL,
  error LONGTEXT DEFAULT NULL,
  token VARCHAR(255) NOT NULL,
  scanned DATETIME NOT NULL,
  PRIMARY KEY(import_id, source),
  CONSTRAINT FK_ROMEKA_IMPORT_SCAN_IMPORT FOREIGN KEY (import_id) REFERENCES romeka_import (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
