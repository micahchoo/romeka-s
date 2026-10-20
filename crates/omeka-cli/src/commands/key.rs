//! `romeka key`: mint, list and revoke API keys.
//!
//! An API key is a row of `api_key` (`Entity/ApiKey.php`). The identity is 32
//! alphanumeric characters and is the primary key. The credential is 32 more and
//! only its bcrypt hash is stored, because `Authentication\Adapter\KeyAdapter`
//! verifies it with `password_verify` and `omeka-api::auth` verifies it the same
//! way. The owner must exist and be active, which `KeyAdapter` also requires.
//!
//! [`MintedKey`] is the only type that carries the plaintext credential, and
//! [`add`] is the only function that returns one. [`KeySummary`] cannot represent
//! a credential at all, so `list` has nothing to leak.

use chrono::NaiveDateTime;

use omeka_api::auth::API_KEY_LENGTH;
use omeka_api::token;
use omeka_db::Location;
use omeka_db::repos::{ApiKeyRepo, Paging, UserFilter, UserRepo};
use omeka_db::rows::NewApiKey;
use omeka_db::with_db;

use crate::error::CliError;

/// The bcrypt cost of a stored credential. Upstream's `setCredential` calls
/// `password_hash` with the runtime default, which is 10 on the oracle's PHP 8.2
/// (`oracle/app.Dockerfile`). The cost travels inside the hash, so a row at any
/// cost still verifies; this one matches the oracle.
const BCRYPT_COST: u32 = 10;

/// Input for `romeka key add`.
#[derive(Debug, Clone)]
pub struct AddOptions {
    pub database: Location,
    /// The owner's email, as `user.email` stores it.
    pub owner: String,
    pub label: String,
}

/// Input for `romeka key list`.
#[derive(Debug, Clone)]
pub struct ListOptions {
    pub database: Location,
}

/// Input for `romeka key revoke`.
#[derive(Debug, Clone)]
pub struct RevokeOptions {
    pub database: Location,
    pub identity: String,
}

/// What `romeka key add` prints once: the pair an API client sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintedKey {
    pub identity: String,
    pub credential: String,
    pub owner_email: String,
}

/// One row of `romeka key list`. There is no credential field to omit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySummary {
    pub identity: String,
    pub owner_email: String,
    pub label: String,
    pub created: NaiveDateTime,
    pub last_accessed: Option<NaiveDateTime>,
}

