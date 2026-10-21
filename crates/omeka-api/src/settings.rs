//! The user and global settings the API exposes, and the admin locale they
//! resolve to.
//!
//! Upstream's admin is server-rendered, so its user settings and its global
//! settings never cross the API: `Admin\UserController::editAction` writes the
//! `user-settings` fieldset straight into `user_setting`, and
//! `Admin\SettingController::browseAction` writes `SettingForm` into `setting`.
//! The port's admin is an SPA, so both travel over HTTP, and this module is the
//! boundary. Only the keys in [`USER_SETTINGS`] and [`GLOBAL_SETTINGS`] cross
//! it: a caller cannot reach a row the port's admin has no field for.
//!
//! Two user keys are port-only, so upstream ignores their rows: the admin's
//! lessons record which lessons a user has seen ([`LESSONS_SEEN`]) and whether
//! the user switched lessons off ([`HIDE_LESSONS`]).
//!
//! A user's settings travel as `o:user_settings` on the user document and the
//! user write, the way `o:site_settings` travels on the site
//! ([`crate::site_settings`]). The global settings have their own port-only
//! route, which only a global admin may read or write.
//!
//! Three global keys are upstream's file validation settings, which
//! `Omeka\File\Validator` reads ([`file_validation`]). A list with no row
//! means upstream's default list, because a Romeka install never ran
//! upstream's `AddDefaultSettingsTask`, which stores the defaults. So
//! [`read_global`] answers each of the three as it applies, and a `null` write
//! restores the default.
//!
//! Five global keys are port-only: [`IMPORT_ALLOW_PRIVATE_URLS`] lets every
//! fetch an ingester makes reach private addresses ([`address_policy`]), and
//! four counts bound an import's uploads and how long its files stay
//! ([`upload_settings`]). Each count answers its default when no row holds
//! it, and a `null` write restores the default.
//!
//! The admin locale is `MvcListeners::bootstrapLocale` (lines 132-146): the
//! signed-in user's `locale`, else the global `locale`, else none, which the
//! client reads as its own default.

use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids::UserId;
use omeka_db::repos::{SettingRepo, UserSettingRepo, ValueRepo};
use omeka_db::rows::MediaRow;
use omeka_db::with_db;
use omeka_media::http_guard::AddressPolicy;
use omeka_media::validation::{DEFAULT_EXTENSIONS, DEFAULT_MEDIA_TYPES, FileValidation};
use serde_json::{Map, Value};

use crate::auth::Actor;
use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::catalog::Catalog;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error};

/// The `locale` key, in `setting` and in `user_setting` alike.
pub const LOCALE: &str = "locale";

/// The ids of the admin lessons a user has seen, a list of strings.
pub const LESSONS_SEEN: &str = "romeka_lessons_seen";

/// `true` when the user switched the admin lessons off.
pub const HIDE_LESSONS: &str = "romeka_hide_lessons";

/// `SettingForm`'s allowed media types, a list of strings.
pub const MEDIA_TYPE_WHITELIST: &str = "media_type_whitelist";

/// `SettingForm`'s allowed file extensions, a list of strings.
pub const EXTENSION_WHITELIST: &str = "extension_whitelist";

/// `SettingForm`'s switch that turns file validation off.
pub const DISABLE_FILE_VALIDATION: &str = "disable_file_validation";

/// `SettingForm`'s property term (`media_alt_text_property`) whose value
/// describes a media that stores no alt text.
pub const MEDIA_ALT_TEXT_PROPERTY: &str = "media_alt_text_property";

/// The operator's switch that lifts the URL guard's address check, so an
/// ingester may fetch from loopback, link-local and private ranges
/// (docs/import-v2.md "URL sources"). Port-only; upstream fetches any URL.
pub const IMPORT_ALLOW_PRIVATE_URLS: &str = "import_allow_private_urls";

/// The largest file one upload, one extracted ZIP entry or one URL fetch
/// may be, in bytes (docs/import-v2.md "Staging", "URL sources").
pub const IMPORT_UPLOAD_MAX_FILE: &str = "import_upload_max_file";

