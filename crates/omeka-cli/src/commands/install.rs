//! `romeka install`: build a database and seed it, mirroring the install tasks.
//!
//! Upstream `InstallController` runs pre-checks then these tasks
//! (`application/config/module.config.php:188-193`): create the schema, record
//! migrations, install the default vocabularies, install the default templates,
//! create the first user, add the default settings. The port's `migrate` covers
//! the first two; the rest are here. Web-only tasks (session destroy, cache
//! clear, directory checks) have no CLI meaning.
//!
//! Every step is idempotent: a second run detects the existing rows and inserts
//! nothing. That matches re-running install against an existing database.

use std::path::PathBuf;

use omeka_db::repos::{PropertyRepo, TemplateRepo, UserRepo, VocabularyRepo};
use omeka_db::rows::{NewTemplate, NewTemplateProperty, NewUser};
use omeka_db::{migrate, open_file};

use omeka_core::permissions::Role;

use crate::error::CliError;
use crate::store;
use crate::vocabularies;

/// `Module::VERSION` at the frozen SHA, the value the `version` setting holds.
pub const VERSION: &str = "4.3.0-alpha";

/// `Paginator::PER_PAGE`, the `pagination_per_page` setting default.
const PAGINATION_PER_PAGE: i64 = 25;

/// The default template label, from `InstallDefaultTemplatesTask::$templates`.
const BASE_RESOURCE_TEMPLATE: &str = "Base Resource";

/// The `Base Resource` template's properties, in upstream order. Each entry is
/// `(vocabulary prefix, local name, alternate label)`.
const BASE_RESOURCE_PROPERTIES: &[(&str, &str, Option<&str>)] = &[
    ("dcterms", "title", None),
    ("dcterms", "rights", None),
    ("dcterms", "type", None),
    ("dcterms", "creator", None),
    ("dcterms", "date", None),
    ("dcterms", "description", None),
    ("dcterms", "format", None),
    ("dcterms", "language", None),
    ("dcterms", "spatial", Some("Place")),
    ("dcterms", "publisher", None),
    ("dcterms", "alternative", None),
    ("dcterms", "contributor", None),
    ("dcterms", "extent", None),
    ("dcterms", "identifier", None),
    ("dcterms", "relation", None),
    ("dcterms", "isReplacedBy", None),
    ("dcterms", "replaces", None),
    ("dcterms", "rightsHolder", None),
    ("dcterms", "subject", None),
    ("dcterms", "temporal", None),
];

/// Operator input for `romeka install`.
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub database: PathBuf,
    pub email: String,
    pub name: String,
    pub password: String,
    pub title: String,
    pub time_zone: String,
    pub locale: String,
}

/// What the install wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub database: PathBuf,
    pub vocabularies_inserted: usize,
    pub classes_inserted: usize,
    pub properties_inserted: usize,
    /// Default vocabulary members an older bundle left with their local name as the label,
    /// given the bundled file's label and comment.
    pub vocabulary_members_corrected: usize,
    pub template_created: bool,
    pub user_created: bool,
    /// The site settings the upgrade moved out of a theme blob into their own
    /// rows, zero on a fresh install.
    pub settings_moved: usize,
}

/// Create the file, migrate, and seed it.
///
/// # Errors
/// A [`CliError`] for an uncreatable database file, a failed migration, or a
/// missing default vocabulary the template depends on.
pub async fn run(options: &InstallOptions) -> Result<InstallReport, CliError> {
    let pool = open_file(&options.database).await?;
    migrate(&pool).await?;

    let vocabularies = vocabularies::converge(&pool).await?;
    let template_created = install_base_resource_template(&pool).await?;
    let user_created = install_admin_user(&pool, options).await?;
    install_default_settings(&pool, options).await?;
    // The upgrade step: an install that predates the per-row site settings
    // holds them in the theme blob. This moves them out, once.
    let settings_moved = omeka_server::settings_upgrade::run(&pool).await?;

    Ok(InstallReport {
        database: options.database.clone(),
        vocabularies_inserted: vocabularies.vocabularies_inserted,
        classes_inserted: vocabularies.classes_inserted,
        properties_inserted: vocabularies.properties_inserted,
        vocabulary_members_corrected: vocabularies.members_corrected,
        template_created,
        user_created,
        settings_moved,
    })
}

async fn install_base_resource_template(pool: &sqlx::SqlitePool) -> Result<bool, CliError> {
    let templates = TemplateRepo::new(pool);
    if templates
        .find_by_label(BASE_RESOURCE_TEMPLATE)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    let vocabulary = VocabularyRepo::new(pool)
        .get_by_prefix("dcterms")
        .await?
        .ok_or_else(|| CliError::message("the dcterms vocabulary was not installed"))?;
    let properties = PropertyRepo::new(pool);

    let template = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: BASE_RESOURCE_TEMPLATE.to_string(),
        })
        .await?;

    // Upstream's `InstallDefaultTemplatesTask::installTemplate` creates this
    // template through the API, so `ResourceTemplateAdapter::hydrate` numbers the
    // rows from one. The install writes them directly, so it applies the same
    // convention here rather than starting at zero.
    for (index, (prefix, local_name, alternate_label)) in
        BASE_RESOURCE_PROPERTIES.iter().enumerate()
    {
        let keyword = format!("{prefix}:{local_name}");
        let property = properties
            .find(vocabulary.id, local_name)
            .await?
            .ok_or_else(|| {
                CliError::message(format!("the default property {keyword} is missing"))
            })?;
        templates
            .add_property(&NewTemplateProperty {
                resource_template_id: template.id,
                property_id: property.id,
                alternate_label: alternate_label.map(str::to_string),
                alternate_comment: None,
                position: Some(index as i64 + 1),
                data_type: None,
                is_required: false,
                is_private: false,
                default_lang: None,
            })
            .await?;
    }
    Ok(true)
}

