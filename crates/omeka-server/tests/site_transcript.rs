//! The media page's transcript, read through the public route as a visitor
//! reads it.
//!
//! Expected values come from the media UI design (C4, micro-interaction 1b),
//! not from the renderer: every cue of one language is in the page, each line
//! links its own moment with `?t=`, and an audio file's captions are read in
//! the list because its element draws none.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{append_key, get, router, send, site_pool};
use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};

const VTT: &str = "text/vtt";

const EN_CUES: &str = "WEBVTT\n\n\
00:00:00.000 --> 00:00:00.400\nA red square, then a blue bar.\n\n\
00:01:02.000 --> 00:01:10.000\nThe second line is here.\n\n\
00:01:11.000 --> 00:01:12.000\nA closing remark.\n";

const FR_CUES: &str = "WEBVTT\n\n\
00:00:00.000 --> 00:00:00.400\nUn carre rouge, puis une barre bleue.\n\n\
00:01:02.000 --> 00:01:10.000\nLa deuxieme ligne est ici.\n";

/// A recording on the seeded item.
async fn add_recording(
    pool: &sqlx::SqlitePool,
    owner: omeka_db::ids::UserId,
    item: ItemId,
    source: &str,
    extension: &str,
    media_type: &str,
    public: bool,
) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some("A recording".to_string()),
                is_public: public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(source.to_string()),
            media_type: Some(media_type.to_string()),
            storage_id: Some(format!("{extension}-missing")),
            extension: Some(extension.to_string()),
            sha256: Some("missing".to_string()),
            size: Some(1),
            has_original: true,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .expect("create a media row")
        .id()
}

/// A caption import through the route the admin calls.
async fn import(app: &Router, media: MediaId, lang: &str, body: &str) {
    let request = Request::builder()
        .method("POST")
        .uri(append_key(&format!(
            "/annotations/media/{}/tracks?lang={lang}",
            media.0
        )))
        .header("content-type", VTT)
        .body(Body::from(body.to_string()))
        .unwrap();
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

/// The media page of `media`, with an optional query.
async fn page(app: &Router, media: MediaId, query: &str) -> String {
    let path = format!("/s/museum/media/{}{query}", media.0);
    let answer = send(app.clone(), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.body
}

/// One WAP supplementing note on a time range, as the admin writes one.
fn timed_note(canvas: &str, text: &str, start: &str, end: &str) -> String {
    serde_json::json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "type": "Annotation",
        "motivation": "supplementing",
        "body": { "type": "TextualBody", "value": text, "format": "text/plain", "language": "en" },
        "target": {
            "type": "SpecificResource",
            "source": { "id": canvas, "type": "Canvas" },
            "selector": {
                "type": "FragmentSelector",
                "conformsTo": "http://www.w3.org/TR/media-frags/",
                "value": format!("t={start},{end}"),
            },
        },
    })
    .to_string()
}

/// A video with one English track.
async fn captioned_video(pool: &sqlx::SqlitePool, seed: &common::Seed) -> (Router, MediaId) {
    let app = router(pool.clone());
    let media = add_recording(
        pool,
        seed.user,
        seed.item,
        "clip.mp4",
        "mp4",
        "video/mp4",
        true,
    )
    .await;
    import(&app, media, "en", EN_CUES).await;
    (app, media)
}

/// Every cue of the language is in the page, each line a link to its moment.
#[tokio::test]
async fn the_media_page_lists_every_cue_with_its_time() {
    let (pool, seed) = site_pool().await;
    let (app, media) = captioned_video(&pool, &seed).await;
    let body = page(&app, media, "").await;

    assert!(
        body.contains("<section class=\"transcript\" id=\"transcript\" data-transcript>"),
        "{body}"
    );
    assert!(
        body.contains(
            "<summary class=\"transcript-heading\">Transcript · English · 3 lines</summary>"
        ),
        "{body}"
    );
    assert!(
        body.contains("<p class=\"transcript-facts\">Captions: English. Transcript below.</p>"),
        "{body}"
    );
    assert!(body.contains("A red square, then a blue bar."), "{body}");
    assert!(body.contains("The second line is here."), "{body}");
    assert!(body.contains("A closing remark."), "{body}");
    assert!(
        body.contains(&format!(
            "href=\"/s/museum/media/{}?t=62&amp;transcript=en#player\"",
            media.0
        )),
        "{body}"
    );
    assert!(
        body.contains(&format!(
            "href=\"/s/museum/media/{}?t=0&amp;transcript=en#player\"",
            media.0
        )),
        "{body}"
    );
    assert!(
        body.contains("<span class=\"transcript-time\">1:02</span>"),
        "{body}"
    );
    // The page ships the one notes and transcript script, and the reload lands
    // on the player.
    assert!(
        body.contains("<script src=\"/assets/js/annotations.js?v="),
        "{body}"
    );
    assert!(body.contains("id=\"player\""), "{body}");
    // One language is no choice, so there is no switcher.
    assert!(!body.contains("transcript-languages"), "{body}");
    // No live region in the transcript: a screen reader is never interrupted
    // per cue. The share block's copy status is elsewhere on the page.
    let transcript = body
        .split("<section class=\"transcript\"")
        .nth(1)
        .and_then(|rest| rest.split("</section>").next())
        .unwrap_or_default();
    assert!(!transcript.contains("aria-live"), "{transcript}");
}

