//! `POST` and `PUT` for `users`.
//!
//! `UserAdapter::hydrate` sets `o:name`, `o:email` and, when the value changes,
//! `o:role` and `o:is_active`, each behind its own privilege: `change-role`,
//! `change-role-admin` for an admin role, and `activate-user`. The `update`
//! privilege on the row comes first. `validateEntity` requires a name, a valid and
//! unused email, and a role. A new user is inactive and has no password, which
//! matches the upstream entity defaults; the password and activation flow is the
//! admin surface's work, not this handler's.
//!
//! `o:user_settings` is a port addition, the user-settings fieldset upstream's
//! `Admin\UserController::editAction` writes outside the API. It is validated
//! with the user and stored after the row, as upstream stores it after the API
//! update ([`crate::settings`]).

use omeka_core::ids::UserId as CoreUserId;
use omeka_core::permissions::{Action, Context, Role, SubjectKind};
use omeka_db::ids::UserId;
use omeka_db::repos::UserRepo;
use omeka_db::rows::{NewUser, UserRow};
use omeka_db::with_db;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id, taken_or_db_error};
use crate::settings;
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
    let settings = user_settings(payload)?;
    if access::is_admin_role(Some(&role)) {
        authorize(
            api,
            Action::ChangeRoleAdmin,
            "change-role-admin",
            &Context::entity(SubjectKind::User),
        )?;
    }
    validate(None, api, &name, &email, &role).await?;
    let taken = taken_or_db_error("o:email", messages::email_taken(&email));
    let row = with_db!(api.pool(), |db| UserRepo::new(db)
        .create(&NewUser {
            email,
            name,
            password_hash: None,
            role,
            is_active: payload.bool_member("o:is_active", false),
            created: None,
        })
        .await)
    .map_err(taken)?;
    settings::store_user(api.pool(), row.id, &settings).await?;
    api.read_member("users", &row.id.0.to_string()).await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(USER_CLASS, raw_id)?;
    let Some(mut row) =
        with_db!(api.pool(), |db| UserRepo::new(db).get(UserId(id)).await).map_err(db_error)?
    else {
        return Err(not_found(USER_CLASS, raw_id));
    };
    let target = Context::User {
        id: CoreUserId::new(row.id.0),
        // A role outside the six is no admin role, which is all the
        // assertions read from it.
        role: Role::from_wire_name(&row.role).unwrap_or(Role::Researcher),
    };
    authorize(api, Action::Update, "update", &target)?;

    let name = member_or(payload, "o:name", partial, &row.name);
    let email = member_or(payload, "o:email", partial, &row.email);
    let role = payload
        .str_member("o:role")
        .map(str::to_string)
        .unwrap_or_else(|| row.role.clone());
    if !role.is_empty() && role != row.role {
        authorize(api, Action::ChangeRole, "change-role", &target)?;
        if access::is_admin_role(Some(&role)) {
            authorize(api, Action::ChangeRoleAdmin, "change-role-admin", &target)?;
        }
    }
    let is_active = payload.bool_member("o:is_active", row.is_active);
    if is_active != row.is_active {
        authorize(api, Action::ActivateUser, "activate-user", &target)?;
    }
    let settings = user_settings(payload)?;
    validate(Some(&row), api, &name, &email, &role).await?;
    row.name = name;
    row.email = email;
    row.role = role;
    row.is_active = is_active;
    with_db!(api.pool(), |db| UserRepo::new(db).update(&row).await).map_err(taken_or_db_error(
        "o:email",
        messages::email_taken(&row.email),
    ))?;
    settings::store_user(api.pool(), row.id, &settings).await?;
    api.read_member("users", raw_id).await
}

/// `AbstractEntityAdapter::authorize` on a user row. The refusal names the
/// privilege as upstream's ACL spells it.
fn authorize(
    api: &Api<'_>,
    action: Action,
    privilege: &str,
    context: &Context,
) -> Result<(), ApiError> {
    if api.actor().is_allowed(action, context) {
        Ok(())
    } else {
        Err(ApiError::PermissionDenied {
            message: messages::permission_denied(privilege, USER_CLASS),
        })
    }
}

/// The settings writes `o:user_settings` asks for. An absent member writes none,
/// so a write from a form without the fieldset keeps what the user stores.
fn user_settings(
    payload: &Payload,
) -> Result<Vec<(&'static str, Option<serde_json::Value>)>, ApiError> {
    payload
        .get(settings::USER_SETTINGS_MEMBER)
        .map_or_else(|| Ok(Vec::new()), settings::parse_user)
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
    let Some(row) =
        with_db!(api.pool(), |db| UserRepo::new(db).get(UserId(id)).await).map_err(db_error)?
    else {
        return Err(not_found(USER_CLASS, raw_id));
    };
    access::user_entity(api.actor(), USER_CLASS, row.id, &row.role)?;
    let answer = crate::read::user_document(api, &row)?;
    with_db!(api.pool(), |db| UserRepo::new(db).delete(row.id).await).map_err(db_error)?;
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
            && with_db!(api.pool(), |db| UserRepo::new(db).get_by_email(email).await)
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
pub(crate) fn email_is_valid(email: &str) -> bool {
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !email.chars().any(char::is_whitespace)
}
