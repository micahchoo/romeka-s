//! The `nameWords` section: for the name of a marked example, the words at
//! each place of every file name with as many parts, with how many files
//! have each (`docs/import-v2.md`, "Name words"). Exact text: nothing is
//! folded, so the person ticks the words that are another file of one item.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::query::{Page, page};
use crate::import2::grouping::natural_cmp;
use crate::import2::scan::StoredIndex;
use crate::import2::values::fold_title;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NameWordsQuery {
    /// The example's path in its source; only its name is split.
    #[serde(default)]
    pub path: Option<String>,
    /// The page of every place.
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
    /// Words holding this text, compared as titles, at every place.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameWords {
    /// The example's name, split.
    pub parts: Vec<String>,
    /// File names with as many parts, over every source.
    pub names: usize,
    /// One page per part, by count, then in natural order.
    pub places: Vec<Page<WordCount>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordCount {
    pub text: String,
    pub count: usize,
}

/// The parts of a file name between `_`, `-`, `.` and space. Two
/// delimiters in a row hold an empty part, so every name splits the same
/// way the admin shows it.
fn split(name: &str) -> impl Iterator<Item = &str> {
    name.split(['_', '-', '.', ' '])
}

pub(super) fn name_words(stored: &StoredIndex, query: &NameWordsQuery) -> NameWords {
    let Some(path) = query.path.as_deref() else {
        return NameWords {
            parts: Vec::new(),
            names: 0,
            places: Vec::new(),
        };
    };
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    let parts: Vec<String> = split(name).map(str::to_string).collect();
    let mut counts: Vec<HashMap<&str, usize>> = vec![HashMap::new(); parts.len()];
    let mut names = 0;
    let mut words: Vec<&str> = Vec::with_capacity(parts.len() + 1);
    for entry in stored
        .index
        .sources
        .iter()
        .flat_map(|source| &source.walk.entries)
    {
        words.clear();
        words.extend(split(entry.name()).take(parts.len() + 1));
        if words.len() != parts.len() {
            continue;
        }
        names += 1;
        for (place, word) in words.iter().enumerate() {
            *counts[place].entry(word).or_default() += 1;
        }
    }
    let text = query.text.as_deref().map(fold_title);
    let places = counts
        .into_iter()
        .map(|place| {
            let mut found: Vec<(&str, usize)> = place
                .into_iter()
                .filter(|(word, _)| {
                    text.as_ref()
                        .is_none_or(|text| fold_title(word).contains(text.as_str()))
                })
                .collect();
            found.sort_by(|(a, a_count), (b, b_count)| {
                b_count.cmp(a_count).then_with(|| natural_cmp(a, b))
            });
            page(
                found.into_iter(),
                query.page,
                query.per_page,
                |(text, count)| WordCount {
                    text: text.to_string(),
                    count,
                },
            )
        })
        .collect();
    NameWords {
        parts,
        names,
        places,
    }
}
