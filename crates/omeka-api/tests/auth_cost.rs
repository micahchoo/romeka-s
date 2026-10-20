//! The API key credential cache, against a real database.
//!
//! `Authentication\Adapter\KeyAdapter::authenticate` verifies the credential
//! with `password_verify` on every request. The port caches that verdict per
//! process, so the tests here pin what must not change: a wrong credential
//! still fails, a deleted key and a deactivated owner stop working at once, and
//! a rotated hash stops the cached verdict from applying.
//!
//! The skip is proved with a counting verifier rather than a clock: two
//! requests with the same key make one call, not two.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use omeka_api::auth::{
    Actor, KEY_CREDENTIAL_PARAM, KEY_IDENTITY_PARAM, authenticate, authenticate_with,
};
use omeka_api::auth_cache::{AuthCache, CredentialVerifier};
use omeka_db::DbPool;
use omeka_db::ids::UserId;
use omeka_db::repos::{ApiKeyRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewUser};
use omeka_db::{migrate, open_memory};
use sqlx::SqlitePool;

const IDENTITY: &str = "0123456789abcdef0123456789abcdef";
const OTHER_IDENTITY: &str = "fedcba9876543210fedcba9876543210";
const CREDENTIAL: &str = "a-credential";

/// A verifier that accepts one credential and counts its calls, so a skipped
/// check is observable as a count that did not move. Clones share the count,
/// which is how the test reads the calls the cache made.
#[derive(Clone)]
struct CountingVerifier {
    expected: String,
    calls: Arc<AtomicUsize>,
}

impl CountingVerifier {
    fn new(expected: &str) -> Self {
        Self {
            expected: expected.to_string(),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl CredentialVerifier for CountingVerifier {
    fn verify(&self, _identity: &str, credential: &str, _credential_hash: &str) -> bool {
        self.calls.fetch_add(1, Ordering::SeqCst);
        credential == self.expected
    }
}

struct Fixture {
    pool: SqlitePool,
    db: DbPool,
    owner: UserId,
}

async fn fixture(role: &str, is_active: bool) -> Fixture {
    let pool = open_memory().await.expect("open");
    migrate(&pool).await.expect("migrate");
    let owner = UserRepo::new(&pool)
        .create(&NewUser {
            email: "owner@example.org".to_string(),
            name: "An Owner".to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active,
            created: None,
        })
        .await
        .expect("create user")
        .id;
    let db = DbPool::from(pool.clone());
    Fixture { pool, db, owner }
}

impl Fixture {
    async fn issue_key(&self, identity: &str, credential_hash: &str) {
        ApiKeyRepo::new(&self.pool)
            .create(&NewApiKey {
                id: identity.to_string(),
                owner_id: self.owner,
                label: "a key".to_string(),
                credential_hash: credential_hash.to_string(),
                last_ip: None,
                last_accessed: None,
                created: None,
            })
            .await
            .expect("create api key");
    }
}

fn pairs<'a>(identity: &'a str, credential: &'a str) -> [(&'a str, &'a str); 2] {
    [
        (KEY_IDENTITY_PARAM, identity),
        (KEY_CREDENTIAL_PARAM, credential),
    ]
}

#[tokio::test]
async fn a_key_that_verified_once_skips_the_next_credential_check() {
    let fixture = fixture("global_admin", true).await;
    fixture.issue_key(IDENTITY, "a-hash").await;
    let counted = CountingVerifier::new(CREDENTIAL);
    let cache = AuthCache::with_verifier(counted.clone(), 8);

    let first = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    let second = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");

    assert_eq!(first, Actor::new(fixture.owner, "global_admin"));
    assert_eq!(second, first);
    assert_eq!(
        counted.calls(),
        1,
        "two requests with one key check the credential once"
    );
    assert_eq!(cache.len(), 1);
}

#[tokio::test]
async fn a_wrong_credential_is_anonymous_twice_in_a_row() {
    let fixture = fixture("global_admin", true).await;
    fixture.issue_key(IDENTITY, "a-hash").await;
    let counted = CountingVerifier::new(CREDENTIAL);
    let cache = AuthCache::with_verifier(counted.clone(), 8);

    for _ in 0..2 {
        let actor = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, "wrong"))
            .await
            .expect("authenticate");
        assert_eq!(actor, Actor::anonymous());
    }
    assert_eq!(counted.calls(), 2, "a failed check is not remembered");
    assert!(cache.is_empty());
}