/// The most bytes one import's uploads may hold, counting the uploads still
/// in progress.
pub const IMPORT_UPLOAD_MAX_TOTAL: &str = "import_upload_max_total";

/// Days an import's uploaded files stay after it is done or undone, or after
/// its last run attempt failed.
pub const IMPORT_STAGING_KEEP_DAYS: &str = "import_staging_keep_days";

/// Days a draft's uploaded files stay after the draft was last worked on.
pub const IMPORT_DRAFT_STAGING_KEEP_DAYS: &str = "import_draft_staging_keep_days";

/// The defaults of the four upload counts: 2 GiB, 50 GiB, 30 days, 7 days.
pub const UPLOAD_DEFAULTS: UploadSettings = UploadSettings {
    max_file: 2 * 1024 * 1024 * 1024,
    max_total: 50 * 1024 * 1024 * 1024,
    staging_keep_days: 30,
    draft_staging_keep_days: 7,
};

/// The upload limits and the staging retention as they apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadSettings {
    pub max_file: u64,
    pub max_total: u64,
    pub staging_keep_days: u64,
    pub draft_staging_keep_days: u64,
}

/// What a setting's value may be on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A string; an empty string removes the row.
    Text,
    /// `true` or `false`; `false` removes the row, because it is the default.
    Flag,
    /// A list of strings; an empty list removes the row.
    Names,
    /// A list of strings, stored even when empty, because an empty allow
    /// list refuses every file. Each entry is trimmed, and empty and repeated
    /// entries are dropped, as `SettingForm`'s input filter does.
    AllowList,
    /// A whole number above zero; no row means the key's default.
    Count,
}

/// The user keys the API reads and writes. Each one has a field in the admin.
pub const USER_SETTINGS: &[&str] = &[LOCALE, HIDE_LESSONS, LESSONS_SEEN];

/// The global keys the API reads and writes.
pub const GLOBAL_SETTINGS: &[&str] = &[
    LOCALE,
    MEDIA_ALT_TEXT_PROPERTY,
    MEDIA_TYPE_WHITELIST,
    EXTENSION_WHITELIST,
    DISABLE_FILE_VALIDATION,
    IMPORT_ALLOW_PRIVATE_URLS,
    IMPORT_UPLOAD_MAX_FILE,
    IMPORT_UPLOAD_MAX_TOTAL,
    IMPORT_STAGING_KEEP_DAYS,
    IMPORT_DRAFT_STAGING_KEEP_DAYS,
];

fn shape(key: &str) -> Shape {
    match key {
        HIDE_LESSONS | DISABLE_FILE_VALIDATION | IMPORT_ALLOW_PRIVATE_URLS => Shape::Flag,
        LESSONS_SEEN => Shape::Names,
        MEDIA_TYPE_WHITELIST | EXTENSION_WHITELIST => Shape::AllowList,
        IMPORT_UPLOAD_MAX_FILE
        | IMPORT_UPLOAD_MAX_TOTAL
        | IMPORT_STAGING_KEEP_DAYS
        | IMPORT_DRAFT_STAGING_KEEP_DAYS => Shape::Count,
        _ => Shape::Text,
    }
}

/// The member a user's settings travel in.
pub const USER_SETTINGS_MEMBER: &str = "o:user_settings";

