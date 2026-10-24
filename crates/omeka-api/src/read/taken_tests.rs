//! A unique violation that slips past a write's own check answers the 422 that check gives.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_db::repos::UserRepo;
use omeka_db::rows::NewUser;

use super::taken_or_db_error;
use crate::errors::ApiError;

fn ada() -> NewUser {
    NewUser {
        email: "ada@example.org".to_string(),
        name: "Ada".to_string(),
        password_hash: None,
        role: "editor".to_string(),
        is_active: true,
        created: None,
    }
}

#[tokio::test]
async fn a_unique_violation_is_the_field_error_and_any_other_failure_a_500() {
    let pool = omeka_db::open_memory().await.unwrap();
    omeka_db::migrate(&pool).await.unwrap();
    let users = UserRepo::new(&pool);
    users.create(&ada()).await.unwrap();

    let second = users.create(&ada()).await.unwrap_err();
    match taken_or_db_error("o:email", "taken")(second) {
        ApiError::Validation(errors) => {
            assert_eq!(errors.get("o:email"), Some(&vec!["taken".to_string()]));
        }
        other => panic!("a 422, not {other:?}"),
    }

    let other = omeka_db::DbError::Conflict("not a unique violation");
    assert!(matches!(
        taken_or_db_error("o:email", "taken")(other),
        ApiError::Internal { .. }
    ));
}
