//! The access rule and the switch, through the service the WAP routes call.
//!
//! Reading a media's notes is reading the media; writing them is updating it.
//! These tests pin both halves, the author/editor split, the switch that keeps
//! captions, the idempotency key and the conditional replace and delete.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use omeka_api::annotation::{
    AnnotationError, Annotations, Created, ExtentSource, IdempotencyKey, IfMatch, Iris, Rights,
    render, search_scope,
};
use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::Api;
use omeka_api::settings::{ANNOTATIONS_PUBLIC, annotations_public};
use omeka_db::DbPool;
use omeka_db::ids::{ItemId, MediaId, UserId};
use omeka_db::repos::{
    Cue, ItemRepo, MediaAnnotationRepo, MediaAnnotationWrite, MediaRepo, SettingRepo, UserRepo,
};
use omeka_db::rows::{MediaRow, NewItem, NewMedia, NewResource, NewUser};
use omeka_db::{migrate, open_memory};
use omeka_media::kind::Extent;
use serde_json::json;
use sqlx::SqlitePool;

const BASE: &str = "https://archive.example";

struct Seed {
    db: DbPool,
    pool: SqlitePool,
    owner: UserId,
    /// Another author, who may read the public media but not write them.
    other: UserId,
    editor: UserId,
    item: ItemId,
    image: MediaId,
    video: MediaId,
    pdf: MediaId,
    private: MediaId,
    /// A public media owned by [`Seed::other`], with the same item.
    theirs: MediaId,
}

struct FakeExtents {
    extents: HashMap<MediaId, Extent>,
}

impl FakeExtents {
    fn of(seed: &Seed) -> Self {
        let mut extents = HashMap::new();
        extents.insert(
            seed.image,
            Extent {
                size: Some((4000, 3000)),
                duration_ms: None,
            },
        );
        extents.insert(
            seed.video,
            Extent {
                size: Some((1920, 1080)),
                duration_ms: Some(90_000),
            },
        );
        extents.insert(
            seed.private,
            Extent {
                size: Some((100, 100)),
                duration_ms: None,
            },
        );
        extents.insert(
            seed.theirs,
            Extent {
                size: Some((800, 600)),
                duration_ms: None,
            },
        );
        Self { extents }
    }
}

impl ExtentSource for FakeExtents {
    fn extent<'a>(
        &'a self,
        media: &'a MediaRow,
    ) -> Pin<Box<dyn Future<Output = Extent> + Send + 'a>> {
        let extent = self.extents.get(&media.id()).copied().unwrap_or_default();
        Box::pin(async move { extent })
    }
}

async fn user(pool: &SqlitePool, email: &str, role: &str) -> UserId {
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: email.to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("user row")
        .id
}

async fn seed() -> Seed {
    let pool = open_memory().await.expect("in-memory database");
    migrate(&pool).await.expect("migrations");
    let owner = user(&pool, "owner@example.com", "author").await;
    let other = user(&pool, "other@example.com", "author").await;
    let editor = user(&pool, "editor@example.com", "editor").await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some("Folio 12r".to_string()),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .expect("item row")
        .id();
    let image = media(&pool, item, owner, "page.jpg", "image/jpeg", true, true).await;
    let video = media(&pool, item, owner, "talk.mp4", "video/mp4", false, true).await;
    let pdf = media(
        &pool,
        item,
        owner,
        "talk.pdf",
        "application/pdf",
        false,
        true,
    )
    .await;
    let private = media(&pool, item, owner, "draft.jpg", "image/jpeg", true, false).await;
    let theirs = media(&pool, item, other, "theirs.jpg", "image/jpeg", true, true).await;
    Seed {
        db: DbPool::from(pool.clone()),
        pool,
        owner,
        other,
        editor,
        item,
        image,
        video,
        pdf,
        private,
        theirs,
    }
}

async fn media(
    pool: &SqlitePool,
    item: ItemId,
    owner: UserId,
    name: &str,
    media_type: &str,
    thumbnails: bool,
    is_public: bool,
) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(name.to_string()),
                is_public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some(format!("storage-{name}")),
            extension: Some(name.rsplit('.').next().unwrap_or("bin").to_string()),
            media_type: Some(media_type.to_string()),
            has_original: true,
            has_thumbnails: thumbnails,
            ..NewMedia::default()
        })
        .await
        .expect("media row")
        .id()
}

async fn api<'a>(seed: &'a Seed, actor: Actor) -> Api<'a> {
    Api::load(&seed.db, BaseUrl::parse(BASE).expect("base URL"), actor)
        .await
        .expect("read surface")
}