/// One write a settings map asks for: a value to store, or `None` to remove the row.
type Change = (&'static str, Option<Value>);

/// The admin locale for a signed-in user: their `locale` setting, else the
/// global one, else `None`. An empty string counts as unset, because upstream
/// tests `!$locale`.
///
/// # Errors
/// [`ApiError::Internal`] when a row cannot be read.
pub async fn admin_locale(pool: &DbPool, user: UserId) -> Result<Option<String>, ApiError> {
    let own =
        with_db!(pool, |db| UserSettingRepo::new(db).get(user, LOCALE).await).map_err(db_error)?;
    if let Some(locale) = non_empty(own) {
        return Ok(Some(locale));
    }
    let global = with_db!(pool, |db| SettingRepo::new(db).get(LOCALE).await).map_err(db_error)?;
    Ok(non_empty(global))
}

/// `MediaRepresentation::altTextResolved` (`MediaRepresentation.php:271`): the
/// media's stored alt text, else its first value on the property
/// [`MEDIA_ALT_TEXT_PROPERTY`] names, else none.
///
/// The fallback reads the value's own text, as the representation's `value()`
/// does, and applies no visibility or language filter: upstream reads the first
/// value of the term whatever its flags.
///
/// # Errors
/// [`DbError`] when the setting or a value row cannot be read.
pub async fn media_alt_text(
    pool: &DbPool,
    catalog: &Catalog,
    media: &MediaRow,
) -> Result<Option<String>, DbError> {
    let stored = media.alt_text.clone().filter(|text| !text.is_empty());
    if stored.is_some() {
        return Ok(stored);
    }
    let term = setting(pool, MEDIA_ALT_TEXT_PROPERTY).await?;
    let Some(term) = term
        .as_ref()
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
    else {
        return Ok(media.alt_text.clone());
    };
    let Some(property) = catalog.property_id(term) else {
        return Ok(media.alt_text.clone());
    };
    let values = with_db!(pool, |db| ValueRepo::new(db)
        .list_for_property(media.resource.id, property)
        .await)?;
    Ok(values
        .into_iter()
        .next()
        .and_then(|value| value.value)
        .or_else(|| media.alt_text.clone()))
}

/// A user's API settings as one map, `None` when the user stores none.
///
/// # Errors
/// [`ApiError::Internal`] when a row cannot be read.
pub async fn load_user(pool: &DbPool, user: UserId) -> Result<Option<Value>, ApiError> {
    let mut map = Map::new();
    for key in USER_SETTINGS {
        if let Some(value) =
            with_db!(pool, |db| UserSettingRepo::new(db).get(user, key).await).map_err(db_error)?
        {
            map.insert((*key).to_string(), value);
        }
    }
    Ok((!map.is_empty()).then_some(Value::Object(map)))
}

/// Validate an `o:user_settings` member into the writes it asks for. The caller
/// validates before it writes the user row and stores after, so a refused map
/// leaves the user as it was.
///
/// # Errors
/// [`ApiError::Validation`], keyed by `o:user_settings`, for a member that is
/// not an object, a key the API does not expose, or a value of the wrong shape.
pub fn parse_user(value: &Value) -> Result<Vec<Change>, ApiError> {
    parse_map(USER_SETTINGS_MEMBER, USER_SETTINGS, value)
}

/// Apply the writes [`parse_user`] returned.
///
/// # Errors
/// [`ApiError::Internal`] for a storage failure.
pub async fn store_user(pool: &DbPool, user: UserId, changes: &[Change]) -> Result<(), ApiError> {
    for (key, value) in changes {
        match value {
            Some(value) => with_db!(pool, |db| UserSettingRepo::new(db)
                .set(user, key, value)
                .await)
            .map_err(db_error)?,
            None => {
                with_db!(pool, |db| UserSettingRepo::new(db).delete(user, key).await)
                    .map_err(db_error)?;
            }
        }
    }
    Ok(())
}

/// The file validation settings as they apply, upstream's
/// `ValidatorFactory`: each list as stored, else upstream's default, and the
/// switch by PHP truthiness, because an adopted Omeka S database stores the
/// checkbox as `"1"` or `"0"`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSettings {
    media_types: Vec<String>,
    extensions: Vec<String>,
    disabled: bool,
}

impl FileSettings {
    async fn load(pool: &DbPool) -> Result<Self, DbError> {
        let list = |stored: Option<Value>, default: &[&str]| match stored {
            Some(Value::Array(names)) => names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => default.iter().map(|name| (*name).to_string()).collect(),
        };
        Ok(Self {
            media_types: list(
                setting(pool, MEDIA_TYPE_WHITELIST).await?,
                DEFAULT_MEDIA_TYPES,
            ),
            extensions: list(
                setting(pool, EXTENSION_WHITELIST).await?,
                DEFAULT_EXTENSIONS,
            ),
            disabled: setting(pool, DISABLE_FILE_VALIDATION)
                .await?
                .is_some_and(|value| php_truthy(&value)),
        })
    }
}

