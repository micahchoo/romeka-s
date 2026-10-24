//! Who may do what with a bulk run.

use omeka_core::ids::UserId as CoreUserId;
use omeka_core::permissions::{Action, Adapter, Context, Role, RoleSet, allows};
use omeka_db::ids::BulkId;
use omeka_db::repos::BulkRepo;
use omeka_db::rows::BulkRow;
use omeka_db::with_db;

use super::BulkError;
use crate::auth::Actor;
use crate::errors::ApiError;
use crate::messages;
use crate::read::Api;

/// Whether `actor` holds `action` on the item adapter (`AclFactory`'s batch privileges).
pub(super) fn holds(actor: &Actor, action: Action) -> bool {
    let roles = RoleSet {
        user: actor.user().map(|user| CoreUserId::new(user.0)),
        global: actor.role().and_then(Role::from_wire_name),
        ..RoleSet::default()
    };
    allows(&roles, action, &Context::adapter(Adapter::Item))
}

/// `Api\Manager::execute`'s refusal for a batch privilege the caller lacks.
pub(super) fn denied(action: Action) -> BulkError {
    let privilege = match action {
        Action::BatchUpdateAll => "batch_update_all",
        _ => "batch_update",
    };
    BulkError::Api(ApiError::PermissionDenied {
        message: messages::permission_denied(privilege, "Omeka\\Api\\Adapter\\ItemAdapter"),
    })
}

/// The run `raw` names, refused unless the caller made it or is an administrator; only its maker
/// may start it. A run the caller may not read answers 404, as a missing one does, so an id tells
/// nothing about another person's run. The dead runs are settled after that check, so a refused
/// caller writes nothing.
pub(super) async fn load(api: &Api<'_>, raw: &str, maker_only: bool) -> Result<BulkRow, BulkError> {
    let not_found = || {
        BulkError::Api(ApiError::not_found(messages::entity_not_found(
            "Romeka\\Entity\\Bulk",
            format!("{{\"id\":{}}}", raw),
        )))
    };
    let id = raw.parse::<u64>().map(BulkId).map_err(|_| not_found())?;
    let row = with_db!(api.pool(), |db| BulkRepo::new(db).get(id).await)?.ok_or_else(not_found)?;
    let is_maker = row.owner_id.is_some() && row.owner_id == api.actor().user();
    if !is_maker && !api.actor().is_admin() {
        return Err(not_found());
    }
    if maker_only && !is_maker {
        return Err(BulkError::Api(ApiError::PermissionDenied {
            message: messages::permission_denied("run", "Romeka\\Entity\\Bulk"),
        }));
    }
    with_db!(api.pool(), |db| BulkRepo::new(db).settle_dead().await)?;
    // Settling deletes an expired preview; the row read before it still says why it cannot run.
    let settled = with_db!(api.pool(), |db| BulkRepo::new(db).get(id).await)?;
    Ok(settled.unwrap_or(row))
}