async fn canvas(notes: &Annotations<'_>, iris: &Iris, media: MediaId) -> String {
    let (media, _) = notes.rights(media).await.expect("rights");
    iris.canvas(&media)
}

fn whole(canvas: &str, text: &str, motivation: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": motivation,
        "body": { "type": "TextualBody", "value": text, "format": "text/plain" },
        "target": canvas,
    }))
    .expect("a JSON document")
}

fn cue(canvas: &str, text: &str, start: &str, end: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "supplementing",
        "body": {
            "type": "TextualBody",
            "value": text,
            "format": "text/plain",
            "language": "en",
        },
        "target": {
            "type": "SpecificResource",
            "source": { "id": canvas, "type": "Canvas" },
            "selector": {
                "type": "FragmentSelector",
                "conformsTo": "http://www.w3.org/TR/media-frags/",
                "value": format!("t={start},{end}"),
            },
        },
    }))
    .expect("a JSON document")
}

async fn create(
    notes: &Annotations<'_>,
    media: MediaId,
    document: &[u8],
    iris: &Iris,
    key: Option<&IdempotencyKey>,
) -> omeka_api::annotation::Annotation {
    match notes
        .create(media, document, iris, key)
        .await
        .expect("created")
    {
        Created::New(annotation) | Created::Replayed(annotation) => annotation,
    }
}

fn strong(tag: &str) -> IfMatch {
    IfMatch::parse(tag).expect("a strong tag")
}

async fn set_switch(seed: &Seed, on: bool) {
    SettingRepo::new(&seed.pool)
        .set(ANNOTATIONS_PUBLIC, &json!(on))
        .await
        .expect("the switch row");
}

#[tokio::test]
async fn an_anonymous_caller_gets_not_found_on_a_private_media() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let editor_notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let hidden = canvas(&editor_notes, &iris, seed.private).await;
    let note = create(
        &editor_notes,
        seed.private,
        &whole(&hidden, "Not for visitors", "commenting"),
        &iris,
        None,
    )
    .await;

    let anonymous_api = api(&seed, Actor::anonymous()).await;
    let anonymous = Annotations::new(&anonymous_api, &extents);
    assert!(matches!(
        anonymous.rights(seed.private).await,
        Err(AnnotationError::NotFound)
    ));
    assert!(matches!(
        anonymous.page(seed.private, None).await,
        Err(AnnotationError::NotFound)
    ));
    assert!(matches!(
        anonymous.get(note.id).await,
        Err(AnnotationError::NotFound)
    ));
}

#[tokio::test]
async fn an_author_reads_another_users_media_but_may_not_write_it() {
    let seed = seed().await;
    let other_api = api(&seed, Actor::new(seed.other, "author")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&other_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;

    let (media, rights) = notes.rights(seed.image).await.expect("readable");
    assert_eq!(rights, Rights::Read);
    assert_eq!(media.item.0, 1);

    assert!(matches!(
        notes
            .create(
                seed.image,
                &whole(&target, "Not mine", "commenting"),
                &iris,
                None
            )
            .await,
        Err(AnnotationError::Forbidden)
    ));
}

#[tokio::test]
async fn an_author_writes_a_media_they_own() {
    let seed = seed().await;
    let other_api = api(&seed, Actor::new(seed.other, "author")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&other_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.theirs).await;

    let (_, rights) = notes.rights(seed.theirs).await.expect("readable");
    assert_eq!(rights, Rights::ReadWrite);
    let annotation = create(
        &notes,
        seed.theirs,
        &whole(&target, "Mine", "commenting"),
        &iris,
        None,
    )
    .await;
    assert_eq!(annotation.revision.etag(), "\"r1\"");
}

#[tokio::test]
async fn an_editor_writes_and_the_etag_moves_on_every_change() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;

    let (_, rights) = notes.rights(seed.image).await.expect("readable");
    assert_eq!(rights, Rights::ReadWrite);
    let first = create(
        &notes,
        seed.image,
        &whole(&target, "Watermark", "commenting"),
        &iris,
        None,
    )
    .await;
    assert_eq!(first.revision.etag(), "\"r1\"");
    assert_eq!(first.owner, Some(seed.editor));

    let second = notes
        .replace(
            first.id,
            &strong(&first.revision.etag()),
            &whole(&target, "Watermark, crowned shield", "commenting"),
            &iris,
        )
        .await
        .expect("replaced");
    assert_eq!(second.revision.etag(), "\"r2\"");
    assert_eq!(
        second.draft.bodies,
        notes.get(first.id).await.expect("read").draft.bodies
    );

    let page = notes.page(seed.image, None).await.expect("page");
    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.rights, Rights::ReadWrite);
    assert_eq!(page.next, None);
}