#[tokio::test]
async fn a_deleted_key_stops_working_on_the_next_request() {
    let fixture = fixture("global_admin", true).await;
    fixture.issue_key(IDENTITY, "a-hash").await;
    let counted = CountingVerifier::new(CREDENTIAL);
    let cache = AuthCache::with_verifier(counted.clone(), 8);

    let before = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(before, Actor::new(fixture.owner, "global_admin"));

    ApiKeyRepo::new(&fixture.pool)
        .delete(IDENTITY)
        .await
        .expect("delete key");

    let after = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(after, Actor::anonymous());
}

#[tokio::test]
async fn a_deactivated_owner_stops_working_on_the_next_request() {
    let fixture = fixture("global_admin", true).await;
    fixture.issue_key(IDENTITY, "a-hash").await;
    let counted = CountingVerifier::new(CREDENTIAL);
    let cache = AuthCache::with_verifier(counted.clone(), 8);

    let before = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(before, Actor::new(fixture.owner, "global_admin"));

    let users = UserRepo::new(&fixture.pool);
    let mut owner = users
        .get(fixture.owner)
        .await
        .expect("get user")
        .expect("the owner exists");
    owner.is_active = false;
    assert!(users.update(&owner).await.expect("update user"));

    let after = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(after, Actor::anonymous());
}

#[tokio::test]
async fn a_rotated_credential_hash_stops_working_on_the_next_request() {
    let fixture = fixture("global_admin", true).await;
    // The real verifier, because the fingerprint covers the stored hash and
    // bcrypt has to answer for the row.
    fixture
        .issue_key(IDENTITY, &bcrypt::hash(CREDENTIAL, 4).unwrap())
        .await;
    let cache = AuthCache::new();

    let before = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(before, Actor::new(fixture.owner, "global_admin"));

    sqlx::query("UPDATE api_key SET credential_hash = ? WHERE id = ?")
        .bind(bcrypt::hash("a different credential", 4).unwrap())
        .bind(IDENTITY)
        .execute(&fixture.pool)
        .await
        .expect("rotate the hash");

    let after = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(after, Actor::anonymous());
}

#[tokio::test]
async fn the_cache_never_holds_more_than_its_capacity() {
    let fixture = fixture("global_admin", true).await;
    // Five distinct identities, each with its own key row, into a cache of two.
    let identities = [
        IDENTITY,
        OTHER_IDENTITY,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccccc",
    ];
    for identity in identities {
        fixture.issue_key(identity, "a-hash").await;
    }
    let counted = CountingVerifier::new(CREDENTIAL);
    let cache = AuthCache::with_verifier(counted.clone(), 2);

    for identity in identities {
        let actor = authenticate_with(&fixture.db, &cache, pairs(identity, CREDENTIAL))
            .await
            .expect("authenticate");
        assert_eq!(actor, Actor::new(fixture.owner, "global_admin"));
    }

    assert_eq!(cache.capacity(), 2);
    assert_eq!(cache.len(), 2, "the bound holds as identities arrive");
    assert_eq!(counted.calls(), 5, "each new identity checks once");

    // The first identity was evicted, so its next request checks again.
    authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    assert_eq!(counted.calls(), 6);
}

#[tokio::test]
async fn a_cached_key_authenticates_like_the_uncached_path() {
    let fixture = fixture("editor", true).await;
    fixture
        .issue_key(IDENTITY, &bcrypt::hash(CREDENTIAL, 4).unwrap())
        .await;
    let cache = AuthCache::new();

    let uncached = authenticate(&fixture.db, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    let first = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");
    let second = authenticate_with(&fixture.db, &cache, pairs(IDENTITY, CREDENTIAL))
        .await
        .expect("authenticate");

    assert_eq!(uncached, Actor::new(fixture.owner, "editor"));
    assert_eq!(first, uncached);
    assert_eq!(second, uncached);
}
