//! `MediaAnnotationRepo` and `MediaExtentRepo`: the Web Annotation rows of one media, the caption
//! rows among them, the text index, and the extent of an audio or video. Every test body runs on
//! SQLite and, with the `mysql` feature and `ROMEKA_MYSQL_URL`, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::on_both;
use omeka_db::repos::{
    ChangeOutcome, Cue, ExtentRow, FtsQuery, InsertOutcome, ItemRepo, MediaAnnotationRepo,
    MediaAnnotationWrite, MediaExtentRepo, MediaRepo, SearchScope, TrackOutcome,
};
use omeka_db::rows::{NewItem, NewMedia, NewResource};

/// A JSON body with `value` as its text.
fn textual(value: &str) -> String {
    format!("{{\"type\":\"TextualBody\",\"value\":\"{value}\",\"format\":\"text/plain\"}}")
}

/// A comment on the whole media.
fn whole<'a>(body_json: &'a str, text: &'a str) -> MediaAnnotationWrite<'a> {
    MediaAnnotationWrite {
        motivation: "commenting",
        selector_kind: "whole",
        selector: None,
        time_ms: None,
        body_json,
        body_text: text,
        lang: None,
        extensions: None,
    }
}

/// A caption cue in `en`.
fn cue<'a>(start: i64, end: i64, body_json: &'a str, text: &'a str) -> Cue<'a> {
    Cue {
        start_ms: start,
        end_ms: end,
        body_json,
        body_text: text,
    }
}

fn anonymous() -> SearchScope {
    SearchScope {
        user: None,
        privileged: false,
        hide_notes: false,
    }
}

/// An item and one media in it. Expands inside an [`on_both!`] body, where `Db` names the engine.
macro_rules! item_and_media {
    ($pool:expr, $item_public:expr, $media_public:expr) => {{
        let item = ItemRepo::<Db>::new($pool)
            .create(&NewItem {
                resource: NewResource {
                    is_public: $item_public,
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        let media = MediaRepo::<Db>::new($pool)
            .create(&NewMedia {
                resource: NewResource {
                    is_public: $media_public,
                    ..NewResource::default()
                },
                item_id: item.id(),
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                ..NewMedia::default()
            })
            .await
            .unwrap();
        (item.id(), media.id())
    }};
}

/// Another media in an item that already exists.
macro_rules! another_media {
    ($pool:expr, $item:expr, $public:expr) => {{
        MediaRepo::<Db>::new($pool)
            .create(&NewMedia {
                resource: NewResource {
                    is_public: $public,
                    ..NewResource::default()
                },
                item_id: $item,
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                ..NewMedia::default()
            })
            .await
            .unwrap()
            .id()
    }};
}

/// A comment on the whole media, inserted.
macro_rules! a_note {
    ($pool:expr, $media:expr, $text:expr) => {{
        let body = textual($text);
        let write = whole(&body, $text);
        match MediaAnnotationRepo::<Db>::new($pool)
            .insert($media, None, None, &write)
            .await
            .unwrap()
        {
            InsertOutcome::Inserted(row) => row,
            InsertOutcome::KeyTaken(row) => row,
        }
    }};
}

on_both!(a_page_is_keyset_in_id_order, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    for text in ["one", "two", "three", "four", "five"] {
        a_note!(pool, media, text);
    }

    let first = repo.page(media, 0, 3, None).await.unwrap();
    assert_eq!(first.len(), 4, "a page asks for limit + 1");
    assert_eq!(first[0].body_json, textual("one"));
    assert_eq!(first[3].body_json, textual("four"));

    let second = repo.page(media, first[2].id, 3, None).await.unwrap();
    assert_eq!(second.len(), 2);
    assert_eq!(second[0].body_json, textual("four"));
    assert_eq!(second[1].body_json, textual("five"));

    let end = repo.page(media, second[1].id, 3, None).await.unwrap();
    assert!(end.is_empty());
});

on_both!(a_stale_replace_answers_the_current_row, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let row = a_note!(pool, media, "first");

    let body = textual("second");
    let replaced = repo
        .replace(row.id, row.revision, &whole(&body, "second"))
        .await
        .unwrap();
    assert_eq!(replaced, ChangeOutcome::Changed { revision: 2 });

    let stale = repo
        .replace(row.id, 1, &whole(&body, "second"))
        .await
        .unwrap();
    match stale {
        ChangeOutcome::Stale { current } => {
            assert_eq!(current.revision, 2);
            assert_eq!(current.body_json, textual("second"));
        }
        other => panic!("expected Stale, got {other:?}"),
    }
});

on_both!(a_delete_names_the_revision_it_read, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let row = a_note!(pool, media, "gone soon");
    assert_eq!(
        repo.delete(row.id, 2).await.unwrap(),
        ChangeOutcome::Stale {
            current: row.clone()
        }
    );
    assert_eq!(
        repo.delete(row.id, row.revision).await.unwrap(),
        ChangeOutcome::Changed { revision: 2 }
    );
    assert_eq!(
        repo.delete(row.id, 1).await.unwrap(),
        ChangeOutcome::Missing
    );
});