#[tokio::test]
async fn a_replayed_key_returns_the_same_note_and_a_changed_body_is_refused() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let key = IdempotencyKey::parse("2c4e9a0b-migrate-0001").expect("a key");
    let document = whole(&target, "The deed", "commenting");

    let first = match notes
        .create(seed.pdf, &document, &iris, Some(&key))
        .await
        .expect("created")
    {
        Created::New(annotation) => annotation,
        Created::Replayed(_) => panic!("the first create is new"),
    };
    let replay = match notes
        .create(seed.pdf, &document, &iris, Some(&key))
        .await
        .expect("replayed")
    {
        Created::Replayed(annotation) => annotation,
        Created::New(_) => panic!("the second create replays"),
    };
    assert_eq!(replay.id, first.id);
    assert_eq!(replay.revision, first.revision);

    assert!(matches!(
        notes
            .create(
                seed.pdf,
                &whole(&target, "Another deed", "commenting"),
                &iris,
                Some(&key)
            )
            .await,
        Err(AnnotationError::KeyReused)
    ));
}

#[tokio::test]
async fn a_weak_or_wrong_tag_never_matches() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let annotation = create(
        &notes,
        seed.pdf,
        &whole(&target, "First", "commenting"),
        &iris,
        None,
    )
    .await;
    let changed = whole(&target, "Second", "commenting");

    let weak = IfMatch::parse("W/\"r1\"").expect("a weak tag header");
    let error = notes
        .replace(annotation.id, &weak, &changed, &iris)
        .await
        .expect_err("a weak tag never matches");
    match error {
        AnnotationError::Stale(current) => {
            assert_eq!(current.revision.etag(), "\"r1\"");
            assert_eq!(
                render(&current, &iris)["body"]["value"],
                "First",
                "the conflict carries the version to show"
            );
        }
        other => panic!("expected Stale, got {other}"),
    }

    assert!(matches!(
        notes
            .replace(annotation.id, &strong("\"r9\""), &changed, &iris)
            .await,
        Err(AnnotationError::Stale(_))
    ));
    assert!(matches!(
        notes.delete(annotation.id, &strong("\"r9\"")).await,
        Err(AnnotationError::Stale(_))
    ));
}

#[tokio::test]
async fn an_equal_replace_keeps_the_revision() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let document = whole(&target, "The deed", "commenting");
    let annotation = create(&notes, seed.pdf, &document, &iris, None).await;

    let same = notes
        .replace(
            annotation.id,
            &strong(&annotation.revision.etag()),
            &document,
            &iris,
        )
        .await
        .expect("an equal replace");
    assert_eq!(same.revision.etag(), "\"r1\"");
}

#[tokio::test]
async fn delete_is_conditional_then_the_note_is_gone() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.pdf).await;
    let annotation = create(
        &notes,
        seed.pdf,
        &whole(&target, "To remove", "commenting"),
        &iris,
        None,
    )
    .await;

    notes
        .delete(annotation.id, &strong(&annotation.revision.etag()))
        .await
        .expect("deleted");
    assert!(matches!(
        notes.get(annotation.id).await,
        Err(AnnotationError::NotFound)
    ));
    assert!(matches!(
        notes.delete(annotation.id, &strong("\"r1\"")).await,
        Err(AnnotationError::NotFound)
    ));
}

#[tokio::test]
async fn the_switch_hides_notes_but_keeps_captions_for_a_reader() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.video).await;

    let comment = create(
        &notes,
        seed.video,
        &whole(&target, "A comment", "commenting"),
        &iris,
        None,
    )
    .await;
    let caption = create(
        &notes,
        seed.video,
        &cue(&target, "The speaker names the place", "1", "2.5"),
        &iris,
        None,
    )
    .await;
    set_switch(&seed, false).await;
    assert!(!annotations_public(&seed.db).await.expect("the switch"));

    let anonymous_api = api(&seed, Actor::anonymous()).await;
    let anonymous = Annotations::new(&anonymous_api, &extents);
    let (_, rights) = anonymous.rights(seed.video).await.expect("readable");
    assert_eq!(rights, Rights::ReadCaptionsOnly);

    let page = anonymous.page(seed.video, None).await.expect("page");
    assert_eq!(page.total, 1, "only the caption is visible");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, caption.id);
    assert!(matches!(
        anonymous.get(comment.id).await,
        Err(AnnotationError::NotFound)
    ));
    assert!(anonymous.get(caption.id).await.is_ok());

    let editor_page = notes.page(seed.video, None).await.expect("editor page");
    assert_eq!(editor_page.total, 2, "an editor still sees everything");

    set_switch(&seed, true).await;
    let page = anonymous.page(seed.video, None).await.expect("page");
    assert_eq!(page.total, 2, "the switch's absent state is on");
}