/// A language link shows that language's cues, and only those.
#[tokio::test]
async fn a_language_link_shows_that_languages_cues() {
    let (pool, seed) = site_pool().await;
    let (app, media) = captioned_video(&pool, &seed).await;
    import(&app, media, "fr", FR_CUES).await;

    let english = page(&app, media, "").await;
    assert!(
        english.contains(&format!(
            "<a class=\"transcript-language\" href=\"/s/museum/media/{}?transcript=en#transcript\" \
             hreflang=\"en\" lang=\"en\" aria-current=\"true\">English (3)</a>",
            media.0
        )),
        "{english}"
    );
    assert!(
        english.contains(&format!(
            "<a class=\"transcript-language\" href=\"/s/museum/media/{}?transcript=fr#transcript\" \
             hreflang=\"fr\" lang=\"fr\">Français (2)</a>",
            media.0
        )),
        "{english}"
    );
    assert!(
        english.contains(
            "<p class=\"transcript-facts\">Captions: English, Français. Transcript below.</p>"
        ),
        "{english}"
    );

    let french = page(&app, media, "?transcript=fr").await;
    assert!(
        french.contains("Un carre rouge, puis une barre bleue."),
        "{french}"
    );
    assert!(
        !french.contains("A red square, then a blue bar."),
        "{french}"
    );
    assert!(
        french.contains(&format!(
            "<a class=\"transcript-language\" href=\"/s/museum/media/{}?transcript=fr#transcript\" \
             hreflang=\"fr\" lang=\"fr\" aria-current=\"true\">Français (2)</a>",
            media.0
        )),
        "{french}"
    );
    assert!(
        french.contains(
            "<summary class=\"transcript-heading\">Transcript · Français · 2 lines</summary>"
        ),
        "{french}"
    );
    // A line keeps the chosen language, so the reload shows the same text.
    assert!(
        french.contains("?t=62&amp;transcript=fr#player"),
        "{french}"
    );
}

/// A link to a language the recording does not offer falls back to the default
/// track rather than failing.
#[tokio::test]
async fn an_unknown_language_falls_back_to_the_default_track() {
    let (pool, seed) = site_pool().await;
    let (app, media) = captioned_video(&pool, &seed).await;
    import(&app, media, "fr", FR_CUES).await;

    for query in ["?transcript=de", "?transcript=", "?transcript=%3Cscript%3E"] {
        let body = page(&app, media, query).await;
        assert!(
            body.contains("A red square, then a blue bar."),
            "{query}: {body}"
        );
    }
}

/// An `<audio>` element draws no captions, so the page draws a band and says
/// the captions are in the list.
#[tokio::test]
async fn an_audio_page_draws_the_band_and_says_where_the_captions_are() {
    let (pool, seed) = site_pool().await;
    let app = router(pool.clone());
    let media = add_recording(
        &pool,
        seed.user,
        seed.item,
        "clip.mp3",
        "mp3",
        "audio/mpeg",
        true,
    )
    .await;
    import(&app, media, "en", EN_CUES).await;

    let body = page(&app, media, "").await;
    assert!(
        body.contains(
            "<p class=\"transcript-band\" data-transcript-band hidden aria-hidden=\"true\"></p>"
        ),
        "{body}"
    );
    assert!(
        body.contains("<p class=\"transcript-facts\">Captions show in the transcript below.</p>"),
        "{body}"
    );
    assert!(body.contains("A red square, then a blue bar."), "{body}");
}

/// A video with no caption track keeps its page exactly as it was: no section,
/// no script.
#[tokio::test]
async fn a_video_without_captions_has_no_transcript() {
    let (pool, seed) = site_pool().await;
    let app = router(pool.clone());
    let media = add_recording(
        &pool,
        seed.user,
        seed.item,
        "clip.mp4",
        "mp4",
        "video/mp4",
        true,
    )
    .await;

    let body = page(&app, media, "").await;
    assert!(!body.contains("data-transcript"), "{body}");
    assert!(!body.contains("transcript-facts"), "{body}");
}

