//! `romeka user`: add, invite, list, set a password and grant admin.
//!
//! Rows speak the `user` table (`Entity/User.php`). The role column holds one of
//! the six `Role` wire names; `--role` is validated at this boundary so a typo
//! fails loudly instead of writing an unknown role the permission matrix cannot
//! map.
//!
//! **Invite.** `romeka user add` with no `--password` is upstream's admin "add
//! user": the account starts inactive and holds no password, a one-time key is
//! minted with `activate` set, and the command prints the set-password link.
//! `Admin\UserController::addAction` mails that link; the CLI prints it, so an
//! installation with no SMTP relay can still onboard a person, and the admin
//! hands it over. With `--password` the account is created active and complete,
//! which is what the command did before and what a scripted install needs.

use omeka_api::login;
use omeka_api::password;
use omeka_db::Location;
use omeka_db::repos::{Paging, UserFilter, UserRepo};
use omeka_db::rows::{NewUser, UserRow};
use omeka_db::with_db;

use omeka_core::permissions::Role;
use omeka_server::mail::SET_PASSWORD_PATH;

use crate::error::CliError;

/// Input for `romeka user add`.
#[derive(Debug, Clone)]
pub struct AddOptions {
    pub database: Location,
    pub email: String,
    pub name: String,
    /// The password to store. `None` makes the command an invite.
    pub password: Option<String>,
    pub role: String,
    /// Origin the invite link is built from.
    pub base_url: String,
}

/// Input for `romeka user list`.
#[derive(Debug, Clone)]
pub struct ListOptions {
    pub database: Location,
    pub role: Option<String>,
}

/// Input for `romeka user set-password`.
#[derive(Debug, Clone)]
pub struct SetPasswordOptions {
    pub database: Location,
    pub email: String,
    pub password: String,
}

/// Input for `romeka user grant-admin`.
#[derive(Debug, Clone)]
pub struct GrantAdminOptions {
    pub database: Location,
    pub email: String,
}

/// What `romeka user add` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedUser {
    pub user: UserRow,
    /// The set-password link an invite minted, absent when a password was given.
    pub invite: Option<Invite>,
}

/// A link that sets a password and, for an activation key, activates the account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub key: String,
    pub link: String,
}

/// Add a user, rejecting an email that already has an account.
///
/// # Errors
/// A [`CliError::Message`] for an unknown role, a duplicate email, or a missing
/// random source for the invite key.
pub async fn add(options: &AddOptions) -> Result<AddedUser, CliError> {
    let role = parse_role(&options.role)?;
    let pool = options.database.open().await?;
    if with_db!(&pool, |db| UserRepo::new(db)
        .get_by_email(&options.email)
        .await)?
    .is_some()
    {
        return Err(CliError::message(format!(
            "a user with email \"{}\" already exists",
            options.email
        )));
    }
    let password_hash = match &options.password {
        Some(plain) => Some(password::hash(plain)?),
        None => None,
    };
    let user = with_db!(&pool, |db| UserRepo::new(db)
        .create(&NewUser {
            email: options.email.clone(),
            name: options.name.clone(),
            password_hash,
            role: role.wire_name().to_string(),
            // An invite starts inactive and the activation key promotes it,
            // which is the state `Admin\UserController::addAction` leaves.
            is_active: options.password.is_some(),
            created: None,
        })
        .await)?;
    if options.password.is_some() {
        return Ok(AddedUser { user, invite: None });
    }
    let issued = login::issue_key(&pool, &user, true).await?;
    Ok(AddedUser {
        user,
        invite: Some(Invite {
            link: format!("{}{SET_PASSWORD_PATH}{}", options.base_url, issued.key),
            key: issued.key,
        }),
    })
}

/// List users, optionally narrowed to one role.
///
/// # Errors
/// A [`CliError::Message`] for an unknown role.
pub async fn list(options: &ListOptions) -> Result<Vec<UserRow>, CliError> {
    let role = match &options.role {
        Some(name) => Some(parse_role(name)?.wire_name().to_string()),
        None => None,
    };
    let pool = options.database.open().await?;
    with_db!(&pool, |db| UserRepo::new(db)
        .list(
            &UserFilter {
                role,
                ..UserFilter::default()
            },
            &Paging::default()
        )
        .await)
    .map_err(CliError::from)
}