#[tokio::test]
async fn summaries_and_counts_leave_out_a_media_the_caller_cannot_read() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.image).await;
    create(
        &notes,
        seed.image,
        &whole(&target, "A note", "commenting"),
        &iris,
        None,
    )
    .await;

    let anonymous_api = api(&seed, Actor::anonymous()).await;
    let anonymous = Annotations::new(&anonymous_api, &extents);
    let media = [seed.image, seed.video, seed.private];
    let summaries = anonymous.summaries(&media, 8).await.expect("summaries");
    assert_eq!(summaries.len(), 2, "the private media is absent");
    let image = summaries.get(&seed.image).expect("the image");
    assert_eq!(image.total, 1);
    assert_eq!(image.first.len(), 1);
    assert_eq!(image.extent.size, Some((4000, 3000)));
    let video = summaries.get(&seed.video).expect("the video");
    assert_eq!(video.total, 0);
    assert!(video.first.is_empty());
    assert_eq!(video.extent.duration_ms, Some(90_000));
    assert!(!summaries.contains_key(&seed.private));

    let counts = anonymous.counts(&media).await.expect("counts");
    assert_eq!(counts.get(&seed.image), Some(&1));
    assert_eq!(counts.get(&seed.video), Some(&0));
    assert_eq!(counts.get(&seed.private), None);
}

/// An extent source that fails the test if a count asks it for a file: the
/// item's counts are one statement about rows, not one open per media.
struct NoExtents;

impl ExtentSource for NoExtents {
    fn extent<'a>(&'a self, _: &'a MediaRow) -> Pin<Box<dyn Future<Output = Extent> + Send + 'a>> {
        Box::pin(async { panic!("a note count must not read a file") })
    }
}

/// One authored row, written straight to the table: the counts read, not the
/// create, is what these tests are about.
async fn insert_note(seed: &Seed, media: MediaId, motivation: &str) {
    // The table holds a `supplementing` row only with a time range.
    let timed = motivation == "supplementing";
    MediaAnnotationRepo::new(&seed.pool)
        .insert(
            media,
            Some(seed.other),
            None,
            &MediaAnnotationWrite {
                motivation,
                selector_kind: if timed { "time" } else { "whole" },
                selector: None,
                time_ms: timed.then_some((0, 1000)),
                body_json: r#"[{"type":"TextualBody","value":"n","format":"text/plain"}]"#,
                body_text: "n",
                lang: None,
                extensions: None,
            },
        )
        .await
        .expect("a note row");
}

