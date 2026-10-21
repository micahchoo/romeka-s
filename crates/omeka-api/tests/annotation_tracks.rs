//! Caption files: the corpus parses to the same cues in both formats, a
//! rendered file parses back equal, and hostile input ends in a typed refusal
//! or a bounded result.
//!
//! Every test drives the public `omeka_api::annotation::tracks` surface, the
//! one unit 11's tracks route will call.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

use omeka_api::annotation::tracks::{
    CueDiff, ExistingCue, MAX_BYTES, MAX_CUES, ParsedCue, TrackError, TrackFormat, diff, parse,
    render_vtt,
};

const CORPUS: &str = "A red square, then a blue bar.";

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media-kinds")
        .join(name);
    fs::read_to_string(path).expect("the media-kind caption fixture")
}

fn cue(start_ms: u64, end_ms: u64, text: &str) -> ParsedCue {
    ParsedCue {
        start_ms,
        end_ms,
        text: text.to_string(),
    }
}

fn body(blocks: &[&str]) -> String {
    format!("WEBVTT\n\n{}\n", blocks.join("\n\n"))
}

#[test]
fn the_corpus_reads_the_same_in_vtt_and_srt() {
    let vtt = parse(&fixture("captions.vtt"), TrackFormat::Vtt).unwrap();
    let srt = parse(&fixture("captions.srt"), TrackFormat::Srt).unwrap();
    assert_eq!(vtt, vec![cue(0, 400, CORPUS)]);
    assert_eq!(vtt, srt);
}

#[test]
fn a_rendered_track_parses_back_equal() {
    let parsed = parse(&fixture("captions.vtt"), TrackFormat::Vtt).unwrap();
    let pairs: Vec<(u64, u64, &str)> = parsed
        .iter()
        .map(|cue| (cue.start_ms, cue.end_ms, cue.text.as_str()))
        .collect();
    let rendered = render_vtt(&pairs);
    assert_eq!(
        rendered,
        "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA red square, then a blue bar.\n"
    );
    assert_eq!(
        parse(&rendered, TrackFormat::Vtt).unwrap(),
        parsed,
        "the render does not read back as the cues it was given"
    );
}

#[test]
fn both_decimal_marks_read_in_both_formats() {
    let comma = body(&["00:00:01,250 --> 00:01:02,500\nComma"]);
    assert_eq!(
        parse(&comma, TrackFormat::Vtt).unwrap(),
        vec![cue(1_250, 62_500, "Comma")]
    );
    let dot = "1\n00:00:01.250 --> 00:01:02.500\nDot\n";
    assert_eq!(
        parse(dot, TrackFormat::Srt).unwrap(),
        vec![cue(1_250, 62_500, "Dot")]
    );
    let hours = body(&["01:02:03.004 --> 01:02:04.500\nLong"]);
    assert_eq!(
        parse(&hours, TrackFormat::Vtt).unwrap(),
        vec![cue(3_723_004, 3_724_500, "Long")]
    );
}

#[test]
fn note_style_and_region_blocks_are_skipped() {
    let file = body(&[
        "NOTE this file was checked by hand",
        "STYLE\n::cue { color: red }",
        "00:00:01.000 --> 00:00:02.000\none",
        "REGION\nid:fred",
        "00:00:03.000 --> 00:00:04.000\ntwo",
    ]);
    assert_eq!(
        parse(&file, TrackFormat::Vtt).unwrap(),
        vec![cue(1_000, 2_000, "one"), cue(3_000, 4_000, "two")]
    );
}

