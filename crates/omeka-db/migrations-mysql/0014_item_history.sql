-- omeka-db 0014_item_history (MySQL): who changed what on an item, and when.
--
-- The SQLite file says what each column holds. This is the same schema in the conventions of
-- `0012_bulk.sql`: InnoDB, utf8mb4, JSON as LONGTEXT, `IF NOT EXISTS` so a run that stopped part
-- way can run again. `source` and the run columns are checked in code (`HistorySource`).

CREATE TABLE IF NOT EXISTS romeka_item_history (
  id INT AUTO_INCREMENT NOT NULL,
  item_id INT NOT NULL,
  created DATETIME NOT NULL,
  user_id INT DEFAULT NULL,
  source VARCHAR(16) NOT NULL,
  bulk_id INT DEFAULT NULL,
  import_id INT DEFAULT NULL,
  changes LONGTEXT NOT NULL,
  INDEX IDX_ROMEKA_ITEM_HISTORY_ITEM (item_id, id),
  INDEX IDX_ROMEKA_ITEM_HISTORY_USER (user_id),
  PRIMARY KEY(id),
  CONSTRAINT FK_ROMEKA_ITEM_HISTORY_USER FOREIGN KEY (user_id) REFERENCES `user` (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