/// The file validation every ingest path checks a file against.
///
/// # Errors
/// [`DbError`] when a setting row cannot be read.
pub async fn file_validation(pool: &DbPool) -> Result<FileValidation, DbError> {
    let settings = FileSettings::load(pool).await?;
    Ok(if settings.disabled {
        FileValidation::Disabled
    } else {
        FileValidation::Allow {
            media_types: settings.media_types,
            extensions: settings.extensions,
        }
    })
}

/// The address policy every fetch an ingester makes runs under: private
/// ranges only when the operator stored [`IMPORT_ALLOW_PRIVATE_URLS`].
///
/// # Errors
/// [`DbError`] when the setting row cannot be read.
pub async fn address_policy(pool: &DbPool) -> Result<AddressPolicy, DbError> {
    Ok(if allows_private_urls(pool).await? {
        AddressPolicy::AllowPrivate
    } else {
        AddressPolicy::PublicOnly
    })
}

/// The upload limits and the staging retention: each count as stored, else
/// its [`UPLOAD_DEFAULTS`] value. A stored value that is not a whole number
/// above zero reads as the default, so a hand-edited row cannot switch a
/// limit off.
///
/// # Errors
/// [`DbError`] when a setting row cannot be read.
pub async fn upload_settings(pool: &DbPool) -> Result<UploadSettings, DbError> {
    let count =
        |stored: Option<Value>, default: u64| stored.as_ref().and_then(count_of).unwrap_or(default);
    Ok(UploadSettings {
        max_file: count(
            setting(pool, IMPORT_UPLOAD_MAX_FILE).await?,
            UPLOAD_DEFAULTS.max_file,
        ),
        max_total: count(
            setting(pool, IMPORT_UPLOAD_MAX_TOTAL).await?,
            UPLOAD_DEFAULTS.max_total,
        ),
        staging_keep_days: count(
            setting(pool, IMPORT_STAGING_KEEP_DAYS).await?,
            UPLOAD_DEFAULTS.staging_keep_days,
        ),
        draft_staging_keep_days: count(
            setting(pool, IMPORT_DRAFT_STAGING_KEEP_DAYS).await?,
            UPLOAD_DEFAULTS.draft_staging_keep_days,
        ),
    })
}

/// A whole number above zero, as a JSON number or a string of digits.
fn count_of(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|count| *count > 0)
}

async fn allows_private_urls(pool: &DbPool) -> Result<bool, DbError> {
    Ok(setting(pool, IMPORT_ALLOW_PRIVATE_URLS)
        .await?
        .is_some_and(|value| php_truthy(&value)))
}

async fn setting(pool: &DbPool, id: &str) -> Result<Option<Value>, DbError> {
    with_db!(pool, |db| SettingRepo::new(db).get(id).await)
}

fn php_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        Value::String(text) => !text.is_empty() && text != "0",
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        Value::Null => false,
    }
}

/// `GET /api/settings`: the global API settings as one map, for a global
/// admin. The three file validation settings and the private URL switch
/// always answer, as they apply.
///
/// # Errors
/// [`ApiError::PermissionDenied`] for any other caller, [`ApiError::Internal`]
/// when a row cannot be read.
pub async fn read_global(api: &Api<'_>) -> Result<ApiResponse, ApiError> {
    authorize_global(api.actor())?;
    let pool = api.pool();
    let mut map = Map::new();
    if let Some(value) = setting(pool, LOCALE).await.map_err(db_error)? {
        map.insert(LOCALE.to_string(), value);
    }
    if let Some(value) = setting(pool, MEDIA_ALT_TEXT_PROPERTY)
        .await
        .map_err(db_error)?
    {
        map.insert(MEDIA_ALT_TEXT_PROPERTY.to_string(), value);
    }
    let files = FileSettings::load(pool).await.map_err(db_error)?;
    map.insert(MEDIA_TYPE_WHITELIST.to_string(), files.media_types.into());
    map.insert(EXTENSION_WHITELIST.to_string(), files.extensions.into());
    map.insert(DISABLE_FILE_VALIDATION.to_string(), files.disabled.into());
    map.insert(
        IMPORT_ALLOW_PRIVATE_URLS.to_string(),
        allows_private_urls(pool).await.map_err(db_error)?.into(),
    );
    let uploads = upload_settings(pool).await.map_err(db_error)?;
    for (key, value) in [
        (IMPORT_UPLOAD_MAX_FILE, uploads.max_file),
        (IMPORT_UPLOAD_MAX_TOTAL, uploads.max_total),
        (IMPORT_STAGING_KEEP_DAYS, uploads.staging_keep_days),
        (
            IMPORT_DRAFT_STAGING_KEEP_DAYS,
            uploads.draft_staging_keep_days,
        ),
    ] {
        map.insert(key.to_string(), value.into());
    }
    Ok(ApiResponse::document(serialize(&Value::Object(map))?))
}