/// Mint a key for an active user, storing only the credential's hash.
///
/// # Errors
/// A [`CliError::Message`] when no account has this email, when the account is
/// inactive, or when the operating system has no randomness.
pub async fn add(options: &AddOptions) -> Result<MintedKey, CliError> {
    let pool = options.database.open().await?;
    let Some(owner) = with_db!(&pool, |db| UserRepo::new(db)
        .get_by_email(&options.owner)
        .await)?
    else {
        return Err(CliError::message(format!(
            "no user with email \"{}\"",
            options.owner
        )));
    };
    if !owner.is_active {
        return Err(CliError::message(format!(
            "user \"{}\" is inactive",
            options.owner
        )));
    }
    let identity = random_string()?;
    let credential = random_string()?;
    let credential_hash = bcrypt::hash(&credential, BCRYPT_COST)
        .map_err(|error| CliError::message(format!("cannot hash the credential: {error}")))?;
    with_db!(&pool, |db| ApiKeyRepo::new(db)
        .create(&NewApiKey {
            id: identity.clone(),
            owner_id: owner.id,
            label: options.label.clone(),
            credential_hash,
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await)?;
    Ok(MintedKey {
        identity,
        credential,
        owner_email: owner.email,
    })
}

/// Every key with its owner's email, ordered by identity.
///
/// # Errors
/// A repository failure.
pub async fn list(options: &ListOptions) -> Result<Vec<KeySummary>, CliError> {
    let pool = options.database.open().await?;
    let users = with_db!(&pool, |db| UserRepo::new(db)
        .list(&UserFilter::default(), &Paging::default())
        .await)?;
    let mut summaries = Vec::new();
    for user in users {
        for key in with_db!(&pool, |db| ApiKeyRepo::new(db)
            .list_for_owner(user.id)
            .await)?
        {
            summaries.push(KeySummary {
                identity: key.id,
                owner_email: user.email.clone(),
                label: key.label,
                created: key.created,
                last_accessed: key.last_accessed,
            });
        }
    }
    summaries.sort_by(|left, right| left.identity.cmp(&right.identity));
    Ok(summaries)
}

/// Delete a key, whether or not it existed.
///
/// # Errors
/// A repository failure.
pub async fn revoke(options: &RevokeOptions) -> Result<bool, CliError> {
    let pool = options.database.open().await?;
    Ok(with_db!(&pool, |db| ApiKeyRepo::new(db)
        .delete(&options.identity)
        .await)?)
}

/// A fresh identity or credential: 32 characters drawn from the upstream
/// alphabet, all of them equally likely.
///
/// `Entity/ApiKey::getString` draws each character with `random_int`, which is
/// what `omeka_api::token` ports; the password-creation keys use the same
/// generator, as `Entity\PasswordCreation::setId` reuses `ApiKey`'s.
fn random_string() -> Result<String, CliError> {
    token::random_string(API_KEY_LENGTH).map_err(CliError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    use omeka_api::auth::{
        Actor, KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, authenticate, identity_is_well_formed,
    };
    use omeka_db::repos::UserRepo;
    use omeka_db::rows::NewUser;

    use crate::testing::{TempDb, temp_db};

    async fn create_user(db: &TempDb, email: &str, is_active: bool) -> u64 {
        UserRepo::new(&db.pool)
            .create(&NewUser {
                email: email.to_string(),
                name: "A Person".to_string(),
                password_hash: None,
                role: "researcher".to_string(),
                is_active,
                created: None,
            })
            .await
            .expect("create user")
            .id
            .0
    }

    fn add_options(db: &TempDb, owner: &str, label: &str) -> AddOptions {
        AddOptions {
            database: db.path.clone().into(),
            owner: owner.to_string(),
            label: label.to_string(),
        }
    }

    async fn authenticate_pair(db: &TempDb, identity: &str, credential: &str) -> Actor {
        authenticate(
            &db.db_pool,
            [
                (KEY_IDENTITY_PARAM, identity),
                (KEY_CREDENTIAL_PARAM, credential),
            ],
        )
        .await
        .expect("authenticate")
    }

    #[tokio::test]
    async fn minted_key_authenticates_as_its_owner_through_the_key_adapter() {
        let db = temp_db().await;
        let owner = create_user(&db, "owner@example.org", true).await;
        let minted = add(&add_options(&db, "owner@example.org", "cli key"))
            .await
            .expect("mint");

        assert!(
            identity_is_well_formed(&minted.identity),
            "{}",
            minted.identity
        );
        assert!(
            minted.credential.len() == API_KEY_LENGTH
                && minted.credential.chars().all(char::is_alphanumeric),
            "{}",
            minted.credential
        );
        assert_ne!(minted.identity, minted.credential);

        let actor = authenticate_pair(&db, &minted.identity, &minted.credential).await;
        assert_ne!(actor, Actor::anonymous());
        assert_eq!(actor.user(), Some(omeka_db::ids::UserId(owner)));
        assert_eq!(actor.role(), Some("researcher"));
    }

    #[tokio::test]
    async fn wrong_credential_is_anonymous_like_the_key_adapter() {
        let db = temp_db().await;
        create_user(&db, "owner@example.org", true).await;
        let minted = add(&add_options(&db, "owner@example.org", "cli key"))
            .await
            .expect("mint");

        let actor = authenticate_pair(&db, &minted.identity, "not-the-credential").await;
        assert_eq!(actor, Actor::anonymous());

        let unknown = authenticate_pair(&db, &"z".repeat(32), &minted.credential).await;
        assert_eq!(unknown, Actor::anonymous());
    }

    #[tokio::test]
    async fn revoke_removes_the_row_and_a_second_revoke_reports_it_missing() {
        let db = temp_db().await;
        create_user(&db, "owner@example.org", true).await;
        let minted = add(&add_options(&db, "owner@example.org", "cli key"))
            .await
            .expect("mint");

        let removed = revoke(&RevokeOptions {
            database: db.path.clone().into(),
            identity: minted.identity.clone(),
        })
        .await
        .expect("revoke");
        assert!(removed);

        let remaining = list(&ListOptions {
            database: db.path.clone().into(),
        })
        .await
        .expect("list");
        assert!(remaining.is_empty(), "{remaining:?}");

        let actor = authenticate_pair(&db, &minted.identity, &minted.credential).await;
        assert_eq!(actor, Actor::anonymous());

        let again = revoke(&RevokeOptions {
            database: db.path.clone().into(),
            identity: minted.identity.clone(),
        })
        .await
        .expect("second revoke");
        assert!(!again);
    }

    #[tokio::test]
    async fn list_reports_the_label_and_the_last_use_after_authentication() {
        let db = temp_db().await;
        create_user(&db, "owner@example.org", true).await;
        let minted = add(&add_options(&db, "owner@example.org", "cli key"))
            .await
            .expect("mint");

        let before = list(&ListOptions {
            database: db.path.clone().into(),
        })
        .await
        .expect("list");
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].identity, minted.identity);
        assert_eq!(before[0].owner_email, "owner@example.org");
        assert_eq!(before[0].label, "cli key");
        assert_eq!(before[0].last_accessed, None);

        authenticate_pair(&db, &minted.identity, &minted.credential).await;

        let after = list(&ListOptions {
            database: db.path.clone().into(),
        })
        .await
        .expect("list");
        assert!(
            after[0].last_accessed.is_some(),
            "the key adapter records the use"
        );
    }

    #[tokio::test]
    async fn add_refuses_an_unknown_owner() {
        let db = temp_db().await;
        let error = add(&add_options(&db, "ghost@example.org", "cli key"))
            .await
            .expect_err("unknown owner is rejected");
        assert!(error.to_string().contains("no user"), "got {error}");
    }

    #[tokio::test]
    async fn add_refuses_an_inactive_owner() {
        let db = temp_db().await;
        create_user(&db, "gone@example.org", false).await;
        let error = add(&add_options(&db, "gone@example.org", "cli key"))
            .await
            .expect_err("inactive owner is rejected");
        assert!(error.to_string().contains("inactive"), "got {error}");

        let keys = list(&ListOptions {
            database: db.path.clone().into(),
        })
        .await
        .expect("list");
        assert!(keys.is_empty(), "a refused mint writes nothing");
    }

    #[tokio::test]
    async fn two_mints_for_one_user_are_independent_keys() {
        let db = temp_db().await;
        let owner = create_user(&db, "owner@example.org", true).await;
        let first = add(&add_options(&db, "owner@example.org", "first"))
            .await
            .expect("first mint");
        let second = add(&add_options(&db, "owner@example.org", "second"))
            .await
            .expect("second mint");

        assert_ne!(first.identity, second.identity);
        assert_eq!(
            authenticate_pair(&db, &first.identity, &first.credential).await,
            Actor::new(omeka_db::ids::UserId(owner), "researcher")
        );
        assert_eq!(
            authenticate_pair(&db, &second.identity, &second.credential).await,
            Actor::new(omeka_db::ids::UserId(owner), "researcher")
        );
        let crossed = authenticate_pair(&db, &first.identity, &second.credential).await;
        assert_eq!(crossed, Actor::anonymous());

        let keys = list(&ListOptions {
            database: db.path.clone().into(),
        })
        .await
        .expect("list");
        assert_eq!(keys.len(), 2);
    }
}
