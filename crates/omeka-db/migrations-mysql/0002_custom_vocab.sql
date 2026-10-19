-- omeka-db 0002_custom_vocab (MySQL): the term list table.
--
-- Source of truth: the Custom Vocab module (github.com/omeka-s-modules/CustomVocab, v2.1.0),
-- `Module::install`, with its two `ALTER TABLE ... ADD CONSTRAINT` statements folded into the
-- `CREATE TABLE` under the module's own constraint names. `IF NOT EXISTS` makes the migration a
-- no-op against an install where the module already created the table.

CREATE TABLE IF NOT EXISTS custom_vocab (
  id INT AUTO_INCREMENT NOT NULL,
  item_set_id INT DEFAULT NULL,
  owner_id INT DEFAULT NULL,
  `label` VARCHAR(190) NOT NULL,
  lang VARCHAR(190) DEFAULT NULL,
  terms LONGTEXT DEFAULT NULL COMMENT '(DC2Type:json)',
  uris LONGTEXT DEFAULT NULL COMMENT '(DC2Type:json)',
  UNIQUE INDEX UNIQ_8533D2A5EA750E8 (`label`),
  INDEX IDX_8533D2A5960278D7 (item_set_id),
  INDEX IDX_8533D2A57E3C61F9 (owner_id),
  PRIMARY KEY(id),
  CONSTRAINT FK_8533D2A5960278D7 FOREIGN KEY (item_set_id) REFERENCES item_set (id) ON DELETE SET NULL,
  CONSTRAINT FK_8533D2A57E3C61F9 FOREIGN KEY (owner_id) REFERENCES `user` (id) ON DELETE SET NULL
) DEFAULT CHARACTER SET utf8mb4 COLLATE `utf8mb4_unicode_ci` ENGINE = InnoDB;