#[test]
fn cue_tags_are_stripped_and_script_stays_plain_text() {
    let file = body(&[
        "00:00:01.000 --> 00:00:02.000\nHello <b>world</b>, <v Roger>says</v> <c.yellow>this</c>",
        "00:00:03.000 --> 00:00:04.000\n<00:00:03.500>mid-cue <i>stress</i>",
        "00:00:05.000 --> 00:00:06.000\n<script>alert(1)</script>",
    ]);
    let cues = parse(&file, TrackFormat::Vtt).unwrap();
    assert_eq!(cues[0].text, "Hello world, says this");
    assert_eq!(cues[1].text, "mid-cue stress");
    assert_eq!(
        cues[2].text, "<script>alert(1)</script>",
        "a tag WebVTT does not define is text the reader sees, never markup"
    );
    let file = body(&["00:00:01.000 --> 00:00:02.000\nif x < 3 and y > 2"]);
    assert_eq!(
        parse(&file, TrackFormat::Vtt).unwrap()[0].text,
        "if x < 3 and y > 2"
    );
}

#[test]
fn a_cue_with_no_text_is_skipped() {
    let file = body(&[
        "00:00:01.000 --> 00:00:02.000",
        "00:00:03.000 --> 00:00:04.000\nkept",
    ]);
    assert_eq!(
        parse(&file, TrackFormat::Vtt).unwrap(),
        vec![cue(3_000, 4_000, "kept")]
    );
}

#[test]
fn a_header_with_metadata_or_missing_its_blank_line_still_reads() {
    let with_metadata = "WEBVTT - checked\nKind: captions\nLanguage: en\n\n\
                         00:00:01.000 --> 00:00:02.000\nhi\n";
    assert_eq!(
        parse(with_metadata, TrackFormat::Vtt).unwrap(),
        vec![cue(1_000, 2_000, "hi")]
    );
    let without_blank = "WEBVTT\n00:00:01.000 --> 00:00:02.000\nhi\n";
    assert_eq!(
        parse(without_blank, TrackFormat::Vtt).unwrap(),
        vec![cue(1_000, 2_000, "hi")]
    );
}

#[test]
fn a_bom_and_crlf_read() {
    let file = "\u{feff}WEBVTT\r\n\r\n1\r\n00:00:01,000 --> 00:00:02,000\r\nhi\r\n";
    assert_eq!(
        parse(file, TrackFormat::Vtt).unwrap(),
        vec![cue(1_000, 2_000, "hi")]
    );
    assert_eq!(
        parse(
            "\u{feff}00:00:01,000 --> 00:00:02,000\nhi\n",
            TrackFormat::Srt
        )
        .unwrap(),
        vec![cue(1_000, 2_000, "hi")]
    );
}

#[test]
fn a_file_that_does_not_start_with_webvtt_is_refused() {
    assert_eq!(
        parse("00:00:01.000 --> 00:00:02.000\nhi\n", TrackFormat::Vtt),
        Err(TrackError::NoHeader)
    );
    assert_eq!(parse("", TrackFormat::Vtt), Err(TrackError::NoHeader));
    assert_eq!(
        parse(
            "WEBVTTX\n\n00:00:01.000 --> 00:00:02.000\nhi\n",
            TrackFormat::Vtt
        ),
        Err(TrackError::NoHeader)
    );
}

#[test]
fn a_line_that_is_not_a_cue_time_names_its_line() {
    let file = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nok\n\n\
                00:00:03.000 -> 00:00:04.000\nbad\n";
    let error = parse(file, TrackFormat::Vtt).unwrap_err();
    assert_eq!(error, TrackError::NotACueTime { line: 6 });
    assert_eq!(error.to_string(), "Line 6 is not a cue time.");

    let fraction = body(&["00:00:01.0000 --> 00:00:02.000\ntoo precise"]);
    assert_eq!(
        parse(&fraction, TrackFormat::Vtt).unwrap_err(),
        TrackError::NotACueTime { line: 3 }
    );

    let minutes = body(&["00:99:00.000 --> 00:00:02.000\nnot a time"]);
    assert_eq!(
        parse(&minutes, TrackFormat::Vtt).unwrap_err(),
        TrackError::NotACueTime { line: 3 }
    );
}