on_both!(a_taken_key_answers_the_row_it_names, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let body = textual("retried");
    let write = whole(&body, "retried");

    let first = repo
        .insert(media, None, Some("6f1c"), &write)
        .await
        .unwrap();
    let InsertOutcome::Inserted(first) = first else {
        panic!("the first create inserted no row");
    };

    let replay = repo
        .insert(media, None, Some("6f1c"), &write)
        .await
        .unwrap();
    match replay {
        InsertOutcome::KeyTaken(taken) => assert_eq!(taken.id, first.id),
        other => panic!("expected KeyTaken, got {other:?}"),
    }
    assert_eq!(repo.total(media, None).await.unwrap(), 1);
});

on_both!(two_notes_without_a_key_coexist, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, media, "one");
    a_note!(pool, media, "two");
    assert_eq!(repo.total(media, None).await.unwrap(), 2);
});

on_both!(a_caption_without_a_time_is_refused, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let body = textual("no cue");
    let write = MediaAnnotationWrite {
        motivation: "supplementing",
        selector_kind: "whole",
        selector: None,
        time_ms: None,
        body_json: &body,
        body_text: "no cue",
        lang: Some("en"),
        extensions: None,
    };
    assert!(repo.insert(media, None, None, &write).await.is_err());
    assert_eq!(repo.total(media, None).await.unwrap(), 0);
});

on_both!(a_track_row_must_be_a_caption, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let inserted = sqlx::query::<Db>(
        "INSERT INTO romeka_media_annotation
             (media_id, revision, created, modified, motivation, origin, lang, selector_kind,
              selector, t_start_ms, t_end_ms, body_json, body_text)
         VALUES (?, 1, '2026-10-20 00:00:00', '2026-10-20 00:00:00', 'commenting', 'track',
                 'en', 'time', NULL, 0, 1000, '{}', 'not a caption')",
    )
    .bind(i64::try_from(media.0).unwrap())
    .execute(pool)
    .await;
    assert!(inserted.is_err(), "a track row that is not supplementing");
});

on_both!(equal_cues_are_unchanged, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let one = textual("First cue");
    let two = textual("Second cue");
    let cues = [
        cue(0, 1000, &one, "First cue"),
        cue(1000, 2500, &two, "Second cue"),
    ];

    let imported = repo.replace_track(media, Some("en"), &cues).await.unwrap();
    assert_eq!(
        imported,
        TrackOutcome::Replaced {
            removed: 0,
            inserted: 2
        }
    );
    let before = repo.cues(media, Some("en")).await.unwrap();

    let again = repo.replace_track(media, Some("en"), &cues).await.unwrap();
    assert_eq!(again, TrackOutcome::Unchanged { cues: 2 });
    let after = repo.cues(media, Some("en")).await.unwrap();
    assert_eq!(after, before, "equal cues bump no revision");
});