#[tokio::test]
async fn an_items_counts_answer_every_readable_media_with_the_rights_its_container_states() {
    let seed = seed().await;
    insert_note(&seed, seed.image, "commenting").await;
    insert_note(&seed, seed.image, "commenting").await;
    insert_note(&seed, seed.video, "commenting").await;
    insert_note(&seed, seed.video, "supplementing").await;
    // An imported cue is a row the container lists under `origin=all` only, so
    // the notes view the item page draws never counts it.
    MediaAnnotationRepo::new(&seed.pool)
        .replace_track(
            seed.video,
            Some("en"),
            &[Cue {
                start_ms: 0,
                end_ms: 1000,
                body_json: r#"[{"type":"TextualBody","value":"A cue","format":"text/plain"}]"#,
                body_text: "A cue",
            }],
        )
        .await
        .expect("a caption row");

    let anonymous_api = api(&seed, Actor::anonymous()).await;
    let anonymous = Annotations::new(&anonymous_api, &NoExtents);
    let counts = anonymous
        .item_counts(seed.item)
        .await
        .expect("the item's counts");
    let by_media: HashMap<MediaId, (u32, Rights)> = counts
        .iter()
        .map(|entry| (entry.media, (entry.count, entry.rights)))
        .collect();
    assert_eq!(
        by_media.get(&seed.image),
        Some(&(2, Rights::Read)),
        "an anonymous reader counts the notes and not the file"
    );
    assert_eq!(
        by_media.get(&seed.video),
        Some(&(2, Rights::Read)),
        "the hand-written caption is a note; the imported cue is not"
    );
    assert_eq!(by_media.get(&seed.pdf), Some(&(0, Rights::Read)));
    assert_eq!(
        by_media.get(&seed.private),
        None,
        "a private media is absent"
    );

    let owner_api = api(&seed, Actor::new(seed.owner, "author")).await;
    let owner = Annotations::new(&owner_api, &NoExtents);
    let own = owner
        .item_counts(seed.item)
        .await
        .expect("the owner's counts");
    let own_by_media: HashMap<MediaId, Rights> = own
        .iter()
        .map(|entry| (entry.media, entry.rights))
        .collect();
    assert_eq!(own_by_media.get(&seed.image), Some(&Rights::ReadWrite));
    assert_eq!(
        own_by_media.get(&seed.theirs),
        Some(&Rights::Read),
        "another author's media is readable, not writable"
    );
    assert_eq!(own_by_media.get(&seed.private), Some(&Rights::ReadWrite));

    set_switch(&seed, false).await;
    let reader_api = api(&seed, Actor::anonymous()).await;
    let reader = Annotations::new(&reader_api, &NoExtents);
    let hidden = reader
        .item_counts(seed.item)
        .await
        .expect("the switch's counts");
    let hidden_by_media: HashMap<MediaId, (u32, Rights)> = hidden
        .iter()
        .map(|entry| (entry.media, (entry.count, entry.rights)))
        .collect();
    assert_eq!(
        hidden_by_media.get(&seed.video),
        Some(&(1, Rights::ReadCaptionsOnly)),
        "the switch keeps the caption row and hides the comment"
    );
    assert_eq!(
        hidden_by_media.get(&seed.image),
        Some(&(0, Rights::ReadCaptionsOnly))
    );
}

#[tokio::test]
async fn summaries_count_only_captions_when_the_switch_is_off() {
    let seed = seed().await;
    let editor_api = api(&seed, Actor::new(seed.editor, "editor")).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&editor_api, &extents);
    let iris = Iris::new(BASE);
    let target = canvas(&notes, &iris, seed.video).await;
    create(
        &notes,
        seed.video,
        &whole(&target, "A comment", "commenting"),
        &iris,
        None,
    )
    .await;
    create(
        &notes,
        seed.video,
        &cue(&target, "A cue", "1", "2"),
        &iris,
        None,
    )
    .await;
    set_switch(&seed, false).await;

    let anonymous_api = api(&seed, Actor::anonymous()).await;
    let anonymous = Annotations::new(&anonymous_api, &extents);
    let summaries = anonymous
        .summaries(&[seed.video], 8)
        .await
        .expect("summaries");
    let summary = summaries.get(&seed.video).expect("the video");
    assert_eq!(summary.total, 1);
    assert_eq!(
        summary.first[0].draft.motivation,
        omeka_api::annotation::Motivation::Supplementing
    );
    let counts = anonymous.counts(&[seed.video]).await.expect("counts");
    assert_eq!(counts.get(&seed.video), Some(&1));
}

#[tokio::test]
async fn a_page_under_the_switch_off_still_names_its_rights() {
    let seed = seed().await;
    set_switch(&seed, false).await;
    let anonymous_api = api(&seed, Actor::anonymous()).await;
    let extents = FakeExtents::of(&seed);
    let notes = Annotations::new(&anonymous_api, &extents);
    let page = notes.page(seed.image, None).await.expect("page");
    assert_eq!(page.rights, Rights::ReadCaptionsOnly);
    assert_eq!(page.total, 0);
    assert!(page.items.is_empty());
}

#[test]
fn the_switch_hides_notes_from_search_only_without_the_write_grant() {
    let owner = UserId(7);
    let items_owner = Some(UserId(7));
    let anonymous = Actor::anonymous();
    let editor = Actor::new(UserId(3), "editor");
    let author = Actor::new(owner, "author");
    let researcher = Actor::new(UserId(9), "researcher");

    assert!(search_scope(&anonymous, items_owner, false).hide_notes);
    assert!(!search_scope(&anonymous, items_owner, true).hide_notes);
    assert!(!search_scope(&editor, items_owner, false).hide_notes);
    assert!(
        !search_scope(&author, items_owner, false).hide_notes,
        "an author may update the media they own"
    );
    assert!(
        search_scope(&author, Some(UserId(8)), false).hide_notes,
        "an author may not update another user's media"
    );
    assert!(search_scope(&researcher, items_owner, false).hide_notes);
    assert!(!search_scope(&anonymous, items_owner, true).privileged);
    assert_eq!(search_scope(&author, items_owner, true).user, Some(owner));
}
