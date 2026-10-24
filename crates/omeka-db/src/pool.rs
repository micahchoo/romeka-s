//! The one pool type the rest of the workspace holds, and the database location it opens.
//!
//! `DESIGN.md` fixes a deployment on one engine, chosen when the process starts. [`DbPool`] holds
//! that engine's pool, and [`with_db!`](crate::with_db) is the one place a caller crosses from it to
//! a repository: the macro matches the engine and runs the same repository call in each arm, so
//! the call is compiled once per engine and the code around it once. The README's "Where the
//! engine is chosen" gives the measured reason this beats a `DB: Backend` parameter through every
//! caller.

use std::fmt;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::DbError;

/// A pool on the engine this deployment runs.
#[derive(Debug, Clone)]
pub enum DbPool {
    Sqlite(SqlitePool),
    #[cfg(feature = "mysql")]
    MySql(sqlx::MySqlPool),
}

impl DbPool {
    /// Close every connection, waiting for those in use to come back. A SQLite file then
    /// checkpoints its write-ahead log as the last connection closes, so a stopped server
    /// leaves one file, not a log to replay.
    pub async fn close(&self) {
        match self {
            Self::Sqlite(pool) => pool.close().await,
            #[cfg(feature = "mysql")]
            Self::MySql(pool) => pool.close().await,
        }
    }
}

impl From<SqlitePool> for DbPool {
    fn from(pool: SqlitePool) -> Self {
        Self::Sqlite(pool)
    }
}

#[cfg(feature = "mysql")]
impl From<sqlx::MySqlPool> for DbPool {
    fn from(pool: sqlx::MySqlPool) -> Self {
        Self::MySql(pool)
    }
}

/// Run `$body` with `$name` bound to the engine's `&sqlx::Pool<_>`.
///
/// ```ignore
/// let row = with_db!(pool, |db| ItemRepo::new(db).get(id).await)?;
/// ```
///
/// The body is compiled once per engine. Both arms must have the same type, which they do for
/// every repository read and write: a row type never names its engine. The `|db|` is not a
/// closure; it is spelled like one so rustfmt formats the call, and the body may `.await` and `?`
/// in the caller's function.
#[cfg(feature = "mysql")]
#[macro_export]
macro_rules! with_db {
    ($pool:expr, |$name:ident| $body:expr) => {
        match $pool {
            $crate::DbPool::Sqlite($name) => $body,
            $crate::DbPool::MySql($name) => $body,
        }
    };
}

/// Run `$body` with `$name` bound to the engine's `&sqlx::Pool<_>`. Without the `mysql` feature
/// SQLite is the only engine.
#[cfg(not(feature = "mysql"))]
#[macro_export]
macro_rules! with_db {
    ($pool:expr, |$name:ident| $body:expr) => {
        match $pool {
            $crate::DbPool::Sqlite($name) => $body,
        }
    };
}

/// Where the database is: a SQLite file, or a server URL.
#[derive(Clone, PartialEq, Eq)]
pub enum Location {
    File(PathBuf),
    /// A `mysql://user:password@host:port/database` URL. The password never reaches
    /// [`fmt::Display`] or [`fmt::Debug`].
    Url(String),
}

impl Location {
    /// A `mysql://` or `mariadb://` URL names a server; anything else is a SQLite file path. A
    /// `sqlite:` prefix is dropped, so `sqlite:///srv/omeka.sqlite` and `/srv/omeka.sqlite` are one
    /// file.
    pub fn parse(text: &str) -> Self {
        let lower = text.to_ascii_lowercase();
        if lower.starts_with("mysql://") || lower.starts_with("mariadb://") {
            return Self::Url(text.to_string());
        }
        let path = text
            .strip_prefix("sqlite://")
            .or_else(|| text.strip_prefix("sqlite:"))
            .unwrap_or(text);
        Self::File(PathBuf::from(path))
    }

    /// The SQLite file, or `None` for a server.
    pub fn file(&self) -> Option<&Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Url(_) => None,
        }
    }

    /// Open a pool, creating a SQLite file that does not exist yet.
    ///
    /// # Errors
    /// A [`DbError`] when the file or the server cannot be opened, or
    /// [`DbError::Unsupported`] for a server URL in a build without the `mysql` feature.
    pub async fn open(&self) -> Result<DbPool, DbError> {
        match self {
            Self::File(path) => Ok(DbPool::Sqlite(crate::open_file(path).await?)),
            Self::Url(url) => open_url(url).await,
        }
    }
}

#[cfg(feature = "mysql")]
async fn open_url(url: &str) -> Result<DbPool, DbError> {
    // sqlx parses only the `mysql` scheme; MariaDB speaks the same protocol.
    let url = match url.get(..10) {
        Some(scheme) if scheme.eq_ignore_ascii_case("mariadb://") => {
            format!("mysql://{}", &url[10..])
        }
        _ => url.to_string(),
    };
    Ok(DbPool::MySql(crate::open_mysql(&url).await?))
}

#[cfg(not(feature = "mysql"))]
async fn open_url(_: &str) -> Result<DbPool, DbError> {
    Err(DbError::Unsupported {
        engine: "mysql",
        operation: "open",
        reason: "this build has no MySQL driver; build omeka-db with the `mysql` feature",
    })
}

impl From<PathBuf> for Location {
    fn from(path: PathBuf) -> Self {
        Self::File(path)
    }
}

impl From<&Path> for Location {
    fn from(path: &Path) -> Self {
        Self::File(path.to_path_buf())
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => write!(f, "{}", path.display()),
            Self::Url(url) => f.write_str(&redacted(url)),
        }
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => f.debug_tuple("File").field(path).finish(),
            Self::Url(url) => f.debug_tuple("Url").field(&redacted(url)).finish(),
        }
    }
}

/// The URL with the password between `user:` and `@` replaced, for logs and errors.
fn redacted(url: &str) -> String {
    let Some(start) = url.find("://").map(|index| index + 3) else {
        return url.to_string();
    };
    let authority_end = url[start..]
        .find('/')
        .map_or(url.len(), |index| start + index);
    let Some(at) = url[start..authority_end]
        .rfind('@')
        .map(|index| start + index)
    else {
        return url.to_string();
    };
    match url[start..at].find(':').map(|index| start + index) {
        Some(colon) => format!("{}:***{}", &url[..colon], &url[at..]),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_url_is_a_url_and_anything_else_a_file() {
        assert_eq!(
            Location::parse("mysql://u:p@h:3306/omeka"),
            Location::Url("mysql://u:p@h:3306/omeka".to_string())
        );
        assert_eq!(
            Location::parse("sqlite:///srv/omeka.sqlite"),
            Location::File(PathBuf::from("/srv/omeka.sqlite"))
        );
        assert_eq!(
            Location::parse("omeka.sqlite"),
            Location::File(PathBuf::from("omeka.sqlite"))
        );
    }

    #[test]
    fn the_password_never_prints() {
        let location = Location::parse("mysql://romeka:s3cret@db:3306/omeka?ssl-mode=required");
        assert_eq!(
            location.to_string(),
            "mysql://romeka:***@db:3306/omeka?ssl-mode=required"
        );
        assert!(!format!("{location:?}").contains("s3cret"));
        assert_eq!(
            Location::parse("mysql://db/omeka").to_string(),
            "mysql://db/omeka"
        );
    }
}