on_both!(a_caption_import_keeps_hand_written_notes, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, media, "a curator's comment");
    let handwritten = textual("a hand-written caption");
    let write = MediaAnnotationWrite {
        motivation: "supplementing",
        selector_kind: "time",
        selector: None,
        time_ms: Some((0, 500)),
        body_json: &handwritten,
        body_text: "a hand-written caption",
        lang: Some("en"),
        extensions: None,
    };
    repo.insert(media, None, None, &write).await.unwrap();

    let one = textual("First cue");
    let replaced = repo
        .replace_track(media, Some("en"), &[cue(0, 1000, &one, "First cue")])
        .await
        .unwrap();
    assert_eq!(
        replaced,
        TrackOutcome::Replaced {
            removed: 0,
            inserted: 1
        }
    );
    let cues = repo.cues(media, Some("en")).await.unwrap();
    assert_eq!(cues.len(), 2, "the hand-written cue is still there");
    assert_eq!(repo.total(media, None).await.unwrap(), 3);
});

on_both!(track_langs_counts_each_language, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let text = textual("Cue");
    repo.replace_track(media, Some("en"), &[cue(0, 10, &text, "Cue")])
        .await
        .unwrap();
    repo.replace_track(
        media,
        Some("en"),
        &[cue(0, 10, &text, "Cue"), cue(10, 20, &text, "Cue")],
    )
    .await
    .unwrap();

    let langs = repo.track_langs(&[media]).await.unwrap();
    assert_eq!(langs, vec![(media, Some("en".to_string()), 2)]);
});

on_both!(firsts_and_counts_group_by_media, |pool| {
    let (_, first) = item_and_media!(pool, true, true);
    let (_, second) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    let whole_body = textual("whole");
    let early = textual("early");
    let late = textual("late");
    repo.insert(
        first,
        None,
        None,
        &MediaAnnotationWrite {
            motivation: "describing",
            selector_kind: "time",
            selector: None,
            time_ms: Some((1000, 2000)),
            body_json: &late,
            body_text: "late",
            lang: None,
            extensions: None,
        },
    )
    .await
    .unwrap();
    a_note!(pool, first, "whole");
    repo.insert(
        first,
        None,
        None,
        &MediaAnnotationWrite {
            motivation: "describing",
            selector_kind: "time",
            selector: None,
            time_ms: Some((10, 20)),
            body_json: &early,
            body_text: "early",
            lang: None,
            extensions: None,
        },
    )
    .await
    .unwrap();
    a_note!(pool, second, "other media");

    let rows = repo.firsts(&[first, second], 8, None).await.unwrap();
    let of_first: Vec<&str> = rows
        .iter()
        .filter(|row| row.media_id == first)
        .map(|row| row.body_json.as_str())
        .collect();
    assert_eq!(of_first, vec![&*early, &*late, &*whole_body]);

    let counts = repo.counts(&[first, second], None).await.unwrap();
    assert_eq!(counts.get(&first), Some(&3));
    assert_eq!(counts.get(&second), Some(&1));

    let capped = repo.firsts(&[first], 1, None).await.unwrap();
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].body_json, early);
});

on_both!(deleting_a_media_removes_its_notes, |pool| {
    let (item, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, media, "archaeology");
    let query = FtsQuery::from_words("archaeology").unwrap();
    let found = repo
        .search(&anonymous(), item, &query, 0, 10)
        .await
        .unwrap();
    assert_eq!(found.len(), 1);

    assert!(MediaRepo::<Db>::new(pool).delete(media).await.unwrap());
    assert_eq!(repo.total(media, None).await.unwrap(), 0);
    let found = repo
        .search(&anonymous(), item, &query, 0, 10)
        .await
        .unwrap();
    assert!(found.is_empty());
});

