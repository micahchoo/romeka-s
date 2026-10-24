-- omeka-db 0012_bulk (MySQL): bulk changes to items.
--
-- The SQLite file says what each column holds. This is the same schema in the conventions of
-- `0006_import_v2.sql`: InnoDB, utf8mb4, JSON as LONGTEXT, `IF NOT EXISTS` so a run that stopped
-- part way (MySQL commits each statement) can run again. `change` and `before` are reserved
-- words, so the columns are `change_json` and `image`.

CREATE TABLE IF NOT EXISTS romeka_bulk (
  id INT AUTO_INCREMENT NOT NULL,
  owner_id INT DEFAULT NULL,
  kind VARCHAR(16) NOT NULL,
  query LONGTEXT NOT NULL,
  change_json LONGTEXT NOT NULL,
  status VARCHAR(16) NOT NULL,
  total INT NOT NULL,
  done INT NOT NULL DEFAULT 0,
  failed INT NOT NULL DEFAULT 0,
  job_id INT DEFAULT NULL,
  error LONGTEXT DEFAULT NULL,
  created DATETIME NOT NULL,
  started DATETIME DEFAULT NULL,
  ended DATETIME DEFAULT NULL,
  heartbeat DATETIME DEFAULT NULL,
  INDEX IDX_ROMEKA_BULK_OWNER (owner_id),
  INDEX IDX_ROMEKA_BULK_JOB (job_id),
  PRIMARY KEY(id),
  CONSTRAINT CHK_ROMEKA_BULK_STATUS CHECK (status IN ('previewed', 'running', 'stopping', 'stopped', 'done', 'failed', 'undoing', 'undo_stopping', 'undone', 'partly_undone')),
  CONSTRAINT CHK_ROMEKA_BULK_COUNTS CHECK (total >= 0 AND done >= 0 AND failed >= 0),
  CONSTRAINT FK_ROMEKA_BULK_OWNER FOREIGN KEY (owner_id) REFERENCES `user` (id) ON DELETE SET NULL,
  CONSTRAINT FK_ROMEKA_BULK_JOB FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_bulk_target (
  bulk_id INT NOT NULL,
  item_id INT NOT NULL,
  state VARCHAR(16) NOT NULL,
  image LONGTEXT DEFAULT NULL,
  error VARCHAR(64) DEFAULT NULL,
  written DATETIME DEFAULT NULL,
  PRIMARY KEY(bulk_id, item_id),
  INDEX IDX_ROMEKA_BULK_TARGET_STATE (bulk_id, state, item_id),
  CONSTRAINT CHK_ROMEKA_BULK_TARGET_STATE CHECK (state IN ('pending', 'done', 'unchanged', 'failed', 'restored', 'kept')),
  CONSTRAINT FK_ROMEKA_BULK_TARGET_BULK FOREIGN KEY (bulk_id) REFERENCES romeka_bulk (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
