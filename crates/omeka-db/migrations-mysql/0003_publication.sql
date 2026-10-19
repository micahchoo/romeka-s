-- omeka-db 0003_publication (MySQL): Romeka's publication state for term lists and vocabularies.
--
-- The SQLite file says what each column holds. This is the same schema in the conventions of
-- `0002_custom_vocab.sql`: `IF NOT EXISTS`, InnoDB, utf8mb4, JSON as LONGTEXT. No upstream table
-- changes, so an Omeka S install that shares the database reads its own tables as before.

CREATE TABLE IF NOT EXISTS romeka_publication (
  id INT AUTO_INCREMENT NOT NULL,
  custom_vocab_id INT DEFAULT NULL,
  vocabulary_id INT DEFAULT NULL,
  slug VARCHAR(190) DEFAULT NULL,
  namespace_uri VARCHAR(255) DEFAULT NULL,
  terms LONGTEXT NOT NULL,
  UNIQUE INDEX UNIQ_ROMEKA_PUBLICATION_CUSTOM_VOCAB (custom_vocab_id),
  UNIQUE INDEX UNIQ_ROMEKA_PUBLICATION_VOCABULARY (vocabulary_id),
  UNIQUE INDEX UNIQ_ROMEKA_PUBLICATION_SLUG (slug),
  PRIMARY KEY(id),
  CONSTRAINT CHK_ROMEKA_PUBLICATION_ONE_TARGET CHECK (custom_vocab_id IS NULL OR vocabulary_id IS NULL),
  CONSTRAINT FK_ROMEKA_PUBLICATION_CUSTOM_VOCAB FOREIGN KEY (custom_vocab_id) REFERENCES custom_vocab (id) ON DELETE SET NULL,
  CONSTRAINT FK_ROMEKA_PUBLICATION_VOCABULARY FOREIGN KEY (vocabulary_id) REFERENCES vocabulary (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;

CREATE TABLE IF NOT EXISTS romeka_publication_version (
  id INT AUTO_INCREMENT NOT NULL,
  publication_id INT NOT NULL,
  version INT NOT NULL,
  created DATETIME NOT NULL,
  snapshot LONGTEXT NOT NULL,
  UNIQUE INDEX UNIQ_ROMEKA_PUBLICATION_VERSION (publication_id, version),
  PRIMARY KEY(id),
  CONSTRAINT FK_ROMEKA_PUBLICATION_VERSION_PUBLICATION FOREIGN KEY (publication_id) REFERENCES romeka_publication (id) ON DELETE CASCADE
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