on_both!(search_hides_a_private_media_from_anonymous, |pool| {
    let (item, public) = item_and_media!(pool, true, true);
    let private = another_media!(pool, item, false);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, public, "shield wall");
    a_note!(pool, private, "shield burial");
    let query = FtsQuery::from_words("shield").unwrap();

    let visible = repo
        .search(&anonymous(), item, &query, 0, 10)
        .await
        .unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].media_id, public);

    let privileged = SearchScope {
        privileged: true,
        ..anonymous()
    };
    let all = repo.search(&privileged, item, &query, 0, 10).await.unwrap();
    assert_eq!(
        all.len(),
        2,
        "the note is there, the anonymous scope hides it"
    );
});

on_both!(a_page_filters_by_motivation, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, media, "a comment");
    let caption = textual("a caption");
    repo.replace_track(media, Some("en"), &[cue(0, 1000, &caption, "a caption")])
        .await
        .unwrap();

    assert_eq!(repo.total(media, None).await.unwrap(), 2);
    assert_eq!(
        repo.total(media, Some(&["supplementing"])).await.unwrap(),
        1
    );
    let captions = repo
        .page(media, 0, 10, Some(&["supplementing"]))
        .await
        .unwrap();
    assert_eq!(captions.len(), 1);
    assert_eq!(captions[0].motivation, "supplementing");
});

on_both!(a_private_item_hides_its_public_media, |pool| {
    let (item, media) = item_and_media!(pool, false, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, media, "helmet");
    let query = FtsQuery::from_words("helmet").unwrap();
    let found = repo
        .search(&anonymous(), item, &query, 0, 10)
        .await
        .unwrap();
    assert!(found.is_empty(), "the item is private");
});

on_both!(the_switch_leaves_only_captions_in_a_search, |pool| {
    let (item, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    a_note!(pool, media, "a shield comment");
    let caption = textual("a shield caption");
    repo.replace_track(
        media,
        Some("en"),
        &[cue(0, 1000, &caption, "a shield caption")],
    )
    .await
    .unwrap();
    let query = FtsQuery::from_words("shield").unwrap();
    let hidden = SearchScope {
        hide_notes: true,
        ..anonymous()
    };
    let found = repo.search(&hidden, item, &query, 0, 10).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].motivation, "supplementing");
});

on_both!(search_treats_operators_as_words, |pool| {
    let (item, media) = item_and_media!(pool, true, true);
    a_note!(pool, media, "shield wall");
    macro_rules! found {
        ($words:expr) => {{
            match FtsQuery::from_words($words) {
                Some(query) => MediaAnnotationRepo::<Db>::new(pool)
                    .search(&anonymous(), item, &query, 0, 10)
                    .await
                    .unwrap(),
                None => Vec::new(),
            }
        }};
    }
    assert_eq!(found!("shield*").len(), 1, "the star does not prefix-match");
    assert!(found!("shie*").is_empty());
    assert_eq!(found!("NEAR shield").len(), 1, "NEAR is a word, not syntax");
    assert!(found!("\"").is_empty());
});

on_both!(search_pages_by_annotation_id, |pool| {
    let (item, media) = item_and_media!(pool, true, true);
    let repo = MediaAnnotationRepo::<Db>::new(pool);
    for text in ["shield one", "shield two", "shield three"] {
        a_note!(pool, media, text);
    }
    let query = FtsQuery::from_words("shield").unwrap();
    let first = repo.search(&anonymous(), item, &query, 0, 2).await.unwrap();
    assert_eq!(first.len(), 3);
    let second = repo
        .search(&anonymous(), item, &query, first[1].id, 2)
        .await
        .unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].body_json, textual("shield three"));
});

on_both!(an_extent_is_keyed_by_the_original_it_probed, |pool| {
    let (_, media) = item_and_media!(pool, true, true);
    let repo = MediaExtentRepo::<Db>::new(pool);
    assert_eq!(repo.get(media, "original/a.m4a").await.unwrap(), None);

    let extent = ExtentRow {
        duration_ms: Some(62_000),
        width: None,
        height: None,
    };
    repo.put(media, "original/a.m4a", &extent).await.unwrap();
    assert_eq!(
        repo.get(media, "original/a.m4a").await.unwrap(),
        Some(extent)
    );
    assert_eq!(repo.get(media, "original/b.m4a").await.unwrap(), None);

    let moved = ExtentRow {
        duration_ms: Some(9_000),
        width: Some(1920),
        height: Some(1080),
    };
    repo.put(media, "original/b.m4a", &moved).await.unwrap();
    assert_eq!(repo.get(media, "original/a.m4a").await.unwrap(), None);
    assert_eq!(
        repo.get(media, "original/b.m4a").await.unwrap(),
        Some(moved)
    );
});