#[test]
fn a_time_that_does_not_end_after_it_begins_is_refused_with_its_line() {
    let file = body(&["00:00:05.000 --> 00:00:05.000\nzero length"]);
    assert_eq!(
        parse(&file, TrackFormat::Vtt).unwrap_err(),
        TrackError::EmptyRange { line: 3 }
    );
    let reversed = body(&["00:00:05.000 --> 00:00:04.000\nbackwards"]);
    assert_eq!(
        parse(&reversed, TrackFormat::Vtt).unwrap_err(),
        TrackError::EmptyRange { line: 3 }
    );
}

#[test]
fn five_thousand_cues_read_and_five_thousand_and_one_are_refused() {
    let at_limit: Vec<String> = (0..MAX_CUES)
        .map(|index| format!("00:00:01.000 --> 00:00:02.000\ncue {index}"))
        .collect();
    let refs: Vec<&str> = at_limit.iter().map(String::as_str).collect();
    let cues = parse(&body(&refs), TrackFormat::Vtt).unwrap();
    assert_eq!(cues.len(), MAX_CUES);

    let over: Vec<String> = (0..=MAX_CUES)
        .map(|index| format!("00:00:01.000 --> 00:00:02.000\ncue {index}"))
        .collect();
    let refs: Vec<&str> = over.iter().map(String::as_str).collect();
    assert_eq!(
        parse(&body(&refs), TrackFormat::Vtt).unwrap_err(),
        TrackError::TooManyCues
    );
}

#[test]
fn one_mebibyte_reads_and_one_byte_more_is_refused() {
    let head = "WEBVTT\n\n00:00:00.000 --> 00:00:00.400\n";
    let mut exact = String::from(head);
    exact.push_str(&"a".repeat(MAX_BYTES - head.len() - 1));
    exact.push('\n');
    assert_eq!(exact.len(), MAX_BYTES);
    let cues = parse(&exact, TrackFormat::Vtt).unwrap();
    assert_eq!(cues[0].text.len(), MAX_BYTES - head.len() - 1);

    exact.push('b');
    assert_eq!(exact.len(), MAX_BYTES + 1);
    assert_eq!(parse(&exact, TrackFormat::Vtt), Err(TrackError::TooLarge));

    let mut wide = String::from(head);
    wide.push_str(&"é".repeat((MAX_BYTES - head.len()) / 2 + 1));
    assert!(wide.chars().count() < MAX_BYTES);
    assert!(wide.len() > MAX_BYTES);
    assert_eq!(parse(&wide, TrackFormat::Vtt), Err(TrackError::TooLarge));
}

#[test]
fn render_orders_cues_and_collapses_a_blank_line() {
    let rendered = render_vtt(&[(2_000, 3_000, "second"), (0, 1_000, "first\n\nstill first")]);
    assert_eq!(
        rendered,
        "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\nfirst\nstill first\n\n\
         00:00:02.000 --> 00:00:03.000\nsecond\n"
    );
    assert_eq!(
        parse(&rendered, TrackFormat::Vtt).unwrap(),
        vec![
            cue(0, 1_000, "first\nstill first"),
            cue(2_000, 3_000, "second")
        ]
    );
}

#[test]
fn render_writes_hours_for_a_long_cue() {
    let rendered = render_vtt(&[(3_723_004, 3_724_500, "long")]);
    assert!(rendered.contains("01:02:03.004 --> 01:02:04.500"));
}

