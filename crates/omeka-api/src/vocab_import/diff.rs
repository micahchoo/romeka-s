//! `RdfImporter::calculateDiff`: what an update from a file would change.
//!
//! Upstream computes only what it can apply safely: members the file adds, and labels and
//! comments the file changes. It never deletes a member or renames one, because values and
//! item types point at members. [`MemberDiff::removed`] lists the stored members the file no
//! longer declares, so the reviewer knows they stay; applying a diff ignores it.

use serde::{Deserialize, Serialize};

use super::members::{Members, Term};

/// One changed label or comment: `[$from, $to]` keyed by local name upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change<T> {
    #[serde(rename = "o:local_name")]
    pub local_name: String,
    pub from: T,
    pub to: T,
}

/// One stored member the file no longer declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    #[serde(rename = "o:local_name")]
    pub local_name: String,
    #[serde(rename = "o:label")]
    pub label: String,
}

/// The diff for one kind of member: upstream's `new`, `label` and `comment` lists, and the
/// members kept although the file dropped them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberDiff {
    #[serde(default)]
    pub new: Vec<Term>,
    #[serde(default)]
    pub label: Vec<Change<String>>,
    #[serde(default)]
    pub comment: Vec<Change<Option<String>>>,
    #[serde(default)]
    pub removed: Vec<Kept>,
}

impl MemberDiff {
    /// Whether applying the diff writes nothing.
    #[must_use]
    pub fn writes_nothing(&self) -> bool {
        self.new.is_empty() && self.label.is_empty() && self.comment.is_empty()
    }
}

/// `RdfImporter::getDiff`'s answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diff {
    #[serde(default)]
    pub classes: MemberDiff,
    #[serde(default)]
    pub properties: MemberDiff,
}

/// The diff from the stored members to the members a file declares.
#[must_use]
pub fn diff(stored: &Members, file: &Members) -> Diff {
    Diff {
        classes: diff_kind(&stored.classes, &file.classes),
        properties: diff_kind(&stored.properties, &file.properties),
    }
}

fn diff_kind(stored: &[Term], file: &[Term]) -> MemberDiff {
    let mut diff = MemberDiff::default();
    for term in file {
        let Some(current) = stored.iter().find(|row| row.local_name == term.local_name) else {
            diff.new.push(term.clone());
            continue;
        };
        if current.label != term.label {
            diff.label.push(Change {
                local_name: term.local_name.clone(),
                from: current.label.clone(),
                to: term.label.clone(),
            });
        }
        if current.comment != term.comment {
            diff.comment.push(Change {
                local_name: term.local_name.clone(),
                from: current.comment.clone(),
                to: term.comment.clone(),
            });
        }
    }
    diff.removed = stored
        .iter()
        .filter(|row| file.iter().all(|term| term.local_name != row.local_name))
        .map(|row| Kept {
            local_name: row.local_name.clone(),
            label: row.label.clone(),
        })
        .collect();
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(local_name: &str, label: &str, comment: Option<&str>) -> Term {
        Term {
            local_name: local_name.to_string(),
            label: label.to_string(),
            comment: comment.map(str::to_string),
        }
    }

    fn properties(terms: Vec<Term>) -> Members {
        Members {
            classes: Vec::new(),
            properties: terms,
        }
    }

    /// `calculateDiff`: a member the file adds is new, a changed label and a changed comment are
    /// listed apart, an unchanged member is absent, and a dropped member is only reported.
    #[test]
    fn the_diff_lists_new_members_changed_texts_and_kept_members() {
        let stored = properties(vec![
            term("name", "Name", Some("A name.")),
            term("age", "Age", None),
            term("gone", "Gone", None),
        ]);
        let file = properties(vec![
            term("name", "Full name", Some("A name.")),
            term("age", "Age", Some("Years.")),
            term("knows", "Knows", None),
        ]);
        let diff = diff(&stored, &file).properties;
        assert_eq!(diff.new, [term("knows", "Knows", None)]);
        assert_eq!(
            diff.label,
            [Change {
                local_name: "name".to_string(),
                from: "Name".to_string(),
                to: "Full name".to_string(),
            }]
        );
        assert_eq!(
            diff.comment,
            [Change {
                local_name: "age".to_string(),
                from: None,
                to: Some("Years.".to_string()),
            }]
        );
        assert_eq!(
            diff.removed,
            [Kept {
                local_name: "gone".to_string(),
                label: "Gone".to_string(),
            }]
        );
    }

    #[test]
    fn an_unchanged_file_is_an_empty_diff() {
        let members = properties(vec![term("name", "Name", Some("A name."))]);
        let diff = diff(&members, &members);
        assert!(diff.properties.writes_nothing() && diff.properties.removed.is_empty());
        assert!(diff.classes.writes_nothing());
    }
}
