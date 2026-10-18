-- omeka-db 0001_init: the Omeka S install schema translated to SQLite.
--
-- Source of truth: source/omeka-s @ 5e413f0, application/data/install/schema.sql (390 lines).
-- That file is the current full schema: it already carries the effect of every DDL migration in
-- application/data/migrations/. The migrations are upgrade paths for older installs, so this
-- initial schema translates schema.sql alone. Evidence: SCHEMA-NOTES.md.
--
-- Translation rules and every deliberate divergence: crates/omeka-db/SCHEMA-NOTES.md.
-- In short: INTEGER PRIMARY KEY AUTOINCREMENT for upstream AUTO_INCREMENT, TEXT for char columns and
-- for datetimes in the upstream format YYYY-MM-DD HH:MM:SS (UTC), TEXT for JSON columns, INTEGER 0/1
-- for tinyint(1) with CHECK constraints, COLLATE NOCASE where upstream collates utf8mb4_unicode_ci and
-- the column is unique or matched by equality, and foreign keys with the upstream ON DELETE actions.

CREATE TABLE user (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL COLLATE NOCASE UNIQUE,
    name TEXT NOT NULL,
    created TEXT NOT NULL,
    modified TEXT,
    password_hash TEXT,
    role TEXT NOT NULL,
    is_active INTEGER NOT NULL CHECK (is_active IN (0, 1))
);

