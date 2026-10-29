//! What one saved state of a site page changed against the state before it (a Romeka addition).
//!
//! Both states are page snapshots as [`crate::read::page_snapshot`] makes them. A save rewrites
//! every block row, so a block has no id that lasts from one version to the next. Blocks are
//! matched by content instead, in three passes:
//!
//! 1. A block equal to an unmatched block of the older state is kept, the first equal one in
//!    order. Equal means the same layout, data, layout data and attachments (item id, media id,
//!    caption); attachment URLs are left out, so a change of host name changes nothing.
//! 2. A block left over is changed when a left-over block of the same kind remains in the older
//!    state, the first in order. Its kind is its layout and, for a Romeka block, the Romeka block
//!    it carries.
//! 3. Blocks still left over are added (newer state) or removed (older state).
//!
//! A kept or changed block moved when its order against the other kept and changed blocks is not
//! in the longest run that kept its order. The counts are exact for this matching.

use serde::Serialize;
use serde_json::Value;

/// The title before and after a save that changed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleChange {
    #[serde(rename = "o:before")]
    pub before: String,
    #[serde(rename = "o:after")]
    pub after: String,
}

/// A changed block, as the admin names it: its layout and layout data in the newer state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangedBlock {
    #[serde(rename = "o:layout")]
    pub layout: Value,
    #[serde(rename = "o:layout_data")]
    pub layout_data: Value,
}

/// How many changed blocks a summary names. More than this, and only the count is given.
pub const NAMED_BLOCKS: usize = 2;

/// What a save changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VersionChanges {
    #[serde(rename = "o:title")]
    pub title: Option<TitleChange>,
    /// The slug, the visibility, the page layout or the page layout data differ.
    #[serde(rename = "o:settings_changed")]
    pub settings_changed: bool,
    #[serde(rename = "o:blocks_added")]
    pub added: usize,
    #[serde(rename = "o:blocks_removed")]
    pub removed: usize,
    #[serde(rename = "o:blocks_changed")]
    pub changed: usize,
    #[serde(rename = "o:blocks_moved")]
    pub moved: usize,
    /// The changed blocks in the newer state's order, when there are at most [`NAMED_BLOCKS`].
    #[serde(rename = "o:changed_blocks")]
    pub changed_blocks: Vec<ChangedBlock>,
}

const SETTINGS: [&str; 4] = ["o:slug", "o:is_public", "o:layout", "o:layout_data"];

/// What `after` changed against `before`, two snapshots of one page.
pub fn changes(before: &Value, after: &Value) -> VersionChanges {
    let title = |state: &Value| state["o:title"].as_str().unwrap_or("").to_string();
    let (old_title, new_title) = (title(before), title(after));
    let old = blocks(before);
    let new = blocks(after);
    let matching = Matching::of(&old, &new);
    let changed: Vec<usize> = matching
        .pairs
        .iter()
        .filter(|pair| pair.changed)
        .map(|pair| pair.new)
        .collect();
    VersionChanges {
        title: (old_title != new_title).then_some(TitleChange {
            before: old_title,
            after: new_title,
        }),
        settings_changed: SETTINGS.iter().any(|key| before[key] != after[key]),
        added: new.len() - matching.pairs.len(),
        removed: old.len() - matching.pairs.len(),
        changed: changed.len(),
        moved: matching.pairs.len() - kept_order(&matching.pairs),
        changed_blocks: if changed.len() <= NAMED_BLOCKS {
            changed.iter().map(|&index| named(new[index].raw)).collect()
        } else {
            Vec::new()
        },
    }
}

/// One block of a snapshot, reduced to what a save can change.
struct Block<'a> {
    raw: &'a Value,
    kind: (&'a Value, &'a Value),
    content: (&'a Value, &'a Value, &'a Value, Vec<Attachment<'a>>),
}

type Attachment<'a> = (&'a Value, &'a Value, &'a Value);

fn blocks(state: &Value) -> Vec<Block<'_>> {
    state["o:block"]
        .as_array()
        .map(|list| list.iter().map(block).collect())
        .unwrap_or_default()
}