/// A timed note a person wrote is a cue too: the transcript and `captions.vtt`
/// serve the same supplementing rows.
#[tokio::test]
async fn a_hand_written_caption_note_is_a_line_too() {
    let (pool, seed) = site_pool().await;
    let (app, media) = captioned_video(&pool, &seed).await;
    let canvas = format!(
        "http://localhost/iiif/3/item/{}/canvas/{}",
        seed.item.0, media.0
    );
    let request = Request::builder()
        .method("POST")
        .uri(append_key(&format!("/annotations/media/{}/", media.0)))
        .header("content-type", "application/ld+json")
        .body(Body::from(timed_note(
            &canvas,
            "A hand-written warning.",
            "30",
            "31",
        )))
        .unwrap();
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);

    let body = page(&app, media, "").await;
    assert!(body.contains("A hand-written warning."), "{body}");
    assert!(
        body.contains("<span class=\"transcript-time\">0:30</span>"),
        "{body}"
    );
    assert!(
        body.contains(
            "<summary class=\"transcript-heading\">Transcript · English · 4 lines</summary>"
        ),
        "{body}"
    );
}

/// A file with notes and captions draws both surfaces, names one player and
/// loads the one shared script once.
#[tokio::test]
async fn notes_and_a_transcript_share_one_player_and_one_script() {
    let (pool, seed) = site_pool().await;
    let (app, media) = captioned_video(&pool, &seed).await;
    let canvas = format!(
        "http://localhost/iiif/3/item/{}/canvas/{}",
        seed.item.0, media.0
    );
    let request = Request::builder()
        .method("POST")
        .uri(append_key(&format!("/annotations/media/{}/", media.0)))
        .header("content-type", "application/ld+json")
        .body(Body::from(timed_note(
            &canvas,
            "The ship sinks here.",
            "30",
            "31",
        )))
        .unwrap();
    let answer = send(app.clone(), request).await;
    assert_eq!(answer.status, StatusCode::CREATED, "{}", answer.body);

    let body = page(&app, media, "").await;
    // Both surfaces are on the page: the notes figure with its list, and the
    // transcript under the player.
    assert!(body.contains("class=\"annotated-media"), "{body}");
    assert!(body.contains("class=\"annotation-list\""), "{body}");
    assert!(body.contains("The ship sinks here."), "{body}");
    assert!(body.contains("data-transcript"), "{body}");
    assert!(body.contains("A red square, then a blue bar."), "{body}");
    // One script, one player: the figure and the transcript agree on both.
    assert_eq!(body.matches("js/annotations.js").count(), 1, "{body}");
    assert_eq!(body.matches("id=\"player\"").count(), 1, "{body}");
}

/// A file the caller may not see has no page, so it has no transcript either.
#[tokio::test]
async fn a_private_recording_has_no_transcript_page() {
    let (pool, seed) = site_pool().await;
    let app = router(pool.clone());
    let media = add_recording(
        &pool,
        seed.user,
        seed.item,
        "closed.mp4",
        "mp4",
        "video/mp4",
        false,
    )
    .await;
    import(&app, media, "en", EN_CUES).await;

    let path = format!("/s/museum/media/{}", media.0);
    let answer = send(app.clone(), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert!(
        !answer.body.contains("A red square, then a blue bar."),
        "{}",
        answer.body
    );
}

/// The player's anchor is the media page's, never an item page's: an item can
/// show several files.
#[tokio::test]
async fn only_the_media_page_names_the_player() {
    let (pool, seed) = site_pool().await;
    let (app, media) = captioned_video(&pool, &seed).await;
    let media_page = page(&app, media, "").await;
    assert!(media_page.contains("id=\"player\""), "{media_page}");

    let path = format!("/s/museum/item/{}", seed.item.0);
    let answer = send(app.clone(), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(!answer.body.contains("id=\"player\""), "{}", answer.body);
}

/// A page that shows a thousand cues is still one document find-in-page
/// searches. The size is recorded here so a change that costs the page bytes
/// has to say so.
#[tokio::test]
async fn a_thousand_cues_are_one_page() {
    let (pool, seed) = site_pool().await;
    let app = router(pool.clone());
    let media = add_recording(
        &pool,
        seed.user,
        seed.item,
        "hour.mp4",
        "mp4",
        "video/mp4",
        true,
    )
    .await;
    let mut vtt = String::from("WEBVTT\n");
    for index in 0..1000 {
        vtt.push_str(&format!(
            "\n00:{:02}:{:02}.000 --> 00:{:02}:{:02}.500\nCue number {index} speaks.\n",
            index / 60 % 60,
            index % 60,
            (index + 1) / 60 % 60,
            (index + 1) % 60
        ));
    }
    import(&app, media, "en", &vtt).await;

    let body = page(&app, media, "").await;
    assert!(body.contains("Cue number 999 speaks."), "{}", body.len());
    assert!(
        body.contains(
            "<summary class=\"transcript-heading\">Transcript · English · 1000 lines</summary>"
        ),
        "{}",
        &body[..body.len().min(400)]
    );
    println!("a 1000 cue media page is {} bytes", body.len());
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, body.as_bytes()).unwrap();
    let gzipped = encoder.finish().unwrap();
    println!("a 1000 cue media page is {} bytes gzipped", gzipped.len());
}
