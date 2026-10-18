//! `session`: the save handler's write, read, delete and garbage collection.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::repos::SessionRepo;

#[tokio::test]
async fn a_session_round_trips_and_replaces_its_data() {
    let pool = common::memory().await;
    let sessions = SessionRepo::new(&pool);

    assert_eq!(sessions.get("sess-1").await.unwrap(), None);
    sessions
        .save("sess-1", b"user=1", Some(1000))
        .await
        .unwrap();
    let stored = sessions.get("sess-1").await.unwrap().unwrap();
    assert_eq!(stored.id, "sess-1");
    assert_eq!(stored.data, b"user=1");
    assert_eq!(stored.modified, 1000);

    sessions
        .save("sess-1", b"user=2", Some(1001))
        .await
        .unwrap();
    let replaced = sessions.get("sess-1").await.unwrap().unwrap();
    assert_eq!(replaced.data, b"user=2", "a repeat id replaces the data");
    assert_eq!(replaced.modified, 1001, "and the time");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1,
        "one row stays for one id"
    );

    assert!(sessions.delete("sess-1").await.unwrap());
    assert_eq!(sessions.get("sess-1").await.unwrap(), None);
    assert!(!sessions.delete("sess-1").await.unwrap());
}

#[tokio::test]
async fn garbage_collection_removes_sessions_older_than_the_cutoff() {
    let pool = common::memory().await;
    let sessions = SessionRepo::new(&pool);
    sessions.save("old", b"a", Some(100)).await.unwrap();
    sessions.save("middle", b"b", Some(300)).await.unwrap();
    sessions.save("new", b"c", Some(500)).await.unwrap();

    assert_eq!(
        sessions.gc(400).await.unwrap(),
        2,
        "two rows passed the cutoff"
    );
    assert_eq!(sessions.get("old").await.unwrap(), None);
    assert_eq!(sessions.get("middle").await.unwrap(), None);
    assert!(sessions.get("new").await.unwrap().is_some());

    assert_eq!(
        sessions.gc(400).await.unwrap(),
        0,
        "nothing left to collect"
    );
}