fn block(raw: &Value) -> Block<'_> {
    let attachments = raw["o:attachment"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|entry| {
                    (
                        &entry["o:item"]["o:id"],
                        &entry["o:media"]["o:id"],
                        &entry["o:caption"],
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    Block {
        raw,
        kind: (&raw["o:layout"], &raw["o:layout_data"]["romeka"]["block"]),
        content: (
            &raw["o:layout"],
            &raw["o:data"],
            &raw["o:layout_data"],
            attachments,
        ),
    }
}

fn named(raw: &Value) -> ChangedBlock {
    ChangedBlock {
        layout: raw["o:layout"].clone(),
        layout_data: raw["o:layout_data"].clone(),
    }
}

/// Whether an older block may pair with a newer one in a matching pass.
type SameBlock = fn(&Block<'_>, &Block<'_>) -> bool;

/// A block of the older state paired with one of the newer, by index.
struct Pair {
    old: usize,
    new: usize,
    changed: bool,
}

struct Matching {
    /// In the newer state's order.
    pairs: Vec<Pair>,
}

impl Matching {
    fn of(old: &[Block<'_>], new: &[Block<'_>]) -> Self {
        let mut taken = vec![false; old.len()];
        let mut partner: Vec<Option<Pair>> = new.iter().map(|_| None).collect();
        let passes: [(bool, SameBlock); 2] = [
            (false, |a, b| a.content == b.content),
            (true, |a, b| a.kind == b.kind),
        ];
        for (changed, same) in passes {
            for (index, block) in new.iter().enumerate() {
                if partner[index].is_some() {
                    continue;
                }
                let found = (0..old.len()).find(|&at| !taken[at] && same(&old[at], block));
                if let Some(at) = found {
                    taken[at] = true;
                    partner[index] = Some(Pair {
                        old: at,
                        new: index,
                        changed,
                    });
                }
            }
        }
        Self {
            pairs: partner.into_iter().flatten().collect(),
        }
    }
}

/// The length of the longest run of pairs whose older positions rise with the newer ones: the
/// blocks that kept their order. Patience sorting, `O(n log n)`.
fn kept_order(pairs: &[Pair]) -> usize {
    let mut tails: Vec<usize> = Vec::new();
    for pair in pairs {
        let at = tails.partition_point(|&tail| tail < pair.old);
        if at == tails.len() {
            tails.push(pair.old);
        } else {
            tails[at] = pair.old;
        }
    }
    tails.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn html(text: &str) -> Value {
        json!({ "o:layout": "html", "o:data": { "html": text }, "o:layout_data": null, "o:attachment": [] })
    }

    fn hero(heading: &str) -> Value {
        json!({
            "o:layout": "html",
            "o:data": { "html": heading },
            "o:layout_data": { "romeka": { "block": "hero", "v": 1, "settings": { "heading": heading } } },
            "o:attachment": [],
        })
    }

    fn page(title: &str, blocks: &[Value]) -> Value {
        json!({ "o:title": title, "o:slug": "intro", "o:is_public": true, "o:layout": null,
                "o:layout_data": [], "o:block": blocks })
    }

    fn counts(changes: &VersionChanges) -> [usize; 4] {
        [
            changes.added,
            changes.removed,
            changes.changed,
            changes.moved,
        ]
    }

    #[test]
    fn an_identical_save_changes_nothing() {
        let state = page("Intro", &[html("a"), hero("Welcome")]);
        let changes = changes(&state, &state);
        assert_eq!(counts(&changes), [0, 0, 0, 0]);
        assert_eq!(changes.title, None);
        assert!(!changes.settings_changed);
        assert!(changes.changed_blocks.is_empty());
    }

    #[test]
    fn a_block_inserted_at_the_top_is_one_added_and_nothing_moved() {
        let before = page("Intro", &[html("a"), html("b")]);
        let after = page("Intro", &[html("new"), html("a"), html("b")]);
        assert_eq!(counts(&changes(&before, &after)), [1, 0, 0, 0]);
    }

    #[test]
    fn a_deleted_block_is_one_removed() {
        let before = page("Intro", &[html("a"), html("b"), html("c")]);
        let after = page("Intro", &[html("a"), html("c")]);
        assert_eq!(counts(&changes(&before, &after)), [0, 1, 0, 0]);
    }

    #[test]
    fn an_edited_block_is_one_changed_and_named_by_its_newer_state() {
        let before = page("Intro", &[html("a"), hero("Welcome")]);
        let after = page("Intro", &[html("a"), hero("Hello")]);
        let changes = changes(&before, &after);
        assert_eq!(counts(&changes), [0, 0, 1, 0]);
        assert_eq!(
            changes.changed_blocks,
            [ChangedBlock {
                layout: json!("html"),
                layout_data: hero("Hello")["o:layout_data"].clone(),
            }]
        );
    }

    #[test]
    fn two_swapped_blocks_are_one_moved() {
        let before = page("Intro", &[html("a"), html("b"), html("c")]);
        let after = page("Intro", &[html("b"), html("a"), html("c")]);
        assert_eq!(counts(&changes(&before, &after)), [0, 0, 0, 1]);
    }

    #[test]
    fn a_block_moved_from_the_end_to_the_top_is_one_moved() {
        let before = page("Intro", &[html("a"), html("b"), html("c"), html("d")]);
        let after = page("Intro", &[html("d"), html("a"), html("b"), html("c")]);
        assert_eq!(counts(&changes(&before, &after)), [0, 0, 0, 1]);
    }

    #[test]
    fn repeated_equal_blocks_pair_in_order() {
        let before = page("Intro", &[html("rule"), html("rule")]);
        let after = page("Intro", &[html("rule"), html("b"), html("rule")]);
        assert_eq!(counts(&changes(&before, &after)), [1, 0, 0, 0]);
    }

    #[test]
    fn a_changed_block_pairs_only_with_a_block_of_its_own_kind() {
        let before = page("Intro", &[html("a")]);
        let after = page("Intro", &[hero("a")]);
        assert_eq!(counts(&changes(&before, &after)), [1, 1, 0, 0]);
    }

    #[test]
    fn a_retitled_page_names_both_titles() {
        let changes = changes(&page("Intro", &[]), &page("Welcome", &[]));
        assert_eq!(
            changes.title,
            Some(TitleChange {
                before: "Intro".to_string(),
                after: "Welcome".to_string(),
            })
        );
        assert!(!changes.settings_changed);
    }

    #[test]
    fn a_hidden_page_is_a_settings_change() {
        let before = page("Intro", &[]);
        let mut after = before.clone();
        after["o:is_public"] = json!(false);
        assert!(changes(&before, &after).settings_changed);
    }

    #[test]
    fn more_than_two_changed_blocks_are_counted_and_not_named() {
        let before = page("Intro", &[html("a"), html("b"), html("c")]);
        let after = page("Intro", &[html("x"), html("y"), html("z")]);
        let changes = changes(&before, &after);
        assert_eq!(counts(&changes), [0, 0, 3, 0]);
        assert!(changes.changed_blocks.is_empty());
    }

    #[test]
    fn an_attachment_url_alone_is_no_change() {
        let attached = |host: &str, caption: &str| {
            json!({ "o:layout": "media", "o:data": {}, "o:layout_data": null, "o:attachment": [{
                "o:item": { "@id": format!("{host}/api/items/7"), "o:id": 7 },
                "o:media": null,
                "o:caption": caption,
            }] })
        };
        let before = page("Intro", &[attached("http://a", "")]);
        let moved_host = page("Intro", &[attached("https://b", "")]);
        let captioned = page("Intro", &[attached("https://b", "A map")]);
        assert_eq!(counts(&changes(&before, &moved_host)), [0, 0, 0, 0]);
        assert_eq!(counts(&changes(&before, &captioned)), [0, 0, 1, 0]);
    }
}