CREATE TABLE vocabulary (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    namespace_uri TEXT NOT NULL COLLATE NOCASE UNIQUE,
    prefix TEXT NOT NULL COLLATE NOCASE UNIQUE,
    label TEXT NOT NULL,
    comment TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_vocabulary_owner_id ON vocabulary (owner_id);

CREATE TABLE resource_class (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    vocabulary_id INTEGER NOT NULL,
    local_name TEXT NOT NULL COLLATE NOCASE,
    label TEXT NOT NULL,
    comment TEXT,
    UNIQUE (vocabulary_id, local_name),
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (vocabulary_id) REFERENCES vocabulary (id)
);
CREATE INDEX ix_resource_class_owner_id ON resource_class (owner_id);
CREATE INDEX ix_resource_class_vocabulary_id ON resource_class (vocabulary_id);

CREATE TABLE property (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    vocabulary_id INTEGER NOT NULL,
    local_name TEXT NOT NULL COLLATE NOCASE,
    label TEXT NOT NULL,
    comment TEXT,
    UNIQUE (vocabulary_id, local_name),
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (vocabulary_id) REFERENCES vocabulary (id)
);
CREATE INDEX ix_property_owner_id ON property (owner_id);
CREATE INDEX ix_property_vocabulary_id ON property (vocabulary_id);

CREATE TABLE resource_template (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    resource_class_id INTEGER,
    title_property_id INTEGER,
    description_property_id INTEGER,
    label TEXT NOT NULL COLLATE NOCASE UNIQUE,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (resource_class_id) REFERENCES resource_class (id) ON DELETE SET NULL,
    FOREIGN KEY (title_property_id) REFERENCES property (id) ON DELETE SET NULL,
    FOREIGN KEY (description_property_id) REFERENCES property (id) ON DELETE SET NULL
);
CREATE INDEX ix_resource_template_owner_id ON resource_template (owner_id);
CREATE INDEX ix_resource_template_resource_class_id ON resource_template (resource_class_id);
CREATE INDEX ix_resource_template_title_property_id ON resource_template (title_property_id);
CREATE INDEX ix_resource_template_description_property_id ON resource_template (description_property_id);

CREATE TABLE resource_template_property (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    resource_template_id INTEGER NOT NULL,
    property_id INTEGER NOT NULL,
    alternate_label TEXT,
    alternate_comment TEXT,
    position INTEGER,
    data_type TEXT,
    is_required INTEGER NOT NULL CHECK (is_required IN (0, 1)),
    is_private INTEGER NOT NULL CHECK (is_private IN (0, 1)),
    default_lang TEXT,
    UNIQUE (resource_template_id, property_id),
    FOREIGN KEY (resource_template_id) REFERENCES resource_template (id),
    FOREIGN KEY (property_id) REFERENCES property (id) ON DELETE CASCADE
);
CREATE INDEX ix_resource_template_property_property_id ON resource_template_property (property_id);

CREATE TABLE asset (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL,
    storage_id TEXT NOT NULL COLLATE NOCASE UNIQUE,
    extension TEXT,
    alt_text TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_asset_owner_id ON asset (owner_id);

CREATE TABLE resource (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    resource_class_id INTEGER,
    resource_template_id INTEGER,
    thumbnail_id INTEGER,
    title TEXT,
    is_public INTEGER NOT NULL CHECK (is_public IN (0, 1)),
    created TEXT NOT NULL,
    modified TEXT,
    resource_type TEXT NOT NULL CHECK (resource_type IN (
        'Omeka\Entity\Item',
        'Omeka\Entity\ItemSet',
        'Omeka\Entity\Media',
        'Omeka\Entity\ValueAnnotation'
    )),
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL,
    FOREIGN KEY (resource_class_id) REFERENCES resource_class (id) ON DELETE SET NULL,
    FOREIGN KEY (resource_template_id) REFERENCES resource_template (id) ON DELETE SET NULL,
    FOREIGN KEY (thumbnail_id) REFERENCES asset (id) ON DELETE SET NULL
);
CREATE INDEX ix_resource_owner_id ON resource (owner_id);
CREATE INDEX ix_resource_resource_class_id ON resource (resource_class_id);
CREATE INDEX ix_resource_resource_template_id ON resource (resource_template_id);
CREATE INDEX ix_resource_thumbnail_id ON resource (thumbnail_id);
CREATE INDEX title ON resource (title);
CREATE INDEX is_public ON resource (is_public);

CREATE TABLE item_set (
    id INTEGER PRIMARY KEY,
    is_open INTEGER NOT NULL CHECK (is_open IN (0, 1)),
    FOREIGN KEY (id) REFERENCES resource (id) ON DELETE CASCADE
);

CREATE TABLE item (
    id INTEGER PRIMARY KEY,
    primary_media_id INTEGER,
    FOREIGN KEY (id) REFERENCES resource (id) ON DELETE CASCADE,
    FOREIGN KEY (primary_media_id) REFERENCES media (id) ON DELETE SET NULL
);
CREATE INDEX ix_item_primary_media_id ON item (primary_media_id);

CREATE TABLE media (
    id INTEGER PRIMARY KEY,
    item_id INTEGER NOT NULL,
    ingester TEXT NOT NULL,
    renderer TEXT NOT NULL,
    data TEXT,
    source TEXT,
    media_type TEXT,
    storage_id TEXT COLLATE NOCASE UNIQUE,
    extension TEXT,
    sha256 TEXT,
    size INTEGER,
    has_original INTEGER NOT NULL CHECK (has_original IN (0, 1)),
    has_thumbnails INTEGER NOT NULL CHECK (has_thumbnails IN (0, 1)),
    position INTEGER,
    lang TEXT,
    alt_text TEXT,
    FOREIGN KEY (item_id) REFERENCES item (id),
    FOREIGN KEY (id) REFERENCES resource (id) ON DELETE CASCADE
);
CREATE INDEX ix_media_item_id ON media (item_id);
CREATE INDEX item_position ON media (item_id, position);
CREATE INDEX media_type ON media (media_type);

CREATE TABLE value_annotation (
    id INTEGER PRIMARY KEY,
    FOREIGN KEY (id) REFERENCES resource (id) ON DELETE CASCADE
);

CREATE TABLE value (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    resource_id INTEGER NOT NULL,
    property_id INTEGER NOT NULL,
    value_resource_id INTEGER,
    value_annotation_id INTEGER UNIQUE,
    type TEXT NOT NULL,
    lang TEXT,
    value TEXT,
    uri TEXT,
    is_public INTEGER NOT NULL CHECK (is_public IN (0, 1)),
    FOREIGN KEY (resource_id) REFERENCES resource (id),
    FOREIGN KEY (property_id) REFERENCES property (id) ON DELETE CASCADE,
    FOREIGN KEY (value_resource_id) REFERENCES resource (id) ON DELETE CASCADE,
    FOREIGN KEY (value_annotation_id) REFERENCES value_annotation (id) ON DELETE SET NULL
);
CREATE INDEX ix_value_resource_id ON value (resource_id);
CREATE INDEX ix_value_property_id ON value (property_id);
CREATE INDEX ix_value_value_resource_id ON value (value_resource_id);
-- Upstream names this index `value`; SQLite keeps tables and indexes in one namespace, so an index
-- cannot share the table's name. Renamed, see SCHEMA-NOTES.md.
CREATE INDEX ix_value_value ON value (value);
CREATE INDEX uri ON value (uri);
CREATE INDEX ix_value_is_public ON value (is_public);

CREATE TABLE item_item_set (
    item_id INTEGER NOT NULL,
    item_set_id INTEGER NOT NULL,
    PRIMARY KEY (item_id, item_set_id),
    FOREIGN KEY (item_id) REFERENCES item (id) ON DELETE CASCADE,
    FOREIGN KEY (item_set_id) REFERENCES item_set (id) ON DELETE CASCADE
);
CREATE INDEX ix_item_item_set_item_set_id ON item_item_set (item_set_id);

CREATE TABLE item_site (
    item_id INTEGER NOT NULL,
    site_id INTEGER NOT NULL,
    PRIMARY KEY (item_id, site_id),
    FOREIGN KEY (item_id) REFERENCES item (id) ON DELETE CASCADE,
    FOREIGN KEY (site_id) REFERENCES site (id) ON DELETE CASCADE
);
CREATE INDEX ix_item_site_site_id ON item_site (site_id);

CREATE TABLE site (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    thumbnail_id INTEGER,
    homepage_id INTEGER UNIQUE,
    owner_id INTEGER,
    slug TEXT NOT NULL COLLATE NOCASE UNIQUE,
    theme TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT,
    navigation TEXT NOT NULL,
    item_pool TEXT NOT NULL,
    created TEXT NOT NULL,
    modified TEXT,
    is_public INTEGER NOT NULL CHECK (is_public IN (0, 1)),
    assign_new_items INTEGER NOT NULL DEFAULT 0 CHECK (assign_new_items IN (0, 1)),
    FOREIGN KEY (thumbnail_id) REFERENCES asset (id) ON DELETE SET NULL,
    FOREIGN KEY (homepage_id) REFERENCES site_page (id) ON DELETE SET NULL,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_site_thumbnail_id ON site (thumbnail_id);
CREATE INDEX ix_site_owner_id ON site (owner_id);

CREATE TABLE site_page (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL,
    slug TEXT NOT NULL COLLATE NOCASE,
    title TEXT NOT NULL,
    is_public INTEGER NOT NULL CHECK (is_public IN (0, 1)),
    layout TEXT,
    layout_data TEXT,
    created TEXT NOT NULL,
    modified TEXT,
    UNIQUE (site_id, slug),
    FOREIGN KEY (site_id) REFERENCES site (id)
);
CREATE INDEX is_public_site_page ON site_page (is_public);
CREATE INDEX ix_site_page_site_id ON site_page (site_id);

CREATE TABLE site_page_block (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL,
    layout TEXT NOT NULL,
    data TEXT NOT NULL,
    layout_data TEXT,
    position INTEGER NOT NULL,
    FOREIGN KEY (page_id) REFERENCES site_page (id)
);
CREATE INDEX ix_site_page_block_page_id ON site_page_block (page_id);
CREATE INDEX page_position ON site_page_block (page_id, position);

CREATE TABLE site_block_attachment (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_id INTEGER NOT NULL,
    item_id INTEGER,
    media_id INTEGER,
    caption TEXT NOT NULL,
    position INTEGER NOT NULL,
    FOREIGN KEY (block_id) REFERENCES site_page_block (id),
    FOREIGN KEY (item_id) REFERENCES item (id) ON DELETE SET NULL,
    FOREIGN KEY (media_id) REFERENCES media (id) ON DELETE SET NULL
);
CREATE INDEX ix_site_block_attachment_block_id ON site_block_attachment (block_id);
CREATE INDEX ix_site_block_attachment_item_id ON site_block_attachment (item_id);
CREATE INDEX ix_site_block_attachment_media_id ON site_block_attachment (media_id);
CREATE INDEX block_position ON site_block_attachment (block_id, position);

CREATE TABLE site_item_set (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL,
    item_set_id INTEGER NOT NULL,
    position INTEGER,
    UNIQUE (site_id, item_set_id),
    FOREIGN KEY (site_id) REFERENCES site (id) ON DELETE CASCADE,
    FOREIGN KEY (item_set_id) REFERENCES item_set (id) ON DELETE CASCADE
);
CREATE INDEX ix_site_item_set_item_set_id ON site_item_set (item_set_id);
CREATE INDEX position_site_item_set ON site_item_set (position);

CREATE TABLE site_permission (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    UNIQUE (site_id, user_id),
    FOREIGN KEY (site_id) REFERENCES site (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE
);
CREATE INDEX ix_site_permission_user_id ON site_permission (user_id);

CREATE TABLE site_setting (
    id TEXT NOT NULL COLLATE NOCASE,
    site_id INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (id, site_id),
    FOREIGN KEY (site_id) REFERENCES site (id) ON DELETE CASCADE
);
CREATE INDEX ix_site_setting_site_id ON site_setting (site_id);

CREATE TABLE user_setting (
    id TEXT NOT NULL COLLATE NOCASE,
    user_id INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (id, user_id),
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE
);
CREATE INDEX ix_user_setting_user_id ON user_setting (user_id);

CREATE TABLE setting (
    id TEXT NOT NULL COLLATE NOCASE PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE module (
    id TEXT NOT NULL COLLATE NOCASE PRIMARY KEY,
    is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
    version TEXT NOT NULL
);

CREATE TABLE job (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    pid TEXT,
    status TEXT,
    class TEXT NOT NULL,
    args TEXT,
    log TEXT,
    started TEXT NOT NULL,
    ended TEXT,
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_job_owner_id ON job (owner_id);

CREATE TABLE api_key (
    id TEXT NOT NULL COLLATE NOCASE PRIMARY KEY,
    owner_id INTEGER NOT NULL,
    label TEXT NOT NULL,
    credential_hash TEXT NOT NULL,
    last_ip BLOB,
    last_accessed TEXT,
    created TEXT NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES user (id)
);
CREATE INDEX ix_api_key_owner_id ON api_key (owner_id);

CREATE TABLE password_creation (
    id TEXT NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL UNIQUE,
    created TEXT NOT NULL,
    activate INTEGER NOT NULL CHECK (activate IN (0, 1)),
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE
);

CREATE TABLE migration (
    version TEXT NOT NULL COLLATE NOCASE PRIMARY KEY
);

CREATE TABLE session (
    id TEXT NOT NULL COLLATE NOCASE PRIMARY KEY,
    data BLOB NOT NULL,
    modified INTEGER NOT NULL
);

CREATE TABLE fulltext_search (
    id INTEGER NOT NULL,
    resource TEXT NOT NULL COLLATE NOCASE,
    owner_id INTEGER,
    is_public INTEGER NOT NULL CHECK (is_public IN (0, 1)),
    title TEXT,
    text TEXT,
    PRIMARY KEY (id, resource),
    FOREIGN KEY (owner_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE INDEX ix_fulltext_search_owner_id ON fulltext_search (owner_id);

-- Upstream indexes (title, text) with MySQL FULLTEXT. SQLite has no FULLTEXT index type; this is the
-- FTS5 translation. The content lives in fulltext_search and the triggers below keep the index in sync,
-- so a MATCH query joins back on rowid. See SCHEMA-NOTES.md.
CREATE VIRTUAL TABLE fulltext_search_fts USING fts5(
    title,
    text,
    content = 'fulltext_search',
    content_rowid = 'rowid'
);
CREATE TRIGGER fulltext_search_fts_ai AFTER INSERT ON fulltext_search BEGIN
    INSERT INTO fulltext_search_fts (rowid, title, text)
    VALUES (new.rowid, new.title, new.text);
END;
CREATE TRIGGER fulltext_search_fts_ad AFTER DELETE ON fulltext_search BEGIN
    INSERT INTO fulltext_search_fts (fulltext_search_fts, rowid, title, text)
    VALUES ('delete', old.rowid, old.title, old.text);
END;
CREATE TRIGGER fulltext_search_fts_au AFTER UPDATE ON fulltext_search BEGIN
    INSERT INTO fulltext_search_fts (fulltext_search_fts, rowid, title, text)
    VALUES ('delete', old.rowid, old.title, old.text);
    INSERT INTO fulltext_search_fts (rowid, title, text)
    VALUES (new.rowid, new.title, new.text);
END;
