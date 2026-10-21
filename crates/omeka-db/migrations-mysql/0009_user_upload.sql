-- omeka-db 0009_user_upload (MySQL): a user's staged media uploads. The shape and the
-- reasons are in `migrations/0009_user_upload.sql`; this file is the same table in
-- upstream's dialect, so an existing Omeka S database gains it and nothing else.
--
-- `user_id` and `item_id` are INT because `user` and `item` use upstream's integer id.
-- `id` is the storage id, 40 hex characters, kept binary so a lookup is exact.
-- `claimed` is when one media create took the upload; see the SQLite file for the
-- claim, the release and the stale bound.

CREATE TABLE IF NOT EXISTS romeka_user_upload (
  user_id INT NOT NULL,
  id VARCHAR(64) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  item_id INT DEFAULT NULL,
  path TEXT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  media_type VARCHAR(190) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin DEFAULT NULL,
  claimed DATETIME DEFAULT NULL,
  length BIGINT NOT NULL,
  received BIGINT NOT NULL DEFAULT 0,
  created DATETIME NOT NULL,
  expires DATETIME NOT NULL,
  PRIMARY KEY(user_id, id),
  CONSTRAINT CHK_ROMEKA_USER_UPLOAD_LENGTH CHECK (length >= 0),
  CONSTRAINT CHK_ROMEKA_USER_UPLOAD_RECEIVED CHECK (received >= 0 AND received <= length),
  CONSTRAINT FK_ROMEKA_USER_UPLOAD_USER FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
  CONSTRAINT FK_ROMEKA_USER_UPLOAD_ITEM FOREIGN KEY (item_id) REFERENCES item (id) ON DELETE SET NULL,
  INDEX ix_romeka_user_upload_expires (expires),
  INDEX ix_romeka_user_upload_item (user_id, item_id)
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