/// Replace a user's password hash.
///
/// # Errors
/// A [`CliError::Message`] when no account has this email.
pub async fn set_password(options: &SetPasswordOptions) -> Result<UserRow, CliError> {
    let pool = options.database.open().await?;
    let mut user = find_by_email(&pool, &options.email).await?;
    user.password_hash = Some(password::hash(&options.password)?);
    with_db!(&pool, |db| UserRepo::new(db).update(&user).await)?;
    Ok(user)
}

/// Set a user's role to `global_admin`.
///
/// # Errors
/// A [`CliError::Message`] when no account has this email.
pub async fn grant_admin(options: &GrantAdminOptions) -> Result<UserRow, CliError> {
    let pool = options.database.open().await?;
    let mut user = find_by_email(&pool, &options.email).await?;
    user.role = Role::GlobalAdmin.wire_name().to_string();
    with_db!(&pool, |db| UserRepo::new(db).update(&user).await)?;
    Ok(user)
}

async fn find_by_email(pool: &omeka_db::DbPool, email: &str) -> Result<UserRow, CliError> {
    with_db!(pool, |db| UserRepo::new(db).get_by_email(email).await)?
        .ok_or_else(|| CliError::message(format!("no user with email \"{email}\"")))
}

fn parse_role(name: &str) -> Result<Role, CliError> {
    Role::from_wire_name(name).ok_or_else(|| CliError::message(format!("unknown role \"{name}\"")))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::temp_db;

    const BASE_URL: &str = "http://localhost:8080";

    fn add_options(database: PathBuf, email: &str, role: &str) -> AddOptions {
        AddOptions {
            database: database.into(),
            email: email.to_string(),
            name: "A Person".to_string(),
            password: Some("secret".to_string()),
            role: role.to_string(),
            base_url: BASE_URL.to_string(),
        }
    }

    fn invite_options(database: PathBuf, email: &str, role: &str) -> AddOptions {
        AddOptions {
            password: None,
            ..add_options(database, email, role)
        }
    }

    #[tokio::test]
    async fn add_then_list_shows_the_user_and_role() {
        let db = temp_db().await;
        let created = add(&add_options(
            db.path.clone(),
            "editor@example.org",
            "editor",
        ))
        .await
        .expect("add");
        assert_eq!(created.user.role, "editor");
        assert!(
            created.user.is_active,
            "an explicit password makes the user active"
        );
        assert!(created.invite.is_none());

        let users = list(&ListOptions {
            database: db.path.clone().into(),
            role: None,
        })
        .await
        .expect("list");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].email, "editor@example.org");

        let editors = list(&ListOptions {
            database: db.path.clone().into(),
            role: Some("editor".to_string()),
        })
        .await
        .expect("list editors");
        assert_eq!(editors.len(), 1);
        let researchers = list(&ListOptions {
            database: db.path.clone().into(),
            role: Some("researcher".to_string()),
        })
        .await
        .expect("list researchers");
        assert!(researchers.is_empty());
    }

    #[tokio::test]
    async fn add_rejects_a_duplicate_email() {
        let db = temp_db().await;
        add(&add_options(db.path.clone(), "dupe@example.org", "author"))
            .await
            .expect("first add");
        let error = add(&add_options(db.path.clone(), "dupe@example.org", "editor"))
            .await
            .expect_err("duplicate is rejected");
        assert!(error.to_string().contains("already exists"), "got {error}");
    }

    #[tokio::test]
    async fn add_rejects_an_unknown_role() {
        let db = temp_db().await;
        let error = add(&add_options(db.path.clone(), "x@example.org", "superuser"))
            .await
            .expect_err("unknown role is rejected");
        assert!(error.to_string().contains("unknown role"), "got {error}");
    }

    #[tokio::test]
    async fn set_password_replaces_the_stored_hash() {
        let db = temp_db().await;
        add(&add_options(
            db.path.clone(),
            "pw@example.org",
            "researcher",
        ))
        .await
        .expect("add");
        let updated = set_password(&SetPasswordOptions {
            database: db.path.clone().into(),
            email: "pw@example.org".to_string(),
            password: "new-secret".to_string(),
        })
        .await
        .expect("set password");
        let stored = updated.password_hash.expect("a hash");
        assert!(omeka_api::password::verify("new-secret", Some(&stored)));
        assert!(!omeka_api::password::verify("secret", Some(&stored)));
    }

    #[tokio::test]
    async fn set_password_fails_for_an_unknown_user() {
        let db = temp_db().await;
        let error = set_password(&SetPasswordOptions {
            database: db.path.clone().into(),
            email: "missing@example.org".to_string(),
            password: "secret".to_string(),
        })
        .await
        .expect_err("unknown user is rejected");
        assert!(error.to_string().contains("no user"), "got {error}");
    }

    #[tokio::test]
    async fn grant_admin_promotes_the_role() {
        let db = temp_db().await;
        add(&add_options(
            db.path.clone(),
            "plain@example.org",
            "researcher",
        ))
        .await
        .expect("add");
        let promoted = grant_admin(&GrantAdminOptions {
            database: db.path.clone().into(),
            email: "plain@example.org".to_string(),
        })
        .await
        .expect("grant admin");
        assert_eq!(promoted.role, "global_admin");

        let admins = list(&ListOptions {
            database: db.path.clone().into(),
            role: Some("global_admin".to_string()),
        })
        .await
        .expect("list admins");
        assert_eq!(admins.len(), 1);
    }

    #[tokio::test]
    async fn grant_admin_fails_for_an_unknown_user() {
        let db = temp_db().await;
        let error = grant_admin(&GrantAdminOptions {
            database: db.path.clone().into(),
            email: "ghost@example.org".to_string(),
        })
        .await
        .expect_err("unknown user is rejected");
        assert!(error.to_string().contains("no user"), "got {error}");
    }

    #[tokio::test]
    async fn add_without_a_password_invites_and_prints_a_link_that_works() {
        let db = temp_db().await;
        let added = add(&invite_options(
            db.path.clone(),
            "new@example.org",
            "editor",
        ))
        .await
        .expect("invite");
        assert!(
            !added.user.is_active,
            "an invited account waits for the activation"
        );
        assert_eq!(added.user.password_hash, None);

        let invite = added.invite.expect("an invite");
        assert_eq!(
            invite.link,
            format!("{BASE_URL}/#/password/{}", invite.key),
            "the link the SPA renders"
        );
        assert_eq!(invite.key.len(), omeka_api::token::TOKEN_LENGTH);

        // The link's key is what the set-password route spends: it sets the
        // password and activates the account.
        let spent = omeka_api::login::set_password(
            &db.db_pool,
            &omeka_api::password::PasswordRules::default(),
            &invite.key,
            "hunter2",
        )
        .await
        .expect("spend the link");
        assert!(spent.is_active, "the activation key promotes the user");
        assert!(omeka_api::password::verify(
            "hunter2",
            spent.password_hash.as_deref()
        ));

        // And the key is spent: a second attempt is refused.
        let again = omeka_api::login::set_password(
            &db.db_pool,
            &omeka_api::password::PasswordRules::default(),
            &invite.key,
            "hunter3",
        )
        .await;
        assert!(again.is_err(), "a one-time key works once");
    }

    #[tokio::test]
    async fn each_invite_holds_its_own_key() {
        let db = temp_db().await;
        let first = add(&invite_options(
            db.path.clone(),
            "new@example.org",
            "author",
        ))
        .await
        .expect("invite");
        let first_key = first.invite.expect("an invite").key;

        let second = add(&invite_options(
            db.path.clone(),
            "other@example.org",
            "author",
        ))
        .await
        .expect("invite");
        let second_key = second.invite.expect("an invite").key;
        assert_ne!(first_key, second_key);
        let keys = omeka_db::repos::DatabaseRepo::new(&db.pool)
            .count_rows("password_creation")
            .await
            .expect("count keys");
        assert_eq!(keys, 2, "one live key per invited user");
    }
}
