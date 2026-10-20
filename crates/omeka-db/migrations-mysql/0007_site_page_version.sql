-- omeka-db 0007_site_page_version (MySQL): the saved versions of each site page.
--
-- The SQLite file says what each column holds. This is the same schema in the conventions of
-- `0003_publication.sql`: `IF NOT EXISTS`, InnoDB, utf8mb4, JSON as LONGTEXT. No upstream table
-- changes, so an Omeka S install that shares the database reads its own tables as before.

CREATE TABLE IF NOT EXISTS romeka_site_page_version (
  id INT AUTO_INCREMENT NOT NULL,
  page_id INT NOT NULL,
  version INT NOT NULL,
  created DATETIME NOT NULL,
  user_id INT DEFAULT NULL,
  snapshot LONGTEXT NOT NULL,
  UNIQUE INDEX UNIQ_ROMEKA_SITE_PAGE_VERSION (page_id, version),
  INDEX IDX_ROMEKA_SITE_PAGE_VERSION_USER (user_id),
  PRIMARY KEY(id),
  CONSTRAINT FK_ROMEKA_SITE_PAGE_VERSION_PAGE FOREIGN KEY (page_id) REFERENCES site_page (id) ON DELETE CASCADE,
  CONSTRAINT FK_ROMEKA_SITE_PAGE_VERSION_USER FOREIGN KEY (user_id) REFERENCES `user` (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