/// `PATCH /api/settings`: merge a map into the global settings, for a global
/// admin, and answer the stored map. A key the map omits keeps its value; a
/// `null` or an empty string removes the row.
///
/// # Errors
/// [`ApiError::PermissionDenied`] for any other caller, a 400 for a body that
/// is not a JSON object, [`ApiError::Validation`] for a map [`read_global`]
/// could not have answered, [`ApiError::Internal`] for a storage failure.
pub async fn write_global(api: &Api<'_>, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    authorize_global(api.actor())?;
    let body: Value =
        serde_json::from_slice(bytes).map_err(|_| ApiError::bad_request(messages::JSON_SYNTAX))?;
    for (key, value) in parse_map("settings", GLOBAL_SETTINGS, &body)? {
        match value {
            Some(value) => with_db!(api.pool(), |db| SettingRepo::new(db).set(key, &value).await)
                .map_err(db_error)?,
            None => {
                with_db!(api.pool(), |db| SettingRepo::new(db).delete(key).await)
                    .map_err(db_error)?;
            }
        }
    }
    read_global(api).await
}

fn authorize_global(actor: &Actor) -> Result<(), ApiError> {
    if actor.role() == Some("global_admin") {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied {
            message: messages::SETTINGS_DENIED.to_string(),
        })
    }
}

fn parse_map(
    member: &str,
    known: &'static [&'static str],
    value: &Value,
) -> Result<Vec<Change>, ApiError> {
    let invalid = |message: String| {
        let mut errors = ValidationErrors::new();
        errors.insert(member.to_string(), vec![message]);
        ApiError::Validation(errors)
    };
    let Value::Object(map) = value else {
        return Err(invalid(messages::SETTINGS_NOT_OBJECT.to_string()));
    };
    let mut changes = Vec::with_capacity(map.len());
    for (key, stored) in map {
        let Some(name) = known.iter().find(|name| **name == key.as_str()) else {
            return Err(invalid(messages::setting_unavailable(key)));
        };
        let change = match (shape(name), stored) {
            (_, Value::Null) => None,
            (Shape::Text, Value::String(text)) if text.is_empty() => None,
            (Shape::Flag, Value::Bool(false)) => None,
            (Shape::Names, Value::Array(names)) if names.is_empty() => None,
            (Shape::Text, Value::String(_)) | (Shape::Flag, Value::Bool(true)) => {
                Some(stored.clone())
            }
            (Shape::Names, Value::Array(names)) if names.iter().all(Value::is_string) => {
                Some(stored.clone())
            }
            (Shape::AllowList, Value::Array(names)) if names.iter().all(Value::is_string) => {
                Some(allow_list(names))
            }
            (Shape::Count, Value::Number(number))
                if number.as_u64().is_some_and(|count| count > 0) =>
            {
                Some(stored.clone())
            }
            (Shape::Text, _) => return Err(invalid(messages::setting_not_text(key))),
            (Shape::Flag, _) => return Err(invalid(messages::setting_not_flag(key))),
            (Shape::Names | Shape::AllowList, _) => {
                return Err(invalid(messages::setting_not_names(key)));
            }
            (Shape::Count, _) => return Err(invalid(messages::setting_not_count(key))),
        };
        changes.push((*name, change));
    }
    Ok(changes)
}

