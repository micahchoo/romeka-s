//! `POST` and `PUT` for `users`.
//!
//! `UserAdapter::hydrate` sets `o:name`, `o:email` and, when the value changes,
//! `o:role` and `o:is_active`; `validateEntity` requires a name, a valid and
//! unused email, and a role. A new user is inactive and has no password, which
//! matches the upstream entity defaults; the password and activation flow is the
//! admin surface's work, not this handler's.

use omeka_db::ids::UserId;
use omeka_db::repos::UserRepo;
use omeka_db::rows::{NewUser, UserRow};

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::Payload;

const USER_CLASS: &str = "Omeka\\Entity\\User";

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    let name = payload.str_member("o:name").unwrap_or_default().to_string();
    let email = payload
        .str_member("o:email")
        .unwrap_or_default()
        .to_string();
    let role = payload.str_member("o:role").unwrap_or_default().to_string();
    validate(None, api, &name, &email, &role).await?;
    let row = UserRepo::new(api.pool())
        .create(&NewUser {
            email,
            name,
            password_hash: None,
            role,
            is_active: payload.bool_member("o:is_active", false),
            created: None,
        })
        .await
        .map_err(db_error)?;
    api.read_member("users", &row.id.0.to_string()).await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(USER_CLASS, raw_id)?;
    let repo = UserRepo::new(api.pool());
    let Some(mut row) = repo.get(UserId(id)).await.map_err(db_error)? else {
        return Err(not_found(USER_CLASS, raw_id));
    };
    api.authorize_user_update(row.id)?;

    let name = member_or(payload, "o:name", partial, &row.name);
    let email = member_or(payload, "o:email", partial, &row.email);
    let role = payload
        .str_member("o:role")
        .map(str::to_string)
        .unwrap_or_else(|| row.role.clone());
    validate(Some(&row), api, &name, &email, &role).await?;
    row.name = name;
    row.email = email;
    row.role = role;
    if payload.has("o:is_active") {
        row.is_active = payload.bool_member("o:is_active", row.is_active);
    }
    repo.update(&row).await.map_err(db_error)?;
    api.read_member("users", raw_id).await
}

/// Delete a user.
///
/// Only the two admin roles hold `delete` on `UserAdapter`
/// (`AclFactory::addRulesForResearcher` and its siblings grant `read`,
/// `update`, `search` only), so the adapter check in [`crate::read::Api::delete`]
/// is the first gate. The entity rules then deny an admin their own account and
/// a site-admin another admin (`IsSelfAssertion`, `UserIsAdminAssertion`).
///
/// Upstream has no refusal for a user who still owns resources:
/// `User::$keys` cascades remove (`Entity/User.php:59`), the keys go first here,
/// and every other row the user owned keeps its data and loses the reference
/// (`resource.owner_id`, `site.owner_id` and the rest are `ON DELETE SET NULL`).
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(USER_CLASS, raw_id)?;
    let repo = UserRepo::new(api.pool());
    let Some(row) = repo.get(UserId(id)).await.map_err(db_error)? else {
        return Err(not_found(USER_CLASS, raw_id));
    };
    access::user_entity(api.actor(), USER_CLASS, row.id, &row.role)?;
    let answer = crate::read::user_document(api, &row)?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}

/// A member's value, or the stored one when a partial update omits it. On a
/// full update an absent name or email hydrates to the empty string, which is
/// what upstream `setName(getValue('o:name'))` does.
fn member_or(payload: &Payload, key: &str, partial: bool, existing: &str) -> String {
    match (partial && !payload.has(key), payload.str_member(key)) {
        (true, _) => existing.to_string(),
        (false, Some(value)) => value.to_string(),
        (false, None) => String::new(),
    }
}

/// `UserAdapter::validateEntity`.
async fn validate(
    existing: Option<&UserRow>,
    api: &Api<'_>,
    name: &str,
    email: &str,
    role: &str,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if name.is_empty() {
        errors.insert("o:name".to_string(), vec![messages::NAME_EMPTY.to_string()]);
    }
    if !email_is_valid(email) {
        errors.insert(
            "o:email".to_string(),
            vec![messages::INVALID_EMAIL.to_string()],
        );
    } else {
        let taken = match existing {
            Some(row) => row.email != email,
            None => true,
        };
        if taken
            && UserRepo::new(api.pool())
                .get_by_email(email)
                .await
                .map_err(db_error)?
                .is_some()
        {
            errors.insert("o:email".to_string(), vec![messages::email_taken(email)]);
        }
    }
    if role.is_empty() {
        errors.insert(
            "o:role".to_string(),
            vec![messages::USER_ROLE_REQUIRED.to_string()],
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// A minimal email shape check. `UserAdapter` uses `Laminas\Validator\EmailAddress`;
/// this accepts the common case and rejects what upstream rejects in practice.
///
/// The value only decides valid or invalid here. The other Laminas messages
/// (`is not a valid hostname`, the dot-atom and quoted-string details) are not
/// reproduced; an input that reaches one of them is still refused, with
/// [`messages::INVALID_EMAIL`]. Reported as a gap.
fn email_is_valid(email: &str) -> bool {
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !email.chars().any(char::is_whitespace)
}