async fn install_admin_user(
    pool: &sqlx::SqlitePool,
    options: &InstallOptions,
) -> Result<bool, CliError> {
    let users = UserRepo::new(pool);
    if users.get_by_email(&options.email).await?.is_some() {
        return Ok(false);
    }
    users
        .create(&NewUser {
            email: options.email.clone(),
            name: options.name.clone(),
            password_hash: Some(omeka_api::password::hash(&options.password)?),
            role: Role::GlobalAdmin.wire_name().to_string(),
            is_active: true,
            created: None,
        })
        .await?;
    Ok(true)
}

async fn install_default_settings(
    pool: &sqlx::SqlitePool,
    options: &InstallOptions,
) -> Result<(), CliError> {
    let settings: [(&str, serde_json::Value); 8] = [
        ("version", serde_json::Value::from(VERSION)),
        (
            "installation_title",
            serde_json::Value::from(options.title.clone()),
        ),
        (
            "administrator_email",
            serde_json::Value::from(options.email.clone()),
        ),
        (
            "time_zone",
            serde_json::Value::from(options.time_zone.clone()),
        ),
        ("locale", serde_json::Value::from(options.locale.clone())),
        (
            "pagination_per_page",
            serde_json::Value::from(PAGINATION_PER_PAGE),
        ),
        ("version_notifications", serde_json::Value::from("1")),
        ("use_htmlpurifier", serde_json::Value::from("1")),
    ];
    for (id, value) in &settings {
        store::write_setting(pool, id, value).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::temp_db;
    use omeka_api::password;
    use omeka_db::repos::{ClassRepo, Paging};

    fn options(database: PathBuf) -> InstallOptions {
        InstallOptions {
            database,
            email: "admin@example.org".to_string(),
            name: "Admin".to_string(),
            password: "hunter2".to_string(),
            title: "Test Install".to_string(),
            time_zone: "UTC".to_string(),
            locale: "en".to_string(),
        }
    }

    #[tokio::test]
    async fn install_writes_vocabularies_template_and_admin() {
        let db = temp_db().await;
        let report = run(&options(db.path.clone())).await.expect("install");

        assert_eq!(report.vocabularies_inserted, 4);
        assert_eq!(report.classes_inserted, 105);
        assert_eq!(report.properties_inserted, 184);
        assert!(report.template_created);
        assert!(report.user_created);

        let vocabularies = VocabularyRepo::new(&db.pool);
        assert!(
            vocabularies
                .get_by_prefix("dcterms")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            vocabularies
                .get_by_prefix("dctype")
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            vocabularies
                .list(&Default::default(), &Paging::default())
                .await
                .unwrap()
                .len(),
            4
        );

        let classes = ClassRepo::new(&db.pool);
        assert_eq!(
            classes
                .list(&Default::default(), &Paging::default())
                .await
                .unwrap()
                .len(),
            105
        );

        let dcterms = vocabularies
            .get_by_prefix("dcterms")
            .await
            .unwrap()
            .unwrap();
        let title = PropertyRepo::new(&db.pool)
            .find(dcterms.id, "title")
            .await
            .unwrap()
            .expect("dcterms:title");
        assert_eq!(title.label, "Title");

        let templates = TemplateRepo::new(&db.pool);
        let template = templates
            .find_by_label("Base Resource")
            .await
            .unwrap()
            .expect("Base Resource");
        let fields = templates.list_properties(template.id).await.unwrap();
        assert_eq!(fields.len(), 20);
        assert_eq!(fields[0].position, Some(1));
        assert_eq!(fields[8].alternate_label.as_deref(), Some("Place"));

        let user = UserRepo::new(&db.pool)
            .get_by_email("admin@example.org")
            .await
            .unwrap()
            .expect("admin user");
        assert_eq!(user.role, "global_admin");
        assert!(user.is_active);
        assert!(password::verify(
            "hunter2",
            Some(user.password_hash.as_deref().expect("a stored hash"))
        ));

        let version = store::read_setting(&db.pool, "version")
            .await
            .unwrap()
            .expect("version setting");
        assert_eq!(version, serde_json::Value::from(VERSION));
    }

    #[tokio::test]
    async fn install_is_idempotent() {
        let db = temp_db().await;
        run(&options(db.path.clone())).await.expect("first install");
        let second = run(&options(db.path.clone()))
            .await
            .expect("second install");

        assert_eq!(second.vocabularies_inserted, 0);
        assert_eq!(second.classes_inserted, 0);
        assert_eq!(second.properties_inserted, 0);
        assert!(!second.template_created);
        assert!(!second.user_created);

        let users = UserRepo::new(&db.pool);
        assert_eq!(
            users
                .list(&Default::default(), &Paging::default())
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            VocabularyRepo::new(&db.pool)
                .list(&Default::default(), &Paging::default())
                .await
                .unwrap()
                .len(),
            4
        );
    }

    #[tokio::test]
    async fn install_fails_on_an_uncreatable_database_path() {
        let options = options(PathBuf::from("/nonexistent-directory-xyz/romeka.db"));
        let error = run(&options).await.expect_err("bad path is an error");
        assert!(matches!(error, CliError::Db(_)), "got {error:?}");
    }
}