/// `SettingForm`'s filter for the two allow lists: trim each entry, drop the
/// empty ones and the repeats, keep the first order.
fn allow_list(names: &[Value]) -> Value {
    let mut kept: Vec<String> = Vec::with_capacity(names.len());
    for name in names.iter().filter_map(Value::as_str).map(str::trim) {
        if !name.is_empty() && !kept.iter().any(|seen| seen == name) {
            kept.push(name.to_string());
        }
    }
    kept.into()
}

fn non_empty(value: Option<Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) if !text.is_empty() => Some(text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn a_map_names_only_the_exposed_keys_and_strings() {
        assert_eq!(
            parse_user(&json!({"locale": "es"})).unwrap(),
            vec![(LOCALE, Some(json!("es")))]
        );
        assert_eq!(
            parse_user(&json!({"locale": ""})).unwrap(),
            vec![(LOCALE, None)],
            "an empty locale is the Default option, which removes the row"
        );
        assert!(parse_user(&json!({"default_resource_template": 1})).is_err());
        assert!(parse_user(&json!({"locale": 3})).is_err());
        assert!(parse_user(&json!(["locale"])).is_err());
    }

    #[test]
    fn an_allow_list_is_trimmed_and_kept_even_when_empty() {
        assert_eq!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"extension_whitelist": [" pdf", "jpg", "", "pdf"]})
            )
            .unwrap(),
            vec![(EXTENSION_WHITELIST, Some(json!(["pdf", "jpg"])))]
        );
        assert_eq!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"media_type_whitelist": []})
            )
            .unwrap(),
            vec![(MEDIA_TYPE_WHITELIST, Some(json!([])))],
            "an empty list refuses every file, so it is stored"
        );
        assert_eq!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"media_type_whitelist": null})
            )
            .unwrap(),
            vec![(MEDIA_TYPE_WHITELIST, None)],
            "null removes the row, which restores the default"
        );
        assert!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"extension_whitelist": "pdf,jpg"})
            )
            .is_err()
        );
    }

    #[test]
    fn an_upload_count_is_a_whole_number_above_zero() {
        assert_eq!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"import_upload_max_file": 1_048_576})
            )
            .unwrap(),
            vec![(IMPORT_UPLOAD_MAX_FILE, Some(json!(1_048_576)))]
        );
        assert_eq!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"import_staging_keep_days": null})
            )
            .unwrap(),
            vec![(IMPORT_STAGING_KEEP_DAYS, None)],
            "null restores the default"
        );
        for refused in [json!(0), json!(-1), json!(1.5), json!("30"), json!(true)] {
            assert!(
                parse_map(
                    "settings",
                    GLOBAL_SETTINGS,
                    &json!({"import_draft_staging_keep_days": refused})
                )
                .is_err(),
                "{refused}"
            );
        }
        assert_eq!(
            count_of(&json!("12")),
            Some(12),
            "a stored string of digits reads"
        );
        assert_eq!(count_of(&json!(0)), None, "zero reads as the default");
    }

    #[test]
    fn the_lesson_keys_are_user_settings_of_their_own_shape() {
        // The writes are independent rows, so their order is not part of the answer.
        let sorted = |value: Value| {
            let mut changes = parse_user(&value).unwrap();
            changes.sort_by_key(|(key, _)| *key);
            changes
        };
        assert_eq!(
            sorted(json!({"romeka_lessons_seen": ["term"], "romeka_hide_lessons": true})),
            vec![
                (HIDE_LESSONS, Some(json!(true))),
                (LESSONS_SEEN, Some(json!(["term"]))),
            ]
        );
        assert_eq!(
            sorted(json!({"romeka_lessons_seen": [], "romeka_hide_lessons": false})),
            vec![(HIDE_LESSONS, None), (LESSONS_SEEN, None)],
            "the default value removes the row"
        );
        assert!(parse_user(&json!({"romeka_lessons_seen": [1]})).is_err());
        assert!(parse_user(&json!({"romeka_hide_lessons": "true"})).is_err());
        assert!(parse_user(&json!({"locale": true})).is_err());
        assert!(
            parse_map(
                "settings",
                GLOBAL_SETTINGS,
                &json!({"romeka_hide_lessons": true})
            )
            .is_err(),
            "lessons are a user setting only"
        );
    }
}