#[test]
fn the_diff_counts_a_new_file_against_the_track_it_replaces() {
    let old = [
        ExistingCue {
            start_ms: 0,
            end_ms: 400,
            text: "Hello",
            hand_edited: false,
        },
        ExistingCue {
            start_ms: 400,
            end_ms: 800,
            text: "Old words",
            hand_edited: false,
        },
        ExistingCue {
            start_ms: 900,
            end_ms: 1_000,
            text: "Hand fix",
            hand_edited: true,
        },
    ];
    let new = [
        cue(0, 400, "Hello"),
        cue(400, 800, "New words"),
        cue(1_000, 1_400, "Fresh"),
    ];
    assert_eq!(
        diff(&old, &new),
        CueDiff {
            cues: 3,
            start_ms: 0,
            end_ms: 1_400,
            replaces: 3,
            added: 1,
            changed: 1,
            removed: 1,
            unchanged: 1,
            hand_edited_kept: 0,
            hand_edited_lost: 1,
        }
    );
}

#[test]
fn the_same_file_changes_nothing_and_keeps_a_hand_edit_that_matches() {
    let old = [
        ExistingCue {
            start_ms: 0,
            end_ms: 400,
            text: "Hello",
            hand_edited: true,
        },
        ExistingCue {
            start_ms: 400,
            end_ms: 800,
            text: "World",
            hand_edited: false,
        },
    ];
    let new = [cue(0, 400, "Hello"), cue(400, 800, "World")];
    assert_eq!(
        diff(&old, &new),
        CueDiff {
            cues: 2,
            start_ms: 0,
            end_ms: 800,
            replaces: 2,
            added: 0,
            changed: 0,
            removed: 0,
            unchanged: 2,
            hand_edited_kept: 1,
            hand_edited_lost: 0,
        }
    );
}

#[test]
fn two_cues_at_one_time_pair_the_equal_words_first() {
    let old = [
        ExistingCue {
            start_ms: 0,
            end_ms: 400,
            text: "kept words",
            hand_edited: true,
        },
        ExistingCue {
            start_ms: 0,
            end_ms: 400,
            text: "lost words",
            hand_edited: false,
        },
    ];
    let new = [cue(0, 400, "kept words")];
    let result = diff(&old, &new);
    assert_eq!(result.unchanged, 1);
    assert_eq!(result.hand_edited_kept, 1);
    assert_eq!(result.removed, 1);
    assert_eq!(result.hand_edited_lost, 0);
}

#[test]
fn an_empty_file_diffs_to_zeroes() {
    assert_eq!(diff(&[], &[]), CueDiff::default());
    let old = [ExistingCue {
        start_ms: 0,
        end_ms: 400,
        text: "gone",
        hand_edited: true,
    }];
    assert_eq!(
        diff(&old, &[]),
        CueDiff {
            cues: 0,
            start_ms: 0,
            end_ms: 0,
            replaces: 1,
            added: 0,
            changed: 0,
            removed: 1,
            unchanged: 0,
            hand_edited_kept: 0,
            hand_edited_lost: 1,
        }
    );
}

#[test]
fn hostile_files_end_in_a_typed_refusal_or_a_bounded_result() {
    let nested = format!(
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n{}\n",
        "<b>".repeat(100_000)
    );
    let unclosed = format!(
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n{}\n",
        "<".repeat(200_000)
    );
    let not_tags = format!(
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n{}\n",
        "< ".repeat(300_000)
    );
    let cases = [
        nested,
        unclosed,
        not_tags,
        body(&["--> -->"]),
        body(&["00:00:01.000 -->"]),
        body(&["::cue"]),
        String::new(),
        "WEBVTT".to_string(),
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n".to_string(),
    ];
    for file in cases {
        match parse(&file, TrackFormat::Vtt) {
            Ok(cues) => {
                assert!(cues.len() <= MAX_CUES);
                for cue in cues {
                    assert!(cue.text.len() <= file.len());
                    assert!(cue.end_ms > cue.start_ms);
                }
            }
            Err(error) => {
                let typed = matches!(
                    error,
                    TrackError::TooLarge
                        | TrackError::TooManyCues
                        | TrackError::NoHeader
                        | TrackError::NotACueTime { .. }
                        | TrackError::EmptyRange { .. }
                );
                assert!(typed, "a refusal is one of the typed errors");
            }
        }
    }
}