#[tokio::test]
async fn the_sqlite_index_follows_an_update_and_a_delete() {
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource {
                is_public: true,
                ..NewResource::default()
            },
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    let repo = MediaAnnotationRepo::new(&pool);
    let body = textual("archaeology");
    let row = repo
        .insert(media.id(), None, None, &whole(&body, "archaeology"))
        .await
        .unwrap();
    let InsertOutcome::Inserted(row) = row else {
        panic!("no row inserted");
    };
    assert_eq!(hits(&pool, "archaeology").await, vec![row.id]);

    let body = textual("conservation");
    repo.replace(row.id, 1, &whole(&body, "conservation"))
        .await
        .unwrap();
    assert!(hits(&pool, "archaeology").await.is_empty());
    assert_eq!(hits(&pool, "conservation").await, vec![row.id]);

    MediaRepo::new(&pool).delete(media.id()).await.unwrap();
    assert!(
        hits(&pool, "conservation").await.is_empty(),
        "the cascade delete fires the index trigger"
    );
}

/// The ids the FTS5 mirror matches.
async fn hits(pool: &sqlx::SqlitePool, needle: &str) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT romeka_media_annotation.id FROM romeka_media_annotation_fts
         JOIN romeka_media_annotation ON romeka_media_annotation.rowid = romeka_media_annotation_fts.rowid
         WHERE romeka_media_annotation_fts MATCH ? ORDER BY romeka_media_annotation.id",
    )
    .bind(needle)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[cfg(feature = "mysql")]
mod mysql {
    use super::*;
    use sqlx::MySql;

    #[tokio::test]
    async fn mysql_migration_creates_both_tables() {
        let Some(db) = common::mysql_unmigrated().await else {
            return;
        };
        omeka_db::migrate(&db.pool).await.unwrap();
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT CAST(table_name AS CHAR) FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name LIKE 'romeka_media%' ORDER BY table_name",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(tables, ["romeka_media_annotation", "romeka_media_extent"]);

        let checks: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.table_constraints
             WHERE table_schema = DATABASE() AND table_name = 'romeka_media_annotation'
               AND constraint_type = 'CHECK'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(checks >= 6, "the CHECKs reached the server");
        db.cleanup().await;
    }

    #[tokio::test]
    async fn mysql_matches_a_two_letter_word_through_like() {
        let Some(db) = common::mysql().await else {
            return;
        };
        let pool = &db.pool;
        let item = ItemRepo::<MySql>::new(pool)
            .create(&NewItem {
                resource: NewResource {
                    is_public: true,
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        let media = MediaRepo::<MySql>::new(pool)
            .create(&NewMedia {
                resource: NewResource {
                    is_public: true,
                    ..NewResource::default()
                },
                item_id: item.id(),
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                ..NewMedia::default()
            })
            .await
            .unwrap();
        let repo = MediaAnnotationRepo::<MySql>::new(pool);
        let ox = textual("The ox cart");
        repo.insert(media.id(), None, None, &whole(&ox, "The ox cart"))
            .await
            .unwrap();
        let other = textual("A cart of hay");
        repo.insert(media.id(), None, None, &whole(&other, "A cart of hay"))
            .await
            .unwrap();

        // `innodb_ft_min_token_size` is three, so FULLTEXT holds no `ox`; the LIKE half finds it.
        let query = FtsQuery::from_words("ox").unwrap();
        let found = repo
            .search(&anonymous(), item.id(), &query, 0, 10)
            .await
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].body_json, ox);
        db.cleanup().await;
    }
}
